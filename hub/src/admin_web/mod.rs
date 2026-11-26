//! The web admin: its API under `/v1/admin`, for an [`AdminSession`] cookie. The API does what
//! [`crate::admin`] does for the command line, as the admin of the session.
//!
//! Every API request goes through [`guard`]: a request from another site is refused, and any
//! request but a GET or HEAD must carry `X-Rustcraft-Admin: 1`, which a page of another origin
//! cannot add without a CORS preflight that the hub never answers.

use std::{fmt, sync::Arc};

use axum::{
    extract::{
        rejection::QueryRejection, DefaultBodyLimit, FromRequestParts, Path, Query, Request, State,
    },
    http::{
        header::{CACHE_CONTROL, SET_COOKIE, X_CONTENT_TYPE_OPTIONS},
        request::Parts,
        HeaderMap, HeaderValue, Method, StatusCode, Uri,
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
    Json, Router,
};
use rustcraft_protocol::api::ErrorCode;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    accounts::{authenticate, bad_credentials, credentials_by_name, Secret},
    admin::{self, Actor, AdminError, Target},
    audit,
    config::RoleArg,
    error::AppError,
    password::check_username,
    ratelimit::ClientIp,
    routes::ApiJson,
    sessions::{
        self, admin_cookie, not_an_admin, AdminSession, ADMIN_COOKIE, ADMIN_LIFETIME_HOURS,
    },
    AppState,
};

/// The header every API request but a GET or HEAD carries, with the value `1`.
pub const ADMIN_HEADER: &str = "x-rustcraft-admin";
/// The most accounts a page of `GET /v1/admin/accounts` holds, and how many by default.
const MAX_ACCOUNTS: i64 = 100;
const DEFAULT_ACCOUNTS: i64 = 50;
/// The most audit rows a page of `GET /v1/admin/audit` holds, and how many by default.
const MAX_AUDIT: i64 = 200;
const DEFAULT_AUDIT: i64 = 50;
/// The audit rows of an account's detail.
const ACCOUNT_AUDIT: i64 = 50;
/// The longest search of accounts, and the longest action filter, in `char`s.
const FILTER_LENGTH: usize = 64;
/// The longest reason of a suspension, in `char`s.
const REASON_LENGTH: usize = 200;

pub fn routes(state: &AppState) -> Router<AppState> {
    let origin: Arc<str> = origin(&state.public_url).into();
    Router::new()
        .route("/v1/admin/session", post(log_in).delete(log_out))
        .route("/v1/admin/me", get(me))
        .route("/v1/admin/overview", get(overview))
        .route("/v1/admin/accounts", get(accounts))
        .route("/v1/admin/accounts/{id}", get(account))
        .route(
            "/v1/admin/accounts/{id}/roles/{role}",
            put(grant_role).delete(revoke_role),
        )
        .route("/v1/admin/accounts/{id}/quota", put(set_quota))
        .route("/v1/admin/accounts/{id}/suspend", post(suspend))
        .route("/v1/admin/accounts/{id}/unsuspend", post(unsuspend))
        .route(
            "/v1/admin/accounts/{id}/reset-password",
            post(reset_password),
        )
        .route("/v1/admin/accounts/{id}/sessions", delete(end_sessions))
        .route("/v1/admin/realms", get(realms))
        .route("/v1/admin/realms/{id}", delete(archive_realm))
        .route("/v1/admin/realms/{id}/stop", post(stop_realm))
        .route("/v1/admin/audit", get(audit_log))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(origin, guard))
}

/// The origin of a URL, as a browser sends it in `Origin`: `scheme://host[:port]`, lowercase,
/// without the default port.
pub fn origin(url: &str) -> String {
    let Ok(uri) = url.parse::<Uri>() else {
        return url.to_ascii_lowercase();
    };
    let scheme = uri.scheme_str().unwrap_or("https").to_ascii_lowercase();
    let host = uri.host().unwrap_or_default().to_ascii_lowercase();
    match uri.port_u16() {
        Some(443) if scheme == "https" => format!("{scheme}://{host}"),
        Some(80) if scheme == "http" => format!("{scheme}://{host}"),
        Some(port) => format!("{scheme}://{host}:{port}"),
        None => format!("{scheme}://{host}"),
    }
}

/// Refuses what may come from another site, then marks the response as not to be cached nor
/// sniffed.
async fn guard(State(origin): State<Arc<str>>, request: Request, next: Next) -> Response {
    let mut response = match check_request(&origin, request.method(), request.headers()) {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    };
    let headers = response.headers_mut();
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    response
}

fn check_request(origin: &str, method: &Method, headers: &HeaderMap) -> Result<(), AppError> {
    let foreign = || {
        AppError::new(
            ErrorCode::Forbidden,
            "This request comes from another site.",
        )
    };
    if let Some(given) = headers.get("origin") {
        if !given.as_bytes().eq_ignore_ascii_case(origin.as_bytes()) {
            return Err(foreign());
        }
    }
    if let Some(site) = headers.get("sec-fetch-site") {
        if !matches!(site.as_bytes(), b"same-origin" | b"none") {
            return Err(foreign());
        }
    }
    if method != Method::GET
        && method != Method::HEAD
        && headers.get(ADMIN_HEADER).map(HeaderValue::as_bytes) != Some(b"1")
    {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "This request lacks the X-Rustcraft-Admin header.",
        ));
    }
    Ok(())
}

/// The query string, refused with an `invalid_request` error when it does not parse.
struct ApiQuery<T>(T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequestParts<S> for ApiQuery<T> {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        let query: Result<Query<T>, QueryRejection> = Query::from_request_parts(parts, state).await;
        match query {
            Ok(Query(value)) => Ok(Self(value)),
            Err(rejection) => Err(AppError::new(
                ErrorCode::InvalidRequest,
                rejection.body_text(),
            )),
        }
    }
}

impl From<AdminError> for AppError {
    fn from(error: AdminError) -> Self {
        match error {
            AdminError::NotFound(message) => AppError::new(ErrorCode::NotFound, message),
            AdminError::Invalid(message) => AppError::new(ErrorCode::InvalidRequest, message),
            AdminError::Refused(message) => AppError::new(ErrorCode::Forbidden, message),
            AdminError::Internal(error) => {
                tracing::error!(%error, "an admin operation failed");
                AppError::new(ErrorCode::Internal, "Something went wrong on the hub.")
            }
        }
    }
}

fn actor(session: &AdminSession) -> Actor {
    Actor::web(session.account_id, &session.username)
}

fn unix(at: OffsetDateTime) -> i64 {
    at.unix_timestamp()
}

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::InvalidRequest, message)
}

fn account_id(text: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(text)
        .map_err(|_| AppError::new(ErrorCode::NotFound, "There is no such account."))
}

fn realm_id(text: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(text).map_err(|_| AppError::new(ErrorCode::NotFound, "There is no such world."))
}

fn parse_role(text: &str) -> Result<RoleArg, AppError> {
    match text {
        "admin" => Ok(RoleArg::Admin),
        "host" => Ok(RoleArg::Host),
        _ => Err(AppError::new(ErrorCode::NotFound, "There is no such role.")),
    }
}

/// A page of `limit` rows, `default` when not given, refused beyond `1..=max`.
fn limit(given: Option<i64>, default: i64, max: i64) -> Result<i64, AppError> {
    let limit = given.unwrap_or(default);
    if !(1..=max).contains(&limit) {
        return Err(invalid(format!("The limit is 1 to {max}.")));
    }
    Ok(limit)
}

/// A filter of the query string: an empty one is no filter.
fn filter(given: Option<String>) -> Result<Option<String>, AppError> {
    let given = given.map(|text| text.trim().to_owned());
    match given {
        Some(text) if text.is_empty() => Ok(None),
        Some(text) if text.chars().count() > FILTER_LENGTH => Err(invalid(format!(
            "A filter has at most {FILTER_LENGTH} characters."
        ))),
        other => Ok(other),
    }
}

fn cookie(value: &str, max_age: i64) -> HeaderValue {
    HeaderValue::try_from(format!(
        "{ADMIN_COOKIE}={value}; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age={max_age}"
    ))
    .expect("a base64url token is a valid header value")
}

/// `POST /v1/admin/session`
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LoginRequest {
    username: String,
    password: String,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    username: String,
}

/// Logs in an admin with their password, through the limits on failed logins of the game's
/// login, and sets the session cookie. A correct password of an account without the `admin`
/// role is refused, and written in the audit log as `admin_login_refused`.
async fn log_in(
    State(state): State<AppState>,
    ClientIp(ip): ClientIp,
    headers: HeaderMap,
    ApiJson(request): ApiJson<LoginRequest>,
) -> Result<(HeaderMap, Json<LoginResponse>), AppError> {
    if check_username(&request.username).is_err() {
        return Err(bad_credentials());
    }
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
    let admin: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM account_roles WHERE account_id = $1 AND role = 'admin')",
    )
    .bind(account.id)
    .fetch_one(&mut *tx)
    .await?;
    if !admin {
        audit::record(
            &mut tx,
            audit::SELF,
            "admin_login_refused",
            Some(account.id),
            json!({}),
        )
        .await?;
        tx.commit().await?;
        return Err(not_an_admin());
    }
    // The session the browser held before, if any, ends: a login always starts a new one.
    if let Some(old) = admin_cookie(&headers) {
        sessions::delete_admin(&mut tx, &old).await?;
    }
    let token = sessions::create_admin(&mut tx, account.id).await?;
    Actor::web(account.id, &account.username)
        .record(&mut tx, "admin_login", Some(account.id), json!({}))
        .await?;
    tx.commit().await?;
    let mut response = HeaderMap::new();
    response.insert(
        SET_COOKIE,
        cookie(&token.0, i64::from(ADMIN_LIFETIME_HOURS) * 3600),
    );
    Ok((
        response,
        Json(LoginResponse {
            username: account.username,
        }),
    ))
}

/// Ends the session of the cookie, whatever its account has become, and clears the cookie.
async fn log_out(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap), AppError> {
    if let Some(token) = admin_cookie(&headers) {
        let mut tx = state.db.begin().await?;
        if let Some((id, username)) = sessions::delete_admin(&mut tx, &token).await? {
            Actor::web(id, &username)
                .record(&mut tx, "admin_logout", Some(id), json!({}))
                .await?;
        }
        tx.commit().await?;
    }
    let mut response = HeaderMap::new();
    response.insert(SET_COOKIE, cookie("", 0));
    Ok((StatusCode::NO_CONTENT, response))
}

#[derive(Debug, Serialize)]
struct MeResponse {
    id: Uuid,
    username: String,
}

async fn me(session: AdminSession) -> Json<MeResponse> {
    Json(MeResponse {
        id: session.account_id,
        username: session.username,
    })
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct AccountCounts {
    active: i64,
    suspended: i64,
    deleted: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SessionCounts {
    game: i64,
    admin_web: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
struct SignupCounts {
    last_24h: i64,
    last_7d: i64,
}

#[derive(Debug, Default, Serialize)]
struct RealmCounts {
    stopped: i64,
    starting: i64,
    running: i64,
    stopping: i64,
    failed: i64,
    archived: i64,
}

#[derive(Debug, Serialize)]
struct Overview {
    accounts: AccountCounts,
    /// Accounts not deleted that hold the role.
    admins: i64,
    hosts: i64,
    /// Sessions not expired, by kind.
    sessions: SessionCounts,
    signups: SignupCounts,
    failed_logins_24h: i64,
    realms: RealmCounts,
    /// The worlds that hold a slot, which `max_running` bounds.
    running: i64,
    max_running: u16,
}

async fn overview(
    State(state): State<AppState>,
    _session: AdminSession,
) -> Result<Json<Overview>, AppError> {
    let db = &state.db;
    let accounts = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'active') AS active, \
             count(*) FILTER (WHERE status = 'suspended') AS suspended, \
             count(*) FILTER (WHERE status = 'deleted') AS deleted \
         FROM accounts",
    )
    .fetch_one(db)
    .await?;
    let (admins, hosts): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE r.role = 'admin'), count(*) FILTER (WHERE r.role = 'host') \
         FROM account_roles r JOIN accounts a ON a.id = r.account_id \
         WHERE a.status <> 'deleted'",
    )
    .fetch_one(db)
    .await?;
    let sessions = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE kind = 'game') AS game, \
             count(*) FILTER (WHERE kind = 'admin_web') AS admin_web \
         FROM sessions WHERE expires_at > now()",
    )
    .fetch_one(db)
    .await?;
    let signups = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE created_at > now() - interval '24 hours') AS last_24h, \
             count(*) AS last_7d \
         FROM accounts WHERE created_at > now() - interval '7 days'",
    )
    .fetch_one(db)
    .await?;
    let failed_logins_24h: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_log \
         WHERE action = 'login_failed' AND at > now() - interval '24 hours'",
    )
    .fetch_one(db)
    .await?;
    let by_state: Vec<(String, i64)> =
        sqlx::query_as("SELECT state, count(*) FROM realms GROUP BY state")
            .fetch_all(db)
            .await?;
    let mut realms = RealmCounts::default();
    for (name, count) in by_state {
        match name.as_str() {
            "stopped" => realms.stopped = count,
            "starting" => realms.starting = count,
            "running" => realms.running = count,
            "stopping" => realms.stopping = count,
            "failed" => realms.failed = count,
            "archived" => realms.archived = count,
            _ => {}
        }
    }
    let running: i64 = sqlx::query_scalar("SELECT count(*) FROM realms WHERE slot IS NOT NULL")
        .fetch_one(db)
        .await?;
    Ok(Json(Overview {
        accounts,
        admins,
        hosts,
        sessions,
        signups,
        failed_logins_24h,
        realms,
        running,
        max_running: state.realms.max_running,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountsQuery {
    q: Option<String>,
    status: Option<String>,
    role: Option<String>,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AccountItem {
    id: Uuid,
    username: String,
    /// `active`, `suspended` or `deleted`.
    status: String,
    status_reason: Option<String>,
    /// `admin` and `host`, sorted.
    roles: Vec<String>,
    realm_quota: i32,
    created_at: i64,
}

#[derive(sqlx::FromRow)]
struct AccountRow {
    id: Uuid,
    username: String,
    status: String,
    status_reason: Option<String>,
    roles: Vec<String>,
    realm_quota: i32,
    created_at: OffsetDateTime,
    deleted_at: Option<OffsetDateTime>,
}

impl AccountRow {
    fn item(self) -> AccountItem {
        AccountItem {
            id: self.id,
            username: self.username,
            status: self.status,
            status_reason: self.status_reason,
            roles: self.roles,
            realm_quota: self.realm_quota,
            created_at: unix(self.created_at),
        }
    }
}

const ACCOUNT_ROW: &str = "SELECT a.id, a.username::text AS username, a.status, \
         a.status_reason, \
         ARRAY(SELECT r.role FROM account_roles r WHERE r.account_id = a.id ORDER BY r.role) \
             AS roles, \
         a.realm_quota, a.created_at, a.deleted_at \
     FROM accounts a";

/// The filter of [`accounts`]: `$1` a pattern of `ILIKE`, `$2` the status, `$3` the role.
const ACCOUNTS_FILTER: &str = "WHERE ($1::text IS NULL OR a.username::text ILIKE $1 ESCAPE '\\') \
     AND ($2::text IS NULL OR a.status = $2) \
     AND ($3::text IS NULL OR EXISTS (SELECT 1 FROM account_roles r \
         WHERE r.account_id = a.id AND r.role = $3))";

#[derive(Debug, Serialize)]
struct AccountPage {
    /// The accounts that match, on every page.
    total: i64,
    accounts: Vec<AccountItem>,
}

/// A page of the accounts, newest first.
async fn accounts(
    State(state): State<AppState>,
    _session: AdminSession,
    ApiQuery(query): ApiQuery<AccountsQuery>,
) -> Result<Json<AccountPage>, AppError> {
    let pattern = filter(query.q)?.map(|q| {
        let escaped = q
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        format!("%{escaped}%")
    });
    let status = filter(query.status)?;
    if status
        .as_deref()
        .is_some_and(|status| !matches!(status, "active" | "suspended" | "deleted"))
    {
        return Err(invalid("The status is active, suspended or deleted."));
    }
    let role = filter(query.role)?;
    if role
        .as_deref()
        .is_some_and(|role| !matches!(role, "admin" | "host"))
    {
        return Err(invalid("The role is admin or host."));
    }
    let limit = limit(query.limit, DEFAULT_ACCOUNTS, MAX_ACCOUNTS)?;
    let offset = query.offset.unwrap_or(0);
    if offset < 0 {
        return Err(invalid("The offset cannot be negative."));
    }
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM accounts a {ACCOUNTS_FILTER}"
    ))
    .bind(&pattern)
    .bind(&status)
    .bind(&role)
    .fetch_one(&state.db)
    .await?;
    let rows: Vec<AccountRow> = sqlx::query_as(&format!(
        "{ACCOUNT_ROW} {ACCOUNTS_FILTER} ORDER BY a.created_at DESC, a.id DESC \
         LIMIT $4 OFFSET $5"
    ))
    .bind(&pattern)
    .bind(&status)
    .bind(&role)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(AccountPage {
        total,
        accounts: rows.into_iter().map(AccountRow::item).collect(),
    }))
}

#[derive(Debug, Serialize)]
struct SessionItem {
    id: Uuid,
    /// `game` or `admin_web`.
    kind: String,
    device_name: Option<String>,
    created_at: i64,
    last_used_at: i64,
    expires_at: i64,
}

#[derive(Debug, Serialize)]
struct Owner {
    id: Uuid,
    username: String,
}

#[derive(Debug, Serialize)]
struct RealmItem {
    id: Uuid,
    name: String,
    owner: Owner,
    /// `stopped`, `starting`, `running`, `stopping`, `failed` or `archived`.
    state: String,
    slot: Option<i16>,
    /// The game port of the slot.
    port: Option<u16>,
    players: i32,
    last_played_at: Option<i64>,
    failed_reason: Option<String>,
    created_at: i64,
    state_changed_at: i64,
}

#[derive(sqlx::FromRow)]
struct RealmRow {
    id: Uuid,
    name: String,
    owner_id: Uuid,
    owner_name: String,
    state: String,
    slot: Option<i16>,
    players: i32,
    last_played_at: Option<OffsetDateTime>,
    failed_reason: Option<String>,
    created_at: OffsetDateTime,
    state_changed_at: OffsetDateTime,
}

const REALM_ROW: &str = "SELECT r.id, r.name, r.owner_id, o.username::text AS owner_name, \
         r.state, r.slot, r.players, r.last_played_at, r.failed_reason, r.created_at, \
         r.state_changed_at \
     FROM realms r JOIN accounts o ON o.id = r.owner_id";

fn realm_items(state: &AppState, rows: Vec<RealmRow>) -> Vec<RealmItem> {
    rows.into_iter()
        .map(|row| RealmItem {
            id: row.id,
            name: row.name,
            owner: Owner {
                id: row.owner_id,
                username: row.owner_name,
            },
            state: row.state,
            slot: row.slot,
            port: row.slot.map(|slot| state.realms.game_port(slot)),
            players: row.players,
            last_played_at: row.last_played_at.map(unix),
            failed_reason: row.failed_reason,
            created_at: unix(row.created_at),
            state_changed_at: unix(row.state_changed_at),
        })
        .collect()
}

#[derive(Debug, Serialize)]
struct AuditItem {
    id: i64,
    at: i64,
    actor: String,
    action: String,
    account_id: Option<Uuid>,
    /// The username of `account_id`.
    username: Option<String>,
    details: Value,
}

#[derive(sqlx::FromRow)]
struct AuditRow {
    id: i64,
    at: OffsetDateTime,
    actor: String,
    action: String,
    account_id: Option<Uuid>,
    username: Option<String>,
    details: Value,
}

impl AuditRow {
    fn item(self) -> AuditItem {
        AuditItem {
            id: self.id,
            at: unix(self.at),
            actor: self.actor,
            action: self.action,
            account_id: self.account_id,
            username: self.username,
            details: self.details,
        }
    }
}

const AUDIT_ROW: &str = "SELECT l.id, l.at, l.actor, l.action, l.account_id, \
         a.username::text AS username, l.details \
     FROM audit_log l LEFT JOIN accounts a ON a.id = l.account_id";

#[derive(Debug, Serialize)]
struct AccountDetail {
    #[serde(flatten)]
    account: AccountItem,
    deleted_at: Option<i64>,
    /// The sessions not expired, newest first.
    sessions: Vec<SessionItem>,
    /// The certificates neither revoked nor expired.
    active_certificates: i64,
    /// The worlds it owns, archived ones included, oldest first.
    realms: Vec<RealmItem>,
    /// Its last audit rows, newest first.
    audit: Vec<AuditItem>,
}

async fn account(
    State(state): State<AppState>,
    _session: AdminSession,
    Path(id): Path<String>,
) -> Result<Json<AccountDetail>, AppError> {
    let id = account_id(&id)?;
    let db = &state.db;
    let row: AccountRow = sqlx::query_as(&format!("{ACCOUNT_ROW} WHERE a.id = $1"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "There is no such account."))?;
    let deleted_at = row.deleted_at.map(unix);
    let session_rows: Vec<(
        Uuid,
        String,
        Option<String>,
        OffsetDateTime,
        OffsetDateTime,
        OffsetDateTime,
    )> = sqlx::query_as(
        "SELECT id, kind, device_name, created_at, last_used_at, expires_at FROM sessions \
             WHERE account_id = $1 AND expires_at > now() ORDER BY created_at DESC, id",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let sessions = session_rows
        .into_iter()
        .map(
            |(id, kind, device_name, created_at, last_used_at, expires_at)| SessionItem {
                id,
                kind,
                device_name,
                created_at: unix(created_at),
                last_used_at: unix(last_used_at),
                expires_at: unix(expires_at),
            },
        )
        .collect();
    let active_certificates: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM certificates \
         WHERE account_id = $1 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    let realms = sqlx::query_as(&format!(
        "{REALM_ROW} WHERE r.owner_id = $1 ORDER BY r.created_at, r.id"
    ))
    .bind(id)
    .fetch_all(db)
    .await?;
    let audit: Vec<AuditRow> = sqlx::query_as(&format!(
        "{AUDIT_ROW} WHERE l.account_id = $1 ORDER BY l.id DESC LIMIT $2"
    ))
    .bind(id)
    .bind(ACCOUNT_AUDIT)
    .fetch_all(db)
    .await?;
    Ok(Json(AccountDetail {
        account: row.item(),
        deleted_at,
        sessions,
        active_certificates,
        realms: realm_items(&state, realms),
        audit: audit.into_iter().map(AuditRow::item).collect(),
    }))
}

/// `PUT /v1/admin/accounts/{id}/roles/{role}`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRequest {
    /// Only with `host`: the quota of worlds, 1 when not given.
    #[serde(default)]
    realm_quota: Option<i32>,
}

async fn grant_role(
    State(state): State<AppState>,
    session: AdminSession,
    Path((id, role)): Path<(String, String)>,
    ApiJson(request): ApiJson<GrantRequest>,
) -> Result<StatusCode, AppError> {
    let target = Target::Id(account_id(&id)?);
    let role = parse_role(&role)?;
    admin::grant_role(
        &state.db,
        &actor(&session),
        target,
        role,
        request.realm_quota,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke_role(
    State(state): State<AppState>,
    session: AdminSession,
    Path((id, role)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let target = Target::Id(account_id(&id)?);
    let role = parse_role(&role)?;
    admin::revoke_role(&state.db, &actor(&session), target, role).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `PUT /v1/admin/accounts/{id}/quota`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaRequest {
    realm_quota: i32,
}

async fn set_quota(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    ApiJson(request): ApiJson<QuotaRequest>,
) -> Result<StatusCode, AppError> {
    let target = Target::Id(account_id(&id)?);
    admin::set_realm_quota(&state.db, &actor(&session), target, request.realm_quota).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /v1/admin/accounts/{id}/suspend`
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuspendRequest {
    reason: String,
}

async fn suspend(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
    ApiJson(request): ApiJson<SuspendRequest>,
) -> Result<StatusCode, AppError> {
    let target = Target::Id(account_id(&id)?);
    let length = request.reason.trim().chars().count();
    if !(1..=REASON_LENGTH).contains(&length) {
        return Err(invalid(format!(
            "The reason has 1 to {REASON_LENGTH} characters."
        )));
    }
    admin::suspend_account(&state.db, &actor(&session), target, &request.reason).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn unsuspend(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let target = Target::Id(account_id(&id)?);
    admin::unsuspend_account(&state.db, &actor(&session), target).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The temporary password of a reset, shown once. Its `Debug` does not show it.
#[derive(Serialize)]
struct ResetResponse {
    password: String,
}

impl fmt::Debug for ResetResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResetResponse")
            .field("password", &"<redacted>")
            .finish()
    }
}

async fn reset_password(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
) -> Result<Json<ResetResponse>, AppError> {
    let target = Target::Id(account_id(&id)?);
    let reset =
        admin::reset_account_password(&state.db, &state.hasher, &actor(&session), target).await?;
    Ok(Json(ResetResponse {
        password: reset.password,
    }))
}

async fn end_sessions(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let target = Target::Id(account_id(&id)?);
    admin::end_sessions(&state.db, &actor(&session), target).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RealmsQuery {
    state: Option<String>,
}

#[derive(Debug, Serialize)]
struct RealmPage {
    realms: Vec<RealmItem>,
}

/// The worlds in `state`, or all but the archived ones, oldest first.
async fn realms(
    State(state): State<AppState>,
    _session: AdminSession,
    ApiQuery(query): ApiQuery<RealmsQuery>,
) -> Result<Json<RealmPage>, AppError> {
    let wanted = filter(query.state)?;
    if wanted.as_deref().is_some_and(|wanted| {
        !matches!(
            wanted,
            "stopped" | "starting" | "running" | "stopping" | "failed" | "archived"
        )
    }) {
        return Err(invalid(
            "The state is stopped, starting, running, stopping, failed or archived.",
        ));
    }
    let rows = sqlx::query_as(&format!(
        "{REALM_ROW} WHERE CASE WHEN $1::text IS NULL THEN r.state <> 'archived' \
             ELSE r.state = $1 END \
         ORDER BY r.created_at, r.id"
    ))
    .bind(&wanted)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(RealmPage {
        realms: realm_items(&state, rows),
    }))
}

async fn stop_realm(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    admin::stop_realm(&state, &actor(&session), realm_id(&id)?).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn archive_realm(
    State(state): State<AppState>,
    session: AdminSession,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    admin::archive_realm(&state, &actor(&session), realm_id(&id)?).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditQuery {
    /// An account id, or a username.
    account: Option<String>,
    action: Option<String>,
    /// Only rows of a lower id.
    before: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct AuditPage {
    entries: Vec<AuditItem>,
    /// The `before` of the next page, when this one is full.
    next_before: Option<i64>,
}

/// The account a filter names, by id or by username.
async fn named_account(db: &PgPool, given: &str) -> Result<Option<Uuid>, AppError> {
    if let Ok(id) = Uuid::parse_str(given) {
        return Ok(Some(id));
    }
    Ok(
        sqlx::query_scalar("SELECT id FROM accounts WHERE username = $1::citext")
            .bind(given)
            .fetch_optional(db)
            .await?,
    )
}

/// Audit rows, newest first, a page at a time by id.
async fn audit_log(
    State(state): State<AppState>,
    _session: AdminSession,
    ApiQuery(query): ApiQuery<AuditQuery>,
) -> Result<Json<AuditPage>, AppError> {
    let limit = limit(query.limit, DEFAULT_AUDIT, MAX_AUDIT)?;
    let account = match filter(query.account)? {
        Some(given) => match named_account(&state.db, &given).await? {
            Some(id) => Some(id),
            None => {
                return Ok(Json(AuditPage {
                    entries: Vec::new(),
                    next_before: None,
                }))
            }
        },
        None => None,
    };
    let action = filter(query.action)?;
    let rows: Vec<AuditRow> = sqlx::query_as(&format!(
        "{AUDIT_ROW} WHERE ($1::uuid IS NULL OR l.account_id = $1) \
             AND ($2::text IS NULL OR l.action = $2) \
             AND ($3::bigint IS NULL OR l.id < $3) \
         ORDER BY l.id DESC LIMIT $4"
    ))
    .bind(account)
    .bind(&action)
    .bind(query.before)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    let next_before = (rows.len() as i64 == limit)
        .then(|| rows.last().map(|row| row.id))
        .flatten();
    Ok(Json(AuditPage {
        entries: rows.into_iter().map(AuditRow::item).collect(),
        next_before,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_origin_of_a_url_is_its_scheme_host_and_port() {
        assert_eq!(
            origin("https://hub.rustcraft.dev"),
            "https://hub.rustcraft.dev"
        );
        assert_eq!(origin("https://Hub.Example/"), "https://hub.example");
        assert_eq!(origin("https://hub.example:443/x"), "https://hub.example");
        assert_eq!(origin("http://localhost:8080"), "http://localhost:8080");
        assert_eq!(origin("http://localhost:80"), "http://localhost");
    }

    #[test]
    fn a_request_from_another_site_is_refused() {
        let origin = "https://hub.example";
        let headers = |pairs: &[(&'static str, &'static str)]| {
            let mut headers = HeaderMap::new();
            for (name, value) in pairs {
                headers.insert(*name, HeaderValue::from_static(value));
            }
            headers
        };
        assert!(check_request(origin, &Method::GET, &headers(&[])).is_ok());
        assert!(check_request(origin, &Method::POST, &headers(&[])).is_err());
        assert!(check_request(origin, &Method::POST, &headers(&[(ADMIN_HEADER, "1")])).is_ok());
        assert!(check_request(origin, &Method::POST, &headers(&[(ADMIN_HEADER, "yes")])).is_err());
        let same = [
            ("origin", "https://hub.example"),
            ("sec-fetch-site", "same-origin"),
        ];
        assert!(check_request(origin, &Method::GET, &headers(&same)).is_ok());
        let typed = [("sec-fetch-site", "none")];
        assert!(check_request(origin, &Method::GET, &headers(&typed)).is_ok());
        for foreign in [
            [("origin", "https://evil.example")],
            [("origin", "null")],
            [("sec-fetch-site", "cross-site")],
            [("sec-fetch-site", "same-site")],
        ] {
            assert!(check_request(origin, &Method::GET, &headers(&foreign)).is_err());
        }
    }
}
