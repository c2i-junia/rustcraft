//! Sessions: tokens of 32 random bytes, of which the hub keeps only the SHA-256. A session of
//! kind `game` is a bearer token of the game client; one of kind `admin_web` is the cookie of
//! the web admin. Neither is accepted in place of the other.

use std::fmt;

use axum::{
    extract::FromRequestParts,
    http::{
        header::{AUTHORIZATION, COOKIE},
        request::Parts,
        HeaderMap,
    },
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

/// How long a web admin session lives after its last use, in hours.
pub const ADMIN_IDLE_HOURS: i32 = 12;
/// How long a web admin session lives at most from its creation, in hours.
pub const ADMIN_LIFETIME_HOURS: i32 = 24;
/// The cookie of the web admin. `__Host-` makes browsers take it only when it is `Secure`, has
/// `Path=/` and no `Domain`, so that no other site, subdomains included, can set it.
pub const ADMIN_COOKIE: &str = "__Host-rustcraft-admin";

/// The game session of a request, from its `Authorization: Bearer <token>` header. Using it
/// extends it by [`SESSION_DAYS`].
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
        let session = sqlx::query_as(
            "UPDATE sessions s SET last_used_at = now(), \
                 expires_at = now() + make_interval(days => $2) \
             FROM accounts a \
             WHERE s.token_hash = $1 AND s.kind = 'game' AND s.expires_at > now() \
                 AND a.id = s.account_id AND a.status = 'active' \
             RETURNING s.id, s.account_id",
        )
        .bind(&token_hash(&token)[..])
        .bind(SESSION_DAYS)
        .fetch_optional(&state.db)
        .await?;
        if let Some(session) = session {
            return Ok(session);
        }
        // Not extended, but a suspended account is told why instead of "log in first".
        let suspended: Option<Option<String>> = sqlx::query_scalar(
            "SELECT a.status_reason FROM sessions s JOIN accounts a ON a.id = s.account_id \
             WHERE s.token_hash = $1 AND s.kind = 'game' AND s.expires_at > now() \
                 AND a.status = 'suspended'",
        )
        .bind(&token_hash(&token)[..])
        .fetch_optional(&state.db)
        .await?;
        Err(match suspended {
            Some(reason) => suspended_error(reason.as_deref()),
            None => unauthorized(),
        })
    }
}

/// The refusal of a suspended account, with the reason the admin gave.
pub fn suspended_error(reason: Option<&str>) -> AppError {
    AppError::new(
        ErrorCode::Forbidden,
        match reason {
            Some(reason) => format!("This account is suspended: {reason}"),
            None => "This account is suspended.".to_owned(),
        },
    )
}

fn token_hash(token: &[u8; 32]) -> [u8; 32] {
    Sha256::digest(token).into()
}

/// The session of the web admin, from its [`ADMIN_COOKIE`], of an active account that holds
/// the `admin` role at the time of the request. Using it extends it by [`ADMIN_IDLE_HOURS`],
/// never beyond [`ADMIN_LIFETIME_HOURS`] after its creation.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AdminSession {
    pub id: Uuid,
    pub account_id: Uuid,
    pub username: String,
}

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, AppError> {
        let unauthorized = || AppError::new(ErrorCode::Unauthorized, "Log in first.");
        let token = admin_cookie(&parts.headers).ok_or_else(unauthorized)?;
        let hash = token_hash(&token);
        let session = sqlx::query_as(
            "UPDATE sessions s SET last_used_at = now(), \
                 expires_at = LEAST(now() + make_interval(hours => $2), \
                     s.created_at + make_interval(hours => $3)) \
             FROM accounts a \
             WHERE s.token_hash = $1 AND s.kind = 'admin_web' AND s.expires_at > now() \
                 AND s.created_at + make_interval(hours => $3) > now() \
                 AND a.id = s.account_id AND a.status = 'active' \
                 AND EXISTS (SELECT 1 FROM account_roles r \
                     WHERE r.account_id = a.id AND r.role = 'admin') \
             RETURNING s.id, s.account_id, a.username::text AS username",
        )
        .bind(&hash[..])
        .bind(ADMIN_IDLE_HOURS)
        .bind(ADMIN_LIFETIME_HOURS)
        .fetch_optional(&state.db)
        .await?;
        if let Some(session) = session {
            return Ok(session);
        }
        // A live session whose account lost the role is told so, and is not extended.
        let demoted: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM sessions s JOIN accounts a ON a.id = s.account_id \
             WHERE s.token_hash = $1 AND s.kind = 'admin_web' AND s.expires_at > now() \
                 AND s.created_at + make_interval(hours => $2) > now() \
                 AND a.status = 'active')",
        )
        .bind(&hash[..])
        .bind(ADMIN_LIFETIME_HOURS)
        .fetch_one(&state.db)
        .await?;
        Err(if demoted {
            not_an_admin()
        } else {
            unauthorized()
        })
    }
}

/// The refusal of an account without the `admin` role.
pub fn not_an_admin() -> AppError {
    AppError::new(ErrorCode::Forbidden, "This account is not an admin.")
}

/// The token of the request's [`ADMIN_COOKIE`], when it has a well-formed one.
pub fn admin_cookie(headers: &HeaderMap) -> Option<[u8; 32]> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == ADMIN_COOKIE)
        .and_then(|(_, token)| URL_SAFE_NO_PAD.decode(token.trim()).ok())
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
}

/// A session token, base64url without padding. Its `Debug` does not show it.
pub struct Token(pub String);

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(<redacted>)")
    }
}

/// Opens a game session and returns its token, base64url without padding.
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

/// Opens a web admin session of [`ADMIN_IDLE_HOURS`] and returns its token.
pub async fn create_admin(conn: &mut PgConnection, account_id: Uuid) -> Result<Token, sqlx::Error> {
    let mut token = [0u8; 32];
    OsRng.fill_bytes(&mut token);
    sqlx::query(
        "INSERT INTO sessions (id, account_id, token_hash, kind, expires_at) \
         VALUES ($1, $2, $3, 'admin_web', now() + make_interval(hours => $4))",
    )
    .bind(Uuid::new_v4())
    .bind(account_id)
    .bind(&token_hash(&token)[..])
    .bind(ADMIN_IDLE_HOURS)
    .execute(conn)
    .await?;
    Ok(Token(URL_SAFE_NO_PAD.encode(token)))
}

/// Deletes the web admin session of `token`, and returns its account id and username.
pub async fn delete_admin(
    conn: &mut PgConnection,
    token: &[u8; 32],
) -> Result<Option<(Uuid, String)>, sqlx::Error> {
    sqlx::query_as(
        "DELETE FROM sessions s USING accounts a \
         WHERE s.token_hash = $1 AND s.kind = 'admin_web' AND a.id = s.account_id \
         RETURNING s.account_id, a.username::text",
    )
    .bind(&token_hash(token)[..])
    .fetch_optional(conn)
    .await
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
