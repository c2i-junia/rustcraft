//! The requests carried out on Docker.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use bollard::errors::Error as DockerError;
use bollard::models::{ContainerStateStatusEnum, VolumeCreateRequest};
use bollard::query_parameters::{
    CreateContainerOptionsBuilder, ListVolumesOptionsBuilder, RemoveContainerOptionsBuilder,
    StopContainerOptionsBuilder, WaitContainerOptionsBuilder,
};
use bollard::Docker;
use futures::StreamExt;
use rustcraft_agent_protocol::{RealmState, Request, Response};
use rustcraft_protocol::api::RealmSettings;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::config::Config;
use crate::spec::{self, archive_step, archived_at, realm_name, ArchiveStep};

/// How long an archive's copy may take.
const COPY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// The agent: its configuration, Docker, and a lock per world, so that two requests on one
/// world do not overlap while requests on different worlds run at once.
pub struct Agent {
    config: Config,
    docker: Docker,
    locks: Mutex<HashMap<Uuid, Arc<tokio::sync::Mutex<()>>>>,
}

/// Why a valid request was not carried out.
type Failure = String;

impl Agent {
    pub fn new(config: Config, docker: Docker) -> Agent {
        Agent {
            config,
            docker,
            locks: Default::default(),
        }
    }

    /// Carries out `request` and answers it.
    pub async fn handle(&self, request: Request) -> Response {
        let realm = match &request {
            Request::Start { realm, .. }
            | Request::Stop { realm }
            | Request::Status { realm }
            | Request::Archive { realm } => Some(*realm),
            Request::Purge => None,
        };
        let lock = realm.map(|realm| self.lock(realm));
        let _guard = match &lock {
            Some(lock) => Some(lock.lock().await),
            None => None,
        };
        let result = match request {
            Request::Start {
                realm,
                slot,
                token,
                settings,
            } => match spec::check_start(&self.config, slot, &token, &settings) {
                Ok(()) => self.start(realm, slot, &token, &settings).await,
                Err(reason) => return Response::Refused { reason },
            },
            Request::Stop { realm } => self.stop(realm).await.map(|()| Response::Done),
            Request::Status { realm } => self
                .status(realm)
                .await
                .map(|state| Response::Status { state }),
            Request::Archive { realm } => self.archive(realm).await,
            Request::Purge => self.purge().await,
        };
        result.unwrap_or_else(|error| Response::Failed { error })
    }

    fn lock(&self, realm: Uuid) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.locks.lock().expect("no panic while it is held");
        locks.retain(|_, lock| Arc::strong_count(lock) > 1);
        locks.entry(realm).or_default().clone()
    }

    async fn start(
        &self,
        realm: Uuid,
        slot: u16,
        token: &str,
        settings: &RealmSettings,
    ) -> Result<Response, Failure> {
        let name = realm_name(realm);
        match self.status(realm).await? {
            RealmState::Running => {
                return Ok(Response::Refused {
                    reason: "the world is running".into(),
                })
            }
            // Left over by a server that exited: the new start brings a new token and image
            RealmState::Exited { .. } => self.remove_container(&name).await?,
            RealmState::Absent => {}
        }
        let new_volume = !self.volume_exists(&name).await?;
        if new_volume {
            self.docker
                .create_volume(VolumeCreateRequest {
                    name: Some(name.clone()),
                    labels: Some(spec::labels(realm)),
                    ..Default::default()
                })
                .await
                .map_err(failed("create the volume"))?;
        }
        let body = spec::server_spec(&self.config, realm, slot, token, settings)
            .expect("checked by check_start");
        let options = CreateContainerOptionsBuilder::default().name(&name).build();
        let started = match self.docker.create_container(Some(options), body).await {
            Ok(_) => self
                .docker
                .start_container(&name, None)
                .await
                .map_err(failed("start the container")),
            Err(err) => Err(failed("create the container")(err)),
        };
        if let Err(error) = started {
            // A port taken by another world, say: nothing is left behind
            let removed = self.remove_container(&name).await;
            if new_volume {
                if let Err(err) = self.docker.remove_volume(&name, None::<RemoveVolume>).await {
                    failed("remove the new volume")(err);
                }
            }
            return Err(removed.err().unwrap_or(error));
        }
        tracing::info!(%realm, slot, "started");
        Ok(Response::Done)
    }

    async fn volume_exists(&self, name: &str) -> Result<bool, Failure> {
        match self.docker.inspect_volume(name).await {
            Ok(_) => Ok(true),
            Err(err) if is_not_found(&err) => Ok(false),
            Err(err) => Err(failed("inspect a volume")(err)),
        }
    }

    async fn stop(&self, realm: Uuid) -> Result<(), Failure> {
        let name = realm_name(realm);
        if self.status(realm).await? == RealmState::Absent {
            return Ok(());
        }
        let grace = i32::try_from(self.config.stop_grace_seconds).unwrap_or(i32::MAX);
        let options = StopContainerOptionsBuilder::default().t(grace).build();
        match self.docker.stop_container(&name, Some(options)).await {
            // 304: it was not running
            Ok(())
            | Err(DockerError::DockerResponseServerError {
                status_code: 304, ..
            }) => {}
            Err(err) => return Err(failed("stop the container")(err)),
        }
        self.remove_container(&name).await?;
        tracing::info!(%realm, "stopped");
        Ok(())
    }

    async fn status(&self, realm: Uuid) -> Result<RealmState, Failure> {
        match self
            .docker
            .inspect_container(&realm_name(realm), None)
            .await
        {
            Ok(container) => {
                let state = container.state.unwrap_or_default();
                Ok(match state.status {
                    Some(
                        ContainerStateStatusEnum::RUNNING | ContainerStateStatusEnum::RESTARTING,
                    ) => RealmState::Running,
                    _ => RealmState::Exited {
                        exit_code: state.exit_code,
                    },
                })
            }
            Err(err) if is_not_found(&err) => Ok(RealmState::Absent),
            Err(err) => Err(failed("inspect the container")(err)),
        }
    }

    /// Docker cannot rename a volume: the world is copied into a new volume named after the
    /// second of its archive, by a throwaway container, and its volume is removed once the copy
    /// is done. An archive is never removed here: one left by an interrupted copy stays beside
    /// the world's volume, and goes with the others through `purge`.
    async fn archive(&self, realm: Uuid) -> Result<Response, Failure> {
        self.stop(realm).await?;
        let volume = realm_name(realm);
        let now = OffsetDateTime::now_utc();
        let at = time::PrimitiveDateTime::new(now.date(), now.time());
        let world_volume = self.volume_exists(&volume).await?;
        let archive_exists = self.volume_exists(&spec::archived_name(realm, at)).await?;
        let archive = match archive_step(realm, at, world_volume, archive_exists) {
            ArchiveStep::Nothing => return Ok(Response::Done),
            ArchiveStep::Refuse(archive) => {
                return Ok(Response::Refused {
                    reason: format!("{archive} exists already"),
                })
            }
            ArchiveStep::Copy(archive) => archive,
        };
        // The copy of an interrupted archive may still be there
        self.remove_container(&copy_name(realm)).await?;
        let mut labels = spec::labels(realm);
        labels.insert(
            "rustcraft.archived".into(),
            archive[archive.len() - 14..].into(),
        );
        self.docker
            .create_volume(VolumeCreateRequest {
                name: Some(archive.clone()),
                labels: Some(labels),
                ..Default::default()
            })
            .await
            .map_err(failed("create the archive"))?;
        // On a failure the world stays where it was, and the partial archive until `purge`
        self.copy(realm, &archive).await?;
        self.docker
            .remove_volume(&volume, None::<RemoveVolume>)
            .await
            .map_err(failed("remove the volume"))?;
        tracing::info!(%realm, archive, "archived");
        Ok(Response::Done)
    }

    async fn copy(&self, realm: Uuid, archive: &str) -> Result<(), Failure> {
        let name = copy_name(realm);
        let options = CreateContainerOptionsBuilder::default().name(&name).build();
        self.docker
            .create_container(Some(options), spec::copy_spec(&self.config, realm, archive))
            .await
            .map_err(failed("create the copy"))?;
        let copied = async {
            self.docker
                .start_container(&name, None)
                .await
                .map_err(failed("start the copy"))?;
            let options = WaitContainerOptionsBuilder::default()
                .condition("not-running")
                .build();
            // Longer than the client's 120 s: a large world takes its time
            let docker = self.docker.clone().with_timeout(COPY_TIMEOUT);
            let mut wait = docker.wait_container(&name, Some(options));
            match wait.next().await {
                Some(Ok(_)) => Ok(()),
                Some(Err(DockerError::DockerContainerWaitError { code, .. })) => {
                    Err(format!("the copy exited with {code}"))
                }
                Some(Err(err)) => Err(failed("wait for the copy")(err)),
                None => Err("the copy was not waited for".into()),
            }
        }
        .await;
        self.remove_container(&name).await?;
        copied
    }

    /// Removes the archives older than `archive_days`, by the time in their name.
    async fn purge(&self) -> Result<Response, Failure> {
        let oldest =
            OffsetDateTime::now_utc().date() - Duration::days(self.config.archive_days.into());
        let filters = HashMap::from([("name", vec!["rustcraft-realm-"])]);
        let options = ListVolumesOptionsBuilder::default()
            .filters(&filters)
            .build();
        let listed = self
            .docker
            .list_volumes(Some(options))
            .await
            .map_err(failed("list the volumes"))?;
        let mut volumes = Vec::new();
        for volume in listed.volumes.unwrap_or_default() {
            if archived_at(&volume.name).is_some_and(|at| at.date() < oldest) {
                self.docker
                    .remove_volume(&volume.name, None::<RemoveVolume>)
                    .await
                    .map_err(failed("remove an archive"))?;
                tracing::info!(volume = volume.name, "purged");
                volumes.push(volume.name);
            }
        }
        volumes.sort();
        Ok(Response::Purged { volumes })
    }

    async fn remove_container(&self, name: &str) -> Result<(), Failure> {
        let options = RemoveContainerOptionsBuilder::default().force(true).build();
        match self.docker.remove_container(name, Some(options)).await {
            Ok(()) => Ok(()),
            Err(err) if is_not_found(&err) => Ok(()),
            Err(err) => Err(failed("remove the container")(err)),
        }
    }
}

/// The throwaway container of an archive's copy.
fn copy_name(realm: Uuid) -> String {
    format!("{}-archiving", realm_name(realm))
}

type RemoveVolume = bollard::query_parameters::RemoveVolumeOptions;

fn is_not_found(err: &DockerError) -> bool {
    matches!(
        err,
        DockerError::DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

fn failed(what: &'static str) -> impl Fn(DockerError) -> Failure {
    move |err| {
        tracing::warn!("could not {what}: {err}");
        format!("could not {what}: {err}")
    }
}
