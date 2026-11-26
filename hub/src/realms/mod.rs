//! Hosted worlds: what the hub knows of them, and their lifecycle, `stopped → starting →
//! running → stopping → stopped`, `failed` when a start or a server goes wrong, `archived` once
//! deleted. The agent runs their containers, the status probe counts their players, and
//! [`monitor`] follows them.

pub mod agent;
pub mod monitor;
pub mod probe;
mod routes;

use std::{sync::Arc, time::Instant};

use anyhow::bail;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use rustcraft_agent_protocol::Request;
use rustcraft_protocol::api::{ErrorCode, RealmSettings, RealmState, StartRealmResponse};
use sha2::{Digest, Sha256};
use sqlx::{types::Json, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{error::AppError, ratelimit::RealmStartLimiter, AppState};
use agent::{failure, RealmAgent};
use probe::StatusProbe;

pub use routes::routes;

/// The advisory lock under which a start counts the running worlds and takes a slot.
pub(crate) const SLOTS_LOCK: i64 = 0x5245_414c_4d53;

/// How the hub runs hosted worlds. `base_port` and `slots` must be those of the agent's
/// configuration: slot `n` is reached at `host`, port `base_port + 2n`.
pub struct Hosting {
    pub agent: Arc<dyn RealmAgent>,
    pub probe: Arc<dyn StatusProbe>,
    /// The host players join the worlds at: `--realm-host`.
    pub host: String,
    pub base_port: u16,
    pub slots: u16,
    /// The most worlds that have a container at once.
    pub max_running: u16,
    pub starts: RealmStartLimiter,
}

impl Hosting {
    pub fn new(
        agent: Arc<dyn RealmAgent>,
        probe: Arc<dyn StatusProbe>,
        host: String,
        base_port: u16,
        slots: u16,
        max_running: u16,
    ) -> anyhow::Result<Self> {
        if slots == 0
            || slots > i16::MAX as u16
            || u32::from(base_port) + 2 * u32::from(slots) > 65536
        {
            bail!("{slots} slots from port {base_port} do not fit below 65536");
        }
        Ok(Self {
            agent,
            probe,
            host,
            base_port,
            slots,
            max_running,
            starts: Default::default(),
        })
    }

    /// The game port of `slot`; its status port is the next one.
    pub fn game_port(&self, slot: i16) -> u16 {
        self.base_port + 2 * slot as u16
    }
}

pub(crate) fn parse_state(name: &str) -> RealmState {
    match name {
        "starting" => RealmState::Starting,
        "running" => RealmState::Running,
        "stopping" => RealmState::Stopping,
        "failed" => RealmState::Failed,
        "archived" => RealmState::Archived,
        _ => RealmState::Stopped,
    }
}

pub(crate) fn token_hash(token: &[u8]) -> [u8; 32] {
    Sha256::digest(token).into()
}

fn internal() -> AppError {
    AppError::new(ErrorCode::Internal, "Something went wrong on the hub.")
}

fn not_found() -> AppError {
    AppError::new(ErrorCode::NotFound, "There is no such world.")
}

fn no_free_slot() -> AppError {
    AppError::new(
        ErrorCode::NoFreeSlot,
        "The hub runs as many worlds as it can. Try again later.",
    )
}

/// Starts a stopped or failed world: takes the lowest free slot, issues a new realm token and
/// asks the agent. A world that is starting or running already is left as it is.
pub async fn start(state: &AppState, id: Uuid) -> Result<StartRealmResponse, AppError> {
    let hosting = &state.realms;
    let response = |state, slot| StartRealmResponse {
        state,
        host: hosting.host.clone(),
        port: hosting.game_port(slot),
    };
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(SLOTS_LOCK)
        .execute(&mut *tx)
        .await?;
    let (current, slot, Json(settings)): (String, Option<i16>, Json<RealmSettings>) =
        sqlx::query_as("SELECT state, slot, settings FROM realms WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(not_found)?;
    match parse_state(&current) {
        current @ (RealmState::Starting | RealmState::Running) => {
            return Ok(response(current, slot.ok_or_else(internal)?));
        }
        RealmState::Stopping => {
            return Err(AppError::new(
                ErrorCode::InvalidRequest,
                "The world is stopping. Try again in a moment.",
            ));
        }
        RealmState::Archived => return Err(not_found()),
        RealmState::Stopped | RealmState::Failed => {}
    }
    let used: Vec<i16> = sqlx::query_scalar("SELECT slot FROM realms WHERE slot IS NOT NULL")
        .fetch_all(&mut *tx)
        .await?;
    if used.len() >= usize::from(hosting.max_running) {
        return Err(no_free_slot());
    }
    let slot = (0..hosting.slots as i16)
        .find(|slot| !used.contains(slot))
        .ok_or_else(no_free_slot)?;
    hosting.starts.start(id, Instant::now())?;
    let mut token = [0u8; 32];
    OsRng.fill_bytes(&mut token);
    sqlx::query(
        "UPDATE realms SET state = 'starting', slot = $2, token_hash = $3, players = 0, \
             state_changed_at = $4, last_empty_at = NULL, failed_reason = NULL \
         WHERE id = $1",
    )
    .bind(id)
    .bind(slot)
    .bind(&token_hash(&token)[..])
    .bind(OffsetDateTime::now_utc())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let request = Request::Start {
        realm: id,
        slot: slot as u16,
        token: URL_SAFE_NO_PAD.encode(token),
        settings,
    };
    if let Some(reason) = failure(&hosting.agent.call(request).await) {
        tracing::warn!(%id, reason, "a world did not start");
        // The agent cleans up a start it refused, but one it did not answer may have a container
        let _ = stop(state, id, Some("The world could not be started.")).await;
        return Err(AppError::new(
            ErrorCode::Internal,
            "The world could not be started. Try again later.",
        ));
    }
    Ok(response(RealmState::Starting, slot))
}

/// Stops a world that is starting, running or stopping already, and ends `failed` with
/// `failure_reason` when there is one. When the agent does not stop it, the world stays
/// `stopping` and the monitor tries again.
pub async fn stop(
    state: &AppState,
    id: Uuid,
    failure_reason: Option<&str>,
) -> Result<(), AppError> {
    let stopping = sqlx::query(
        "UPDATE realms SET state = 'stopping', state_changed_at = $2, \
             failed_reason = COALESCE($3, failed_reason) \
         WHERE id = $1 AND state IN ('starting', 'running', 'stopping')",
    )
    .bind(id)
    .bind(OffsetDateTime::now_utc())
    .bind(failure_reason)
    .execute(&state.db)
    .await?;
    if stopping.rows_affected() == 0 {
        return Ok(());
    }
    if let Some(reason) = failure(&state.realms.agent.call(Request::Stop { realm: id }).await) {
        tracing::warn!(%id, reason, "a world did not stop");
        return Err(internal());
    }
    finish_stop(&state.db, id).await?;
    Ok(())
}

/// Marks a stopping world whose container is gone `stopped`, or `failed` when it stopped for
/// a failure, and frees its slot and token.
pub(crate) async fn finish_stop(db: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE realms SET \
             state = CASE WHEN failed_reason IS NULL THEN 'stopped' ELSE 'failed' END, \
             slot = NULL, token_hash = NULL, players = 0, last_empty_at = NULL, \
             state_changed_at = $2 \
         WHERE id = $1 AND state = 'stopping'",
    )
    .bind(id)
    .bind(OffsetDateTime::now_utc())
    .execute(db)
    .await?;
    Ok(())
}

/// Asks the agent to archive the volumes of worlds marked `archived`, one after the other, in
/// the background. A world the agent does not archive keeps its slot, as its container may
/// still run, until the daily task tries again.
pub fn archive_volumes(state: AppState, ids: Vec<Uuid>) {
    if ids.is_empty() {
        return;
    }
    tokio::spawn(async move {
        for id in ids {
            if let Err(error) = archive_volume(&state, id).await {
                tracing::warn!(%id, %error, "could not record an archive");
            }
        }
    });
}

pub(crate) async fn archive_volume(state: &AppState, id: Uuid) -> Result<(), sqlx::Error> {
    if let Some(reason) = failure(
        &state
            .realms
            .agent
            .call(Request::Archive { realm: id })
            .await,
    ) {
        tracing::warn!(%id, reason, "a world was not archived");
        return Ok(());
    }
    sqlx::query(
        "UPDATE realms SET slot = NULL, volume_archived_at = $2 \
         WHERE id = $1 AND state = 'archived'",
    )
    .bind(id)
    .bind(OffsetDateTime::now_utc())
    .execute(&state.db)
    .await?;
    Ok(())
}

/// The SQL that marks worlds archived: they leave the lists, and their servers lose their
/// token, at once.
pub(crate) const MARK_ARCHIVED: &str = "UPDATE realms SET state = 'archived', \
     token_hash = NULL, players = 0, last_empty_at = NULL, state_changed_at = now()";

/// Reconciles the worlds with the agent, archives the worlds left marked `archived` but not
/// archived by the agent, and asks the agent to purge the old archives.
pub async fn daily(state: &AppState) -> Result<(), sqlx::Error> {
    monitor::reconcile(state).await?;
    let pending: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM realms WHERE state = 'archived' AND volume_archived_at IS NULL",
    )
    .fetch_all(&state.db)
    .await?;
    for id in pending {
        archive_volume(state, id).await?;
    }
    match state.realms.agent.call(Request::Purge).await {
        Ok(rustcraft_agent_protocol::Response::Purged { volumes }) => {
            tracing::info!(?volumes, "purged old archives of worlds");
        }
        other => tracing::warn!(?other, "the agent did not purge the old archives"),
    }
    Ok(())
}
