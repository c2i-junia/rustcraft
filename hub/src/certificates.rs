//! Device certificates, the list of revoked ones, and public profiles.

use std::time::Instant;

use axum::{
    extract::{rejection::QueryRejection, Query, State},
    routing::{get, post},
    Json, Router,
};
use ed25519_dalek::VerifyingKey;
use rustcraft_protocol::{
    api::{CertificateRequest, CertificateResponse, ErrorCode, Profile, Revoked},
    identity::{key_from_hex, sign, Certificate, CERTIFICATE_LIFETIME_SECS, CERTIFICATE_VERSION},
};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{audit, error::AppError, routes::ApiJson, sessions::Session, AppState, SIGNING_KEY_ID};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/certificates", post(issue))
        .route("/v1/revoked", get(revoked))
        .route("/v1/profiles", get(profile))
}

/// Signs a certificate of 7 days for the device key, and keeps its id and dates so that it can
/// be revoked.
async fn issue(
    State(state): State<AppState>,
    session: Session,
    ApiJson(request): ApiJson<CertificateRequest>,
) -> Result<Json<CertificateResponse>, AppError> {
    let invalid = || {
        AppError::new(
            ErrorCode::InvalidRequest,
            "The device key must be an Ed25519 public key of 64 hex characters.",
        )
    };
    let device_key = key_from_hex(&request.device_key).ok_or_else(invalid)?;
    VerifyingKey::from_bytes(&device_key).map_err(|_| invalid())?;
    state
        .certificates
        .issue(session.account_id, Instant::now())?;

    let issued_at = OffsetDateTime::now_utc().unix_timestamp();
    let expires_at = issued_at + CERTIFICATE_LIFETIME_SECS as i64;
    let id = Uuid::new_v4();
    let mut tx = state.db.begin().await?;
    // Only for an active account, with its row locked: a suspension that came after the session
    // was checked has revoked the certificates already, and must not be outrun by a new one.
    let name: Option<String> = sqlx::query_scalar(
        "WITH account AS (SELECT id, display_name FROM accounts \
             WHERE id = $2 AND status = 'active' FOR SHARE), \
         inserted AS (INSERT INTO certificates (id, account_id, device_key, issued_at, expires_at) \
             SELECT $1, id, $3, to_timestamp($4), to_timestamp($5) FROM account) \
         SELECT display_name FROM account",
    )
    .bind(id)
    .bind(session.account_id)
    .bind(&device_key[..])
    .bind(issued_at as f64)
    .bind(expires_at as f64)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(name) = name else {
        return Err(AppError::new(ErrorCode::Unauthorized, "Log in first."));
    };
    audit::record(
        &mut tx,
        audit::SELF,
        "certificate_issued",
        Some(session.account_id),
        json!({"certificate": id}),
    )
    .await?;
    tx.commit().await?;

    let certificate = Certificate {
        version: CERTIFICATE_VERSION,
        hub: state.public_url.to_string(),
        id,
        account: session.account_id,
        name,
        device_key,
        issued_at: issued_at as u64,
        expires_at: expires_at as u64,
        hub_key_id: SIGNING_KEY_ID,
    };
    Ok(Json(CertificateResponse::new(
        &sign(&certificate, &state.signing_key),
        expires_at as u64,
    )))
}

/// The ids of the revoked certificates that have not expired: a server need not remember the
/// others.
async fn revoked(State(state): State<AppState>) -> Result<Json<Revoked>, AppError> {
    let ids = sqlx::query_scalar(
        "SELECT id FROM certificates WHERE revoked_at IS NOT NULL AND expires_at > now() \
         ORDER BY id",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Revoked { ids }))
}

#[derive(Deserialize)]
struct ProfileQuery {
    name: String,
}

async fn profile(
    State(state): State<AppState>,
    query: Result<Query<ProfileQuery>, QueryRejection>,
) -> Result<Json<Profile>, AppError> {
    let Query(ProfileQuery { name }) =
        query.map_err(|_| AppError::new(ErrorCode::InvalidRequest, "Give the name to look up."))?;
    let found: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, username::text FROM accounts WHERE username = $1::citext AND status = 'active'",
    )
    .bind(&name)
    .fetch_optional(&state.db)
    .await?;
    let (id, username) =
        found.ok_or_else(|| AppError::new(ErrorCode::NotFound, "No player has that name."))?;
    Ok(Json(Profile { id, username }))
}
