//! Sessions: bearer tokens of 32 random bytes, of which the hub keeps only the SHA-256.

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::{rngs::OsRng, RngCore};
use rustcraft_protocol::api::ErrorCode;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{error::AppError, AppState};

/// How long a session lives after its last use, in days.
pub const SESSION_DAYS: i32 = 90;

/// The session of a request, from its `Authorization: Bearer <token>` header. Using it extends
/// it by [`SESSION_DAYS`].
#[derive(Clone, Copy, Debug, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub account_id: Uuid,
}

impl FromRequestParts<AppState> for Session {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let unauthorized = || AppError::new(ErrorCode::Unauthorized, "Log in first.");
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .and_then(|token| URL_SAFE_NO_PAD.decode(token.trim()).ok())
            .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
            .ok_or_else(unauthorized)?;
        sqlx::query_as(
            "UPDATE sessions s SET last_used_at = now(), \
                 expires_at = now() + make_interval(days => $2) \
             FROM accounts a \
             WHERE s.token_hash = $1 AND s.expires_at > now() \
                 AND a.id = s.account_id AND a.status = 'active' \
             RETURNING s.id, s.account_id",
        )
        .bind(&token_hash(&token)[..])
        .bind(SESSION_DAYS)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(unauthorized)
    }
}

fn token_hash(token: &[u8; 32]) -> [u8; 32] {
    Sha256::digest(token).into()
}

/// Opens a session and returns its token, base64url without padding.
pub async fn create(
    conn: &mut PgConnection,
    account_id: Uuid,
    device_name: Option<&str>,
) -> Result<String, sqlx::Error> {
    let mut token = [0u8; 32];
    OsRng.fill_bytes(&mut token);
    sqlx::query(
        "INSERT INTO sessions (id, account_id, token_hash, device_name, expires_at) \
         VALUES ($1, $2, $3, $4, now() + make_interval(days => $5))",
    )
    .bind(Uuid::new_v4())
    .bind(account_id)
    .bind(&token_hash(&token)[..])
    .bind(device_name)
    .bind(SESSION_DAYS)
    .execute(conn)
    .await?;
    Ok(URL_SAFE_NO_PAD.encode(token))
}

/// Ends every session of the account but `keep`, and revokes all its certificates.
pub async fn revoke_access(
    conn: &mut PgConnection,
    account_id: Uuid,
    keep: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE account_id = $1 AND id IS DISTINCT FROM $2")
        .bind(account_id)
        .bind(keep)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "UPDATE certificates SET revoked_at = now() WHERE account_id = $1 AND revoked_at IS NULL",
    )
    .bind(account_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Deletes the expired sessions.
pub async fn purge_expired(conn: &mut PgConnection) -> Result<u64, sqlx::Error> {
    let done = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(conn)
        .await?;
    Ok(done.rows_affected())
}
