//! The account routes: sign-up, login and logout, recovery, and the account itself.

use std::{net::IpAddr, time::Instant};

use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::{delete, post, put},
    Json, Router,
};
use rustcraft_protocol::api::{
    AccountSummary, ChangePasswordRequest, DeleteAccountRequest, ErrorCode, LoginRequest, Me,
    RecoveryRequest, Role, SessionResponse, SessionWithRecovery, SignUpRequest,
};
use serde_json::json;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit,
    error::AppError,
    password::{check_password, check_username, new_recovery_code, normalize_recovery_code},
    ratelimit::{check_signup, ip_key, record_signup, ClientIp},
    routes::ApiJson,
    sessions::{self, revoke_access, Session},
    AppState,
};

/// The longest device name, in `char`s.
const DEVICE_NAME_LENGTH: usize = 64;
/// The advisory lock sign-ups take while they check the limits and write their row.
const SIGNUP_LOCK: i64 = 0x5349_474e_5550;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/accounts", post(sign_up))
        .route("/v1/sessions", post(log_in).delete(log_out_everywhere))
        .route("/v1/sessions/current", delete(log_out))
        .route("/v1/recovery", post(recover))
        .route("/v1/me", axum::routing::get(me).delete(delete_account))
        .route("/v1/me/password", put(change_password))
        .layer(DefaultBodyLimit::max(16 * 1024))
}

fn bad_credentials() -> AppError {
    AppError::new(
        ErrorCode::BadCredentials,
        "The username or the password is wrong.",
    )
}

fn username_taken() -> AppError {
    AppError::new(ErrorCode::UsernameTaken, "That username is taken.")
}

fn device_name(name: Option<String>) -> Result<Option<String>, AppError> {
    let name = name
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty());
    if name
        .as_ref()
        .is_some_and(|name| name.chars().count() > DEVICE_NAME_LENGTH)
    {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "A device name has at most 64 characters.",
        ));
    }
    Ok(name)
}

/// An active account and its hashes.
#[derive(sqlx::FromRow)]
struct Credentials {
    id: Uuid,
    username: String,
    password_hash: String,
    recovery_hash: String,
}

const CREDENTIALS: &str = "SELECT a.id, a.username::text AS username, p.password_hash, \
     p.recovery_hash FROM accounts a JOIN password_credentials p ON p.account_id = a.id \
     WHERE a.status = 'active'";

async fn credentials_by_name(
    db: &PgPool,
    username: &str,
) -> Result<Option<Credentials>, sqlx::Error> {
    sqlx::query_as(&format!("{CREDENTIALS} AND a.username = $1::citext"))
        .bind(username)
        .fetch_optional(db)
        .await
}

async fn credentials_by_id(db: &PgPool, id: Uuid) -> Result<Credentials, AppError> {
    sqlx::query_as(&format!("{CREDENTIALS} AND a.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::new(ErrorCode::Unauthorized, "Log in first."))
}

/// Which secret of the account a request gives.
#[derive(Clone, Copy)]
enum Secret {
    Password,
    RecoveryCode,
}

/// Checks `given` against the account's secret, within the limits on failed logins. The attempt
/// counts towards them until it succeeds; a failure is written in the audit log.
async fn authenticate(
    state: &AppState,
    ip: IpAddr,
    username: &str,
    given: String,
    account: Option<Credentials>,
    secret: Secret,
) -> Result<Credentials, AppError> {
    let attempt = state.logins.attempt(username, ip, Instant::now())?;
    let hash = account.as_ref().map(|account| match secret {
        Secret::Password => account.password_hash.clone(),
        Secret::RecoveryCode => account.recovery_hash.clone(),
    });
    let account = match (state.hasher.verify(given, hash).await?, account) {
        (true, Some(account)) => {
            state.logins.forgive(attempt);
            return Ok(account);
        }
        (_, account) => account,
    };
    let method = match secret {
        Secret::Password => "password",
        Secret::RecoveryCode => "recovery_code",
    };
    audit::record(
        &mut *state.db.acquire().await?,
        audit::ANONYMOUS,
        "login_failed",
        account.map(|account| account.id),
        json!({"username": username, "method": method}),
    )
    .await?;
    Err(bad_credentials())
}

async fn sign_up(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ApiJson(request): ApiJson<SignUpRequest>,
) -> Result<(StatusCode, Json<SessionWithRecovery>), AppError> {
    check_username(&request.username)?;
    check_password(&request.password, &request.username)?;
    let device_name = device_name(request.device_name)?;
    let ip_key = ip_key(&state.ip_secret, ip);
    let trusted = state.trusted_ranges.iter().any(|range| range.contains(&ip));
    // Refused early, before the costly hashes, then checked again under the lock.
    let room = check_signup(
        &mut *state.db.acquire().await?,
        &ip_key,
        trusted,
        OffsetDateTime::now_utc(),
    )
    .await?;
    let _slot = state.signups.enter(ip_key, room)?;
    let taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM accounts WHERE username = $1::citext)")
            .bind(&request.username)
            .fetch_one(&state.db)
            .await?;
    if taken {
        return Err(username_taken());
    }
    let recovery = new_recovery_code();
    let password_hash = state.hasher.hash(request.password).await?;
    let recovery_hash = state.hasher.hash(recovery.body).await?;

    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(SIGNUP_LOCK)
        .execute(&mut *tx)
        .await?;
    let now = OffsetDateTime::now_utc();
    check_signup(&mut tx, &ip_key, trusted, now).await?;
    let id = Uuid::new_v4();
    let inserted = sqlx::query(
        "INSERT INTO accounts (id, username, display_name) VALUES ($1, $2, $2) \
         ON CONFLICT (username) DO NOTHING",
    )
    .bind(id)
    .bind(&request.username)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(username_taken());
    }
    sqlx::query(
        "INSERT INTO account_identities (account_id, provider, subject) VALUES ($1, 'password', $2)",
    )
    .bind(id)
    .bind(id.to_string())
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO password_credentials (account_id, password_hash, recovery_hash) \
         VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(password_hash)
    .bind(recovery_hash)
    .execute(&mut *tx)
    .await?;
    record_signup(&mut tx, &ip_key, now).await?;
    let session = sessions::create(&mut tx, id, device_name.as_deref()).await?;
    audit::record(&mut tx, audit::SELF, "signup", Some(id), json!({})).await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(SessionWithRecovery {
            account: AccountSummary {
                id,
                username: request.username,
            },
            session,
            recovery_code: recovery.display,
        }),
    ))
}

async fn log_in(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ApiJson(request): ApiJson<LoginRequest>,
) -> Result<Json<SessionResponse>, AppError> {
    if check_username(&request.username).is_err() {
        return Err(bad_credentials());
    }
    let device_name = device_name(request.device_name)?;
    let account = credentials_by_name(&state.db, &request.username).await?;
    let account = authenticate(
        &state,
        ip,
        &request.username,
        request.password,
        account,
        Secret::Password,
    )
    .await?;
    let mut tx = state.db.begin().await?;
    let session = sessions::create(&mut tx, account.id, device_name.as_deref()).await?;
    audit::record(&mut tx, audit::SELF, "login", Some(account.id), json!({})).await?;
    tx.commit().await?;
    Ok(Json(SessionResponse {
        account: AccountSummary {
            id: account.id,
            username: account.username,
        },
        session,
    }))
}

async fn log_out(State(state): State<AppState>, session: Session) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM sessions WHERE id = $1")
        .bind(session.id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn log_out_everywhere(
    State(state): State<AppState>,
    session: Session,
) -> Result<StatusCode, AppError> {
    sqlx::query("DELETE FROM sessions WHERE account_id = $1")
        .bind(session.account_id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn recover(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    ApiJson(request): ApiJson<RecoveryRequest>,
) -> Result<Json<SessionWithRecovery>, AppError> {
    if check_username(&request.username).is_err() {
        return Err(bad_credentials());
    }
    check_password(&request.new_password, &request.username)?;
    let account = credentials_by_name(&state.db, &request.username).await?;
    let code = normalize_recovery_code(&request.recovery_code).unwrap_or_default();
    let account = authenticate(
        &state,
        ip,
        &request.username,
        code,
        account,
        Secret::RecoveryCode,
    )
    .await?;
    let recovery = new_recovery_code();
    let password_hash = state.hasher.hash(request.new_password).await?;
    let recovery_hash = state.hasher.hash(recovery.body).await?;

    let mut tx = state.db.begin().await?;
    // Only if the code is still the one checked: a code works once.
    let replaced = sqlx::query(
        "UPDATE password_credentials SET password_hash = $2, recovery_hash = $3, \
         updated_at = now() WHERE account_id = $1 AND recovery_hash = $4",
    )
    .bind(account.id)
    .bind(password_hash)
    .bind(recovery_hash)
    .bind(&account.recovery_hash)
    .execute(&mut *tx)
    .await?;
    if replaced.rows_affected() == 0 {
        return Err(bad_credentials());
    }
    revoke_access(&mut tx, account.id, None).await?;
    let session = sessions::create(&mut tx, account.id, None).await?;
    audit::record(
        &mut tx,
        audit::SELF,
        "recovery_used",
        Some(account.id),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(Json(SessionWithRecovery {
        account: AccountSummary {
            id: account.id,
            username: account.username,
        },
        session,
        recovery_code: recovery.display,
    }))
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: Uuid,
    username: String,
    realm_quota: i32,
    created_at: OffsetDateTime,
}

async fn me(State(state): State<AppState>, session: Session) -> Result<Json<Me>, AppError> {
    let account: AccountRow = sqlx::query_as(
        "SELECT id, username::text AS username, realm_quota, created_at FROM accounts \
         WHERE id = $1",
    )
    .bind(session.account_id)
    .fetch_one(&state.db)
    .await?;
    let roles: Vec<String> =
        sqlx::query_scalar("SELECT role FROM account_roles WHERE account_id = $1 ORDER BY role")
            .bind(session.account_id)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(Me {
        id: account.id,
        username: account.username,
        roles: roles
            .iter()
            .filter_map(|role| match role.as_str() {
                "admin" => Some(Role::Admin),
                "host" => Some(Role::Host),
                _ => None,
            })
            .collect(),
        realm_quota: account.realm_quota.try_into().unwrap_or(0),
        created_at: account.created_at.unix_timestamp().try_into().unwrap_or(0),
    }))
}

async fn change_password(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    ApiJson(request): ApiJson<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    let account = credentials_by_id(&state.db, session.account_id).await?;
    let username = account.username.clone();
    check_password(&request.new_password, &username)?;
    let account = authenticate(
        &state,
        ip,
        &username,
        request.current_password,
        Some(account),
        Secret::Password,
    )
    .await?;
    let password_hash = state.hasher.hash(request.new_password).await?;
    let mut tx = state.db.begin().await?;
    // Only if the password is still the one checked.
    let replaced = sqlx::query(
        "UPDATE password_credentials SET password_hash = $2, updated_at = now() \
         WHERE account_id = $1 AND password_hash = $3",
    )
    .bind(account.id)
    .bind(password_hash)
    .bind(&account.password_hash)
    .execute(&mut *tx)
    .await?;
    if replaced.rows_affected() == 0 {
        return Err(bad_credentials());
    }
    revoke_access(&mut tx, account.id, Some(session.id)).await?;
    audit::record(
        &mut tx,
        audit::SELF,
        "password_changed",
        Some(account.id),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Marks the account deleted and removes its ways in. The row stays, and the name with it,
/// until [`crate::maintenance::purge_deleted_accounts`] frees the name 30 days later.
async fn delete_account(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    session: Session,
    ApiJson(request): ApiJson<DeleteAccountRequest>,
) -> Result<StatusCode, AppError> {
    let account = credentials_by_id(&state.db, session.account_id).await?;
    let username = account.username.clone();
    let account = authenticate(
        &state,
        ip,
        &username,
        request.password,
        Some(account),
        Secret::Password,
    )
    .await?;
    let mut tx = state.db.begin().await?;
    sqlx::query("UPDATE accounts SET status = 'deleted', deleted_at = now() WHERE id = $1")
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM password_credentials WHERE account_id = $1")
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM account_identities WHERE account_id = $1")
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
    revoke_access(&mut tx, account.id, None).await?;
    audit::record(
        &mut tx,
        audit::SELF,
        "account_deleted",
        Some(account.id),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
