//! The routes of hosted worlds. A world is seen by its owner and its members only: to anyone
//! else it does not exist (404).

use std::collections::HashMap;

use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rustcraft_agent_protocol::check_settings;
use rustcraft_protocol::{
    api::{
        AccountSummary, CreateRealmRequest, ErrorCode, MemberRole, Realm, RealmAddress,
        RealmConfig, RealmList, RealmMember, RealmState, SetMemberRequest, StartRealmResponse,
    },
    REALM_AUTH_SCHEME,
};
use sqlx::{types::Json as SqlJson, PgPool};
use uuid::Uuid;

use super::{archive_volumes, not_found, parse_state, token_hash, MARK_ARCHIVED};
use crate::{error::AppError, routes::ApiJson, sessions::Session, AppState};

/// The longest name of a world, in `char`s.
const NAME_LENGTH: usize = 32;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/realms", get(list).post(create))
        .route("/v1/realms/{id}", axum::routing::delete(archive))
        .route("/v1/realms/{id}/start", post(start))
        .route("/v1/realms/{id}/stop", post(stop))
        .route(
            "/v1/realms/{id}/members/{username}",
            put(set_member).delete(remove_member),
        )
        .route("/v1/realms/{id}/config", get(config))
        .layer(DefaultBodyLimit::max(16 * 1024))
}

fn forbidden() -> AppError {
    AppError::new(
        ErrorCode::Forbidden,
        "Only the owner of the world can do that.",
    )
}

fn realm_id(text: &str) -> Result<Uuid, AppError> {
    Uuid::parse_str(text).map_err(|_| not_found())
}

fn role_name(role: MemberRole) -> &'static str {
    match role {
        MemberRole::Member => "member",
        MemberRole::Operator => "operator",
    }
}

fn parse_role(name: &str) -> MemberRole {
    match name {
        "operator" => MemberRole::Operator,
        _ => MemberRole::Member,
    }
}

#[derive(sqlx::FromRow)]
struct RealmRow {
    id: Uuid,
    name: String,
    state: String,
    slot: Option<i16>,
    owner_id: Uuid,
    owner_name: String,
}

const REALM_ROW: &str = "SELECT r.id, r.name, r.state, r.slot, r.owner_id, \
         o.username::text AS owner_name \
     FROM realms r JOIN accounts o ON o.id = r.owner_id \
     WHERE r.state <> 'archived'";

/// The world `id`, when `account` owns it or is one of its members.
async fn visible(db: &PgPool, id: Uuid, account: Uuid) -> Result<RealmRow, AppError> {
    sqlx::query_as(&format!(
        "{REALM_ROW} AND r.id = $1 AND (r.owner_id = $2 OR EXISTS \
             (SELECT 1 FROM realm_members m WHERE m.realm_id = r.id AND m.account_id = $2))"
    ))
    .bind(id)
    .bind(account)
    .fetch_optional(db)
    .await?
    .ok_or_else(not_found)
}

/// The world `id`, when `account` owns it: 403 for a member, 404 for anyone else.
async fn owned(db: &PgPool, id: Uuid, account: Uuid) -> Result<RealmRow, AppError> {
    let realm = visible(db, id, account).await?;
    if realm.owner_id != account {
        return Err(forbidden());
    }
    Ok(realm)
}

async fn members(
    db: &PgPool,
    realms: &[Uuid],
) -> Result<HashMap<Uuid, Vec<RealmMember>>, AppError> {
    let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT m.realm_id, a.id, a.username::text, m.role \
         FROM realm_members m JOIN accounts a ON a.id = m.account_id \
         WHERE m.realm_id = ANY($1) ORDER BY a.username",
    )
    .bind(realms)
    .fetch_all(db)
    .await?;
    let mut members: HashMap<Uuid, Vec<RealmMember>> = HashMap::new();
    for (realm, id, username, role) in rows {
        members.entry(realm).or_default().push(RealmMember {
            id,
            username,
            role: parse_role(&role),
        });
    }
    Ok(members)
}

async fn realms(state: &AppState, rows: Vec<RealmRow>) -> Result<RealmList, AppError> {
    let ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let mut members = members(&state.db, &ids).await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            let realm_state = parse_state(&row.state);
            let address = match (realm_state, row.slot) {
                (RealmState::Starting | RealmState::Running, Some(slot)) => Some(RealmAddress {
                    host: state.realms.host.clone(),
                    port: state.realms.game_port(slot),
                }),
                _ => None,
            };
            Realm {
                id: row.id,
                name: row.name,
                owner: AccountSummary {
                    id: row.owner_id,
                    username: row.owner_name,
                },
                state: realm_state,
                address,
                members: members.remove(&row.id).unwrap_or_default(),
            }
        })
        .collect())
}

/// The worlds the account owns or is a member of, oldest first.
async fn list(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<RealmList>, AppError> {
    let rows = sqlx::query_as(&format!(
        "{REALM_ROW} AND (r.owner_id = $1 OR EXISTS \
             (SELECT 1 FROM realm_members m WHERE m.realm_id = r.id AND m.account_id = $1)) \
         ORDER BY r.created_at, r.id"
    ))
    .bind(session.account_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(realms(&state, rows).await?))
}

fn check_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    let length = name.chars().count();
    if !(1..=NAME_LENGTH).contains(&length) || name.chars().any(char::is_control) {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "The name of a world has 1 to 32 characters, and no control character.",
        ));
    }
    Ok(name.to_owned())
}

/// Creates a stopped world, for an account with the `host` role and room in its quota.
async fn create(
    State(state): State<AppState>,
    session: Session,
    ApiJson(request): ApiJson<CreateRealmRequest>,
) -> Result<(StatusCode, Json<Realm>), AppError> {
    let name = check_name(&request.name)?;
    check_settings(&request.settings).map_err(|reason| {
        AppError::new(
            ErrorCode::InvalidRequest,
            format!("The settings of the world are not valid: {reason}."),
        )
    })?;
    let mut tx = state.db.begin().await?;
    // The account's row is locked, so that parallel creations cannot pass the quota together.
    let (quota, host): (i32, bool) = sqlx::query_as(
        "SELECT realm_quota, EXISTS (SELECT 1 FROM account_roles \
             WHERE account_id = a.id AND role = 'host') \
         FROM accounts a WHERE id = $1 FOR UPDATE",
    )
    .bind(session.account_id)
    .fetch_one(&mut *tx)
    .await?;
    if !host {
        return Err(AppError::new(
            ErrorCode::Forbidden,
            "Creating a world needs the host role.",
        ));
    }
    let owned: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM realms WHERE owner_id = $1 AND state <> 'archived'",
    )
    .bind(session.account_id)
    .fetch_one(&mut *tx)
    .await?;
    if owned >= i64::from(quota) {
        return Err(AppError::new(
            ErrorCode::QuotaReached,
            format!("This account may own {quota} world(s), and owns as many."),
        ));
    }
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO realms (id, owner_id, name, settings) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(session.account_id)
        .bind(&name)
        .bind(SqlJson(&request.settings))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let row = visible(&state.db, id, session.account_id).await?;
    let realm = realms(&state, vec![row]).await?.remove(0);
    Ok((StatusCode::CREATED, Json(realm)))
}

/// Starts the world for any of its members. The start runs to its end even if the client goes
/// away.
async fn start(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<StartRealmResponse>, AppError> {
    let id = visible(&state.db, realm_id(&id)?, session.account_id)
        .await?
        .id;
    tokio::spawn(async move { super::start(&state, id).await })
        .await
        .map_err(|_| super::internal())?
        .map(Json)
}

/// Stops the world, in the background: the world is `stopping` until its server has saved.
async fn stop(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = owned(&state.db, realm_id(&id)?, session.account_id)
        .await?
        .id;
    tokio::spawn(async move {
        if let Err(error) = super::stop(&state, id, None).await {
            tracing::warn!(%id, error = error.message, "could not stop a world");
        }
    });
    Ok(StatusCode::ACCEPTED)
}

/// Archives the world: it leaves every list at once, and the agent moves its volume aside.
async fn archive(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let id = owned(&state.db, realm_id(&id)?, session.account_id)
        .await?
        .id;
    sqlx::query(&format!(
        "{MARK_ARCHIVED} WHERE id = $1 AND state <> 'archived'"
    ))
    .bind(id)
    .execute(&state.db)
    .await?;
    archive_volumes(state, vec![id]);
    Ok(StatusCode::NO_CONTENT)
}

/// The active account named `username`.
async fn account_named(db: &PgPool, username: &str) -> Result<Uuid, AppError> {
    sqlx::query_scalar("SELECT id FROM accounts WHERE username = $1::citext AND status = 'active'")
        .bind(username)
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::new(ErrorCode::NotFound, "No player has that name."))
}

/// Adds a member, or changes their role.
async fn set_member(
    State(state): State<AppState>,
    session: Session,
    Path((id, username)): Path<(String, String)>,
    ApiJson(request): ApiJson<SetMemberRequest>,
) -> Result<StatusCode, AppError> {
    let realm = owned(&state.db, realm_id(&id)?, session.account_id).await?;
    let member = account_named(&state.db, &username).await?;
    if member == realm.owner_id {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "The owner of a world is not one of its members.",
        ));
    }
    sqlx::query(
        "INSERT INTO realm_members (realm_id, account_id, role) VALUES ($1, $2, $3) \
         ON CONFLICT (realm_id, account_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(realm.id)
    .bind(member)
    .bind(role_name(request.role))
    .execute(&state.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_member(
    State(state): State<AppState>,
    session: Session,
    Path((id, username)): Path<(String, String)>,
) -> Result<StatusCode, AppError> {
    let realm = owned(&state.db, realm_id(&id)?, session.account_id).await?;
    let removed = sqlx::query(
        "DELETE FROM realm_members m USING accounts a \
         WHERE m.realm_id = $1 AND m.account_id = a.id AND a.username = $2::citext",
    )
    .bind(realm.id)
    .bind(&username)
    .execute(&state.db)
    .await?;
    if removed.rows_affected() == 0 {
        return Err(AppError::new(
            ErrorCode::NotFound,
            "No member of this world has that name.",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `a == b`, in a time that does not depend on where they differ.
fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |diff, (x, y)| diff | (x ^ y)) == 0
}

/// The members of the world, for its server, which shows its realm token. A wrong token and a
/// world that does not exist, or no longer runs, get the same 401.
async fn config(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<RealmConfig>, AppError> {
    let unauthorized = || AppError::new(ErrorCode::Unauthorized, "This is not the world's token.");
    let id = Uuid::parse_str(&id).map_err(|_| unauthorized())?;
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix(REALM_AUTH_SCHEME))
        .and_then(|value| value.strip_prefix(' '))
        .and_then(|token| URL_SAFE_NO_PAD.decode(token.trim()).ok())
        .ok_or_else(unauthorized)?;
    let found: Option<(Vec<u8>, Uuid, String)> = sqlx::query_as(
        "SELECT r.token_hash, o.id, o.username::text FROM realms r \
         JOIN accounts o ON o.id = r.owner_id \
         WHERE r.id = $1 AND r.state <> 'archived' AND r.token_hash IS NOT NULL",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let Some((hash, owner_id, owner_name)) = found else {
        return Err(unauthorized());
    };
    if !same_bytes(&hash, &token_hash(&token)) {
        return Err(unauthorized());
    }
    Ok(Json(RealmConfig {
        members: members(&state.db, &[id])
            .await?
            .remove(&id)
            .unwrap_or_default(),
        owner: AccountSummary {
            id: owner_id,
            username: owner_name,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_has_1_to_32_characters_and_no_control_character() {
        assert_eq!(check_name("  Home  ").unwrap(), "Home");
        assert!(check_name(&"é".repeat(32)).is_ok());
        assert!(check_name(&"é".repeat(33)).is_err());
        assert!(check_name("   ").is_err());
        assert!(check_name("Home\nSweet").is_err());
        assert!(check_name("Bell\u{7}").is_err());
    }

    #[test]
    fn bytes_are_the_same_only_when_all_are() {
        assert!(same_bytes(&[1, 2, 3], &[1, 2, 3]));
        assert!(!same_bytes(&[1, 2, 3], &[1, 2, 4]));
        assert!(!same_bytes(&[1, 2], &[1, 2, 3]));
    }
}
