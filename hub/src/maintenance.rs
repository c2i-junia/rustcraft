//! The periodic tasks: forgetting old sign-ups, expired sessions and the names of deleted
//! accounts.

use std::time::{Duration, Instant};

use sqlx::PgConnection;
use time::OffsetDateTime;

use crate::{ratelimit::purge_signups, sessions, AppState};

/// How long a deleted account keeps its name.
pub const DELETED_NAME_HOLD: time::Duration = time::Duration::days(30);

/// Renames the accounts deleted [`DELETED_NAME_HOLD`] before `now` to `deleted-<id>`, username
/// and display name, which frees their names (`-` is not allowed in a username, so no one can
/// take that one).
pub async fn purge_deleted_accounts(
    conn: &mut PgConnection,
    now: OffsetDateTime,
) -> Result<u64, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE accounts SET username = 'deleted-' || id::text, \
             display_name = 'deleted-' || id::text \
         WHERE status = 'deleted' AND deleted_at <= $1 AND username NOT LIKE 'deleted-%'",
    )
    .bind(now - DELETED_NAME_HOLD)
    .execute(conn)
    .await?;
    Ok(done.rows_affected())
}

async fn hourly(state: &AppState) -> Result<(), sqlx::Error> {
    let mut conn = state.db.acquire().await?;
    let signups = purge_signups(&mut conn, OffsetDateTime::now_utc()).await?;
    let sessions = sessions::purge_expired(&mut conn).await?;
    state.logins.prune(Instant::now());
    state.certificates.prune(Instant::now());
    tracing::info!(
        signups,
        sessions,
        "purged old sign-ups and expired sessions"
    );
    Ok(())
}

async fn daily(state: &AppState) -> Result<(), sqlx::Error> {
    let mut conn = state.db.acquire().await?;
    let names = purge_deleted_accounts(&mut conn, OffsetDateTime::now_utc()).await?;
    tracing::info!(names, "freed the names of deleted accounts");
    Ok(())
}

/// Runs the purges every hour and every day, the first ones at once.
pub fn spawn(state: AppState) {
    let hourly_state = state.clone();
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(Duration::from_secs(60 * 60));
        loop {
            ticks.tick().await;
            if let Err(error) = hourly(&hourly_state).await {
                tracing::warn!(%error, "the hourly purge failed");
            }
        }
    });
    tokio::spawn(async move {
        let mut ticks = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        loop {
            ticks.tick().await;
            if let Err(error) = daily(&state).await {
                tracing::warn!(%error, "the daily purge failed");
            }
        }
    });
}
