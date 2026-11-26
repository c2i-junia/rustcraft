//! Follows the worlds that have a container: a starting world is `running` once it answers on
//! its status port, or `failed` after [`Timings::start_timeout`]; a running world stops after
//! [`Timings::idle_timeout`] without a player, and ends `stopped` or `failed` when its server
//! exits by itself. Each check takes the time it runs at, so that tests choose it.

use std::time::Duration;

use rustcraft_agent_protocol::{RealmState as ContainerState, Request, Response};
use rustcraft_protocol::status::status_port;
use time::OffsetDateTime;
use uuid::Uuid;

use super::{finish_stop, stop};
use crate::AppState;

/// How often the monitor looks, and how long it waits.
#[derive(Clone, Debug)]
pub struct Timings {
    /// How often starting worlds are probed.
    pub starting_every: Duration,
    /// How often running worlds are probed, and stopping ones looked at.
    pub running_every: Duration,
    /// How long a world has to answer on its status port after its start.
    pub start_timeout: time::Duration,
    /// How long a running world may have no player.
    pub idle_timeout: time::Duration,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            starting_every: Duration::from_secs(5),
            running_every: Duration::from_secs(30),
            start_timeout: time::Duration::seconds(60),
            idle_timeout: time::Duration::minutes(10),
        }
    }
}

#[derive(sqlx::FromRow)]
struct Active {
    id: Uuid,
    state: String,
    slot: i16,
    state_changed_at: OffsetDateTime,
    last_empty_at: Option<OffsetDateTime>,
}

async fn active(state: &AppState, states: &[&str]) -> Result<Vec<Active>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, state, slot, state_changed_at, last_empty_at FROM realms \
         WHERE state = ANY($1) AND slot IS NOT NULL ORDER BY slot",
    )
    .bind(states)
    .fetch_all(&state.db)
    .await
}

async fn players(state: &AppState, slot: i16) -> Option<u32> {
    let port = status_port(state.realms.game_port(slot))?;
    state.realms.probe.players(port).await
}

/// The state of a world's container, or nothing when the agent does not tell.
async fn container(state: &AppState, id: Uuid) -> Option<ContainerState> {
    match state.realms.agent.call(Request::Status { realm: id }).await {
        Ok(Response::Status { state }) => Some(state),
        other => {
            tracing::warn!(%id, ?other, "the agent did not tell the state of a world");
            None
        }
    }
}

/// Why a world whose container is gone failed, or nothing when its server ended well.
fn exit_failure(container: ContainerState) -> Option<String> {
    match container {
        ContainerState::Exited { exit_code: Some(0) } | ContainerState::Running => None,
        ContainerState::Exited {
            exit_code: Some(code),
        } => Some(format!("The server exited with code {code}.")),
        ContainerState::Exited { exit_code: None } | ContainerState::Absent => {
            Some("The server is gone.".into())
        }
    }
}

/// Ends a world whose container is gone: `stopped`, or `failed` with `reason`.
async fn gone(state: &AppState, id: Uuid, reason: Option<String>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE realms SET state = 'stopping', failed_reason = COALESCE($2, failed_reason) \
         WHERE id = $1 AND state IN ('starting', 'running', 'stopping')",
    )
    .bind(id)
    .bind(reason)
    .execute(&state.db)
    .await?;
    finish_stop(&state.db, id).await
}

/// Records what a probe of a running world found at `now`; a world that does not answer
/// counts as empty, so that a server that hangs stops too.
async fn record_players(
    state: &AppState,
    id: Uuid,
    players: Option<u32>,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    let players = players.unwrap_or(0);
    sqlx::query(
        "UPDATE realms SET players = $2, \
             last_empty_at = CASE WHEN $2 = 0 THEN COALESCE(last_empty_at, $3) END, \
             last_played_at = CASE WHEN $2 > 0 THEN $3 ELSE last_played_at END \
         WHERE id = $1",
    )
    .bind(id)
    .bind(i32::try_from(players).unwrap_or(i32::MAX))
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Probes the starting worlds: those that answer are `running`, those whose server is gone or
/// that have not answered within the timeout are stopped and `failed`.
pub async fn check_starting(
    state: &AppState,
    timings: &Timings,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    for realm in active(state, &["starting"]).await? {
        if let Some(count) = players(state, realm.slot).await {
            sqlx::query(
                "UPDATE realms SET state = 'running', state_changed_at = $2 \
                 WHERE id = $1 AND state = 'starting'",
            )
            .bind(realm.id)
            .bind(now)
            .execute(&state.db)
            .await?;
            record_players(state, realm.id, Some(count), now).await?;
            continue;
        }
        match container(state, realm.id).await {
            Some(ContainerState::Absent | ContainerState::Exited { .. }) => {
                let reason = "The server ended before it answered.".to_owned();
                gone(state, realm.id, Some(reason)).await?;
            }
            _ if now - realm.state_changed_at >= timings.start_timeout => {
                let reason = format!(
                    "The server did not answer within {} seconds.",
                    timings.start_timeout.whole_seconds()
                );
                // A world the agent does not stop stays `stopping`, which `check_running` retries
                let _ = stop(state, realm.id, Some(&reason)).await;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Looks at the running and stopping worlds: a world whose container is gone ends, a stopping
/// one whose container runs is stopped again, and a running one stops once it has had no
/// player for the idle timeout.
pub async fn check_running(
    state: &AppState,
    timings: &Timings,
    now: OffsetDateTime,
) -> Result<(), sqlx::Error> {
    for realm in active(state, &["running", "stopping"]).await? {
        let Some(container) = container(state, realm.id).await else {
            continue;
        };
        if container != ContainerState::Running {
            // A stopping world's container is gone because it was stopped: the stop ends
            let reason = match realm.state.as_str() {
                "running" => exit_failure(container),
                _ => None,
            };
            gone(state, realm.id, reason).await?;
            continue;
        }
        if realm.state == "stopping" {
            let _ = stop(state, realm.id, None).await;
            continue;
        }
        let count = players(state, realm.slot).await;
        if count.unwrap_or(0) == 0
            && realm
                .last_empty_at
                .is_some_and(|since| now - since >= timings.idle_timeout)
        {
            tracing::info!(id = %realm.id, "stopping a world without players");
            let _ = stop(state, realm.id, None).await;
            continue;
        }
        record_players(state, realm.id, count, now).await?;
    }
    Ok(())
}

/// Brings the database in line with the agent, when the hub starts and every day: a world whose
/// container is gone ends, a stopping one is stopped again, and the container of a world
/// without a slot, left by a start whose row was cleared while the agent started it, is
/// stopped. Starting worlds go on with their timeout.
pub async fn reconcile(state: &AppState) -> Result<(), sqlx::Error> {
    for realm in active(state, &["starting", "running", "stopping"]).await? {
        match container(state, realm.id).await {
            Some(ContainerState::Running) if realm.state == "stopping" => {
                let _ = stop(state, realm.id, None).await;
            }
            Some(container @ (ContainerState::Absent | ContainerState::Exited { .. })) => {
                let reason = match realm.state.as_str() {
                    "starting" => Some("The server ended before it answered.".to_owned()),
                    "running" => exit_failure(container),
                    _ => None,
                };
                gone(state, realm.id, reason).await?;
            }
            _ => {}
        }
    }
    let idle: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM realms WHERE state IN ('stopped', 'failed')")
            .fetch_all(&state.db)
            .await?;
    for id in idle {
        // Only a running container: an exited one is removed by the next start. A start that
        // comes after this check is refused by the agent while the stray container runs, so
        // this stop never ends a world started since.
        if container(state, id).await == Some(ContainerState::Running) {
            tracing::warn!(%id, "stopping the container of a world without a slot");
            let stopped = state.realms.agent.call(Request::Stop { realm: id }).await;
            if let Some(reason) = super::agent::failure(&stopped) {
                tracing::warn!(%id, reason, "could not stop a stray container");
            }
        }
    }
    Ok(())
}

/// Reconciles, then checks the starting and the running worlds at their intervals.
pub fn spawn(state: AppState, timings: Timings) {
    tokio::spawn(async move {
        if let Err(error) = reconcile(&state).await {
            tracing::warn!(%error, "could not reconcile the worlds with the agent");
        }
        let mut starting = tokio::time::interval(timings.starting_every);
        let mut running = tokio::time::interval(timings.running_every);
        let now = OffsetDateTime::now_utc;
        loop {
            let result = tokio::select! {
                _ = starting.tick() => check_starting(&state, &timings, now()).await,
                _ = running.tick() => check_running(&state, &timings, now()).await,
            };
            if let Err(error) = result {
                tracing::warn!(%error, "could not check the worlds");
            }
        }
    });
}
