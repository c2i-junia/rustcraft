//! What an admin does to accounts and worlds, for the `rustcraft-hub admin` commands and the
//! web admin alike. Each operation takes its [`Actor`], writes its `audit_log` row as that
//! actor, and returns data; the commands below format their text from it, and take the pool so
//! that tests run them against a test database.

use std::fmt::{self, Write};

use anyhow::bail;
use serde_json::{json, Value};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit,
    config::{AdminCommand, RoleArg},
    error::AppError,
    password::{new_temporary_password, Hasher},
    realms::{self, MARK_ARCHIVED},
    sessions::revoke_access,
    AppState,
};

/// How many worlds `grant <user> host` allows without `--realms`.
pub const DEFAULT_REALMS: i32 = 1;

pub async fn run(db: &PgPool, command: AdminCommand) -> anyhow::Result<String> {
    match command {
        AdminCommand::Users => users(db).await,
        AdminCommand::Grant { user, role, realms } => grant(db, &user, role, realms).await,
        AdminCommand::Revoke { user, role } => revoke(db, &user, role).await,
        AdminCommand::Suspend { user, reason } => suspend(db, &user, &reason).await,
        AdminCommand::Unsuspend { user } => unsuspend(db, &user).await,
        AdminCommand::ResetPassword { user } => reset_password(db, &user).await,
        AdminCommand::Realms => realms(db).await,
    }
}

pub fn role_name(role: RoleArg) -> &'static str {
    match role {
        RoleArg::Admin => "admin",
        RoleArg::Host => "host",
    }
}

/// Who does an admin operation: the command line, or an admin of the web interface.
#[derive(Clone, Debug)]
pub struct Actor {
    name: String,
    admin_id: Option<Uuid>,
}

impl Actor {
    /// The `rustcraft-hub admin` commands, `admin-cli` in the audit log.
    pub fn cli() -> Self {
        Self {
            name: audit::ADMIN_CLI.to_owned(),
            admin_id: None,
        }
    }

    /// The admin `username` of the web interface, `admin:<username>` in the audit log, whose
    /// rows also hold `details.admin_id`.
    pub fn web(admin_id: Uuid, username: &str) -> Self {
        Self {
            name: format!("admin:{username}"),
            admin_id: Some(admin_id),
        }
    }

    /// Writes an `audit_log` row as this actor.
    pub(crate) async fn record(
        &self,
        conn: &mut PgConnection,
        action: &str,
        account_id: Option<Uuid>,
        mut details: Value,
    ) -> Result<(), sqlx::Error> {
        if let Some(admin_id) = self.admin_id {
            details["admin_id"] = admin_id.to_string().into();
        }
        audit::record(conn, &self.name, action, account_id, details).await
    }

    /// Refuses with `message` when the target is the actor's own account.
    fn not_on_self(&self, target: Uuid, message: &str) -> Result<(), AdminError> {
        if self.admin_id == Some(target) {
            return Err(AdminError::Refused(message.to_owned()));
        }
        Ok(())
    }
}

/// The account an operation is on: by name for the commands, by id for the web.
#[derive(Clone, Copy, Debug)]
pub enum Target<'a> {
    Name(&'a str),
    Id(Uuid),
}

/// Why an admin operation did not happen. Its `Display` is the sentence to show.
#[derive(Debug)]
pub enum AdminError {
    /// No such account or world.
    NotFound(String),
    /// The request does not apply, such as suspending a suspended account.
    Invalid(String),
    /// Not allowed: on a deleted account, or on oneself.
    Refused(String),
    Internal(anyhow::Error),
}

impl fmt::Display for AdminError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(message) | Self::Invalid(message) | Self::Refused(message) => {
                f.write_str(message)
            }
            Self::Internal(error) => write!(f, "{error}"),
        }
    }
}

impl From<sqlx::Error> for AdminError {
    fn from(error: sqlx::Error) -> Self {
        Self::Internal(error.into())
    }
}

impl From<AppError> for AdminError {
    fn from(error: AppError) -> Self {
        Self::Internal(anyhow::anyhow!("{}", error.message))
    }
}

impl From<AdminError> for anyhow::Error {
    fn from(error: AdminError) -> Self {
        match error {
            AdminError::Internal(error) => error,
            other => anyhow::anyhow!("{other}"),
        }
    }
}

/// An account found for an operation, locked until the end of its transaction.
struct Found {
    id: Uuid,
    status: String,
    /// How messages name it: as the command gave it, or its username.
    label: String,
}

/// The target account, locked.
async fn find(conn: &mut PgConnection, target: Target<'_>) -> Result<Found, AdminError> {
    let found: Option<(Uuid, String, String)> = match target {
        Target::Name(user) => {
            sqlx::query_as(
                "SELECT id, status, $1::text FROM accounts WHERE username = $1::citext \
                 FOR UPDATE",
            )
            .bind(user)
            .fetch_optional(conn)
            .await?
        }
        Target::Id(id) => {
            sqlx::query_as(
                "SELECT id, status, username::text FROM accounts WHERE id = $1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(conn)
            .await?
        }
    };
    let (id, status, label) = found.ok_or_else(|| {
        AdminError::NotFound(match target {
            Target::Name(user) => format!("no account is named {user}"),
            Target::Id(_) => "There is no such account.".to_owned(),
        })
    })?;
    Ok(Found { id, status, label })
}

/// Like [`find`], and refuses a deleted account.
async fn find_live(conn: &mut PgConnection, target: Target<'_>) -> Result<Found, AdminError> {
    let found = find(conn, target).await?;
    if found.status == "deleted" {
        return Err(AdminError::Refused(format!("{} is deleted", found.label)));
    }
    Ok(found)
}

#[derive(sqlx::FromRow)]
struct UserRow {
    username: String,
    roles: String,
    realm_quota: i32,
    status: String,
    created_at: OffsetDateTime,
}

/// A table of the accounts, oldest first.
pub async fn users(db: &PgPool) -> anyhow::Result<String> {
    let rows: Vec<UserRow> = sqlx::query_as(
        "SELECT a.username::text AS username, \
             COALESCE((SELECT string_agg(r.role, ',' ORDER BY r.role) FROM account_roles r \
                 WHERE r.account_id = a.id), '') AS roles, \
             a.realm_quota, a.status, a.created_at \
         FROM accounts a ORDER BY a.created_at, a.username",
    )
    .fetch_all(db)
    .await?;
    let rows: Vec<[String; 5]> = rows
        .into_iter()
        .map(|row| {
            let date = row.created_at.date();
            [
                row.username,
                if row.roles.is_empty() {
                    "-".to_owned()
                } else {
                    row.roles
                },
                row.realm_quota.to_string(),
                row.status,
                date.to_string(),
            ]
        })
        .collect();
    table(["NAME", "ROLES", "QUOTA", "STATUS", "CREATED"], &rows)
}

/// The rows under `header`, in columns as wide as their widest cell.
fn table<const N: usize>(header: [&str; N], rows: &[[String; N]]) -> anyhow::Result<String> {
    let header = header.map(str::to_owned);
    let mut widths = [0; N];
    for row in std::iter::once(&header).chain(rows) {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut table = String::new();
    for row in std::iter::once(&header).chain(rows) {
        let line: Vec<String> = row
            .iter()
            .zip(widths)
            .map(|(cell, width)| format!("{cell:<width$}"))
            .collect();
        writeln!(table, "{}", line.join("  ").trim_end())?;
    }
    Ok(table.trim_end().to_owned())
}

#[derive(sqlx::FromRow)]
struct RealmRow {
    id: Uuid,
    name: String,
    owner: String,
    state: String,
    slot: Option<i16>,
    players: i32,
    last_played_at: Option<OffsetDateTime>,
}

/// A table of the hosted worlds, oldest first, archived ones included.
pub async fn realms(db: &PgPool) -> anyhow::Result<String> {
    let rows: Vec<RealmRow> = sqlx::query_as(
        "SELECT r.id, r.name, a.username::text AS owner, r.state, r.slot, r.players, \
             r.last_played_at \
         FROM realms r JOIN accounts a ON a.id = r.owner_id ORDER BY r.created_at, r.id",
    )
    .fetch_all(db)
    .await?;
    let rows: Vec<[String; 7]> = rows
        .into_iter()
        .map(|row| {
            [
                row.id.to_string(),
                row.name,
                row.owner,
                row.state,
                row.slot.map_or("-".to_owned(), |slot| slot.to_string()),
                row.players.to_string(),
                row.last_played_at.map_or("-".to_owned(), |at| {
                    format!("{} {:02}:{:02}", at.date(), at.hour(), at.minute())
                }),
            ]
        })
        .collect();
    table(
        [
            "ID",
            "NAME",
            "OWNER",
            "STATE",
            "SLOT",
            "PLAYERS",
            "LAST PLAYED",
        ],
        &rows,
    )
}

/// Gives `role`; for `host`, also sets the quota of worlds (1 unless `realms` says otherwise).
pub async fn grant(
    db: &PgPool,
    user: &str,
    role: RoleArg,
    realms: Option<i32>,
) -> anyhow::Result<String> {
    if realms.is_some() && role != RoleArg::Host {
        bail!("--realms only goes with the host role");
    }
    if realms.is_some_and(|realms| realms < 0) {
        bail!("--realms cannot be negative");
    }
    let granted = grant_role(db, &Actor::cli(), Target::Name(user), role, realms).await?;
    let mut message = format!("{} is now {}", granted.label, role_name(role));
    if let Some(quota) = granted.realm_quota {
        message.push_str(&format!(" with {quota} world(s)"));
    }
    Ok(message)
}

/// What [`grant_role`] did.
#[derive(Debug)]
pub struct Granted {
    pub label: String,
    /// The quota of worlds it set, with the `host` role.
    pub realm_quota: Option<i32>,
}

/// Gives `role`; for `host`, also sets the quota of worlds ([`DEFAULT_REALMS`] unless `realms`
/// says otherwise). Writes `role_granted`.
pub async fn grant_role(
    db: &PgPool,
    actor: &Actor,
    target: Target<'_>,
    role: RoleArg,
    realms: Option<i32>,
) -> Result<Granted, AdminError> {
    if realms.is_some() && role != RoleArg::Host {
        return Err(AdminError::Invalid(
            "A quota of worlds only goes with the host role.".to_owned(),
        ));
    }
    if realms.is_some_and(|realms| realms < 0) {
        return Err(AdminError::Invalid(
            "A quota of worlds cannot be negative.".to_owned(),
        ));
    }
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    sqlx::query(
        "INSERT INTO account_roles (account_id, role) VALUES ($1, $2) \
         ON CONFLICT (account_id, role) DO NOTHING",
    )
    .bind(found.id)
    .bind(role_name(role))
    .execute(&mut *tx)
    .await?;
    let mut details = json!({"role": role_name(role)});
    let mut realm_quota = None;
    if role == RoleArg::Host {
        let quota = realms.unwrap_or(DEFAULT_REALMS);
        sqlx::query("UPDATE accounts SET realm_quota = $2 WHERE id = $1")
            .bind(found.id)
            .bind(quota)
            .execute(&mut *tx)
            .await?;
        details["realm_quota"] = quota.into();
        realm_quota = Some(quota);
    }
    actor
        .record(&mut tx, "role_granted", Some(found.id), details)
        .await?;
    tx.commit().await?;
    Ok(Granted {
        label: found.label,
        realm_quota,
    })
}

/// Takes `role` away. The quota of worlds stays, and does nothing without the role.
pub async fn revoke(db: &PgPool, user: &str, role: RoleArg) -> anyhow::Result<String> {
    let label = revoke_role(db, &Actor::cli(), Target::Name(user), role).await?;
    Ok(format!("{label} is no longer {}", role_name(role)))
}

/// Takes `role` away and writes `role_revoked`. An admin cannot take their own `admin` role.
pub async fn revoke_role(
    db: &PgPool,
    actor: &Actor,
    target: Target<'_>,
    role: RoleArg,
) -> Result<String, AdminError> {
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    if role == RoleArg::Admin {
        actor.not_on_self(found.id, "You cannot revoke your own admin role.")?;
    }
    let removed = sqlx::query("DELETE FROM account_roles WHERE account_id = $1 AND role = $2")
        .bind(found.id)
        .bind(role_name(role))
        .execute(&mut *tx)
        .await?;
    if removed.rows_affected() == 0 {
        return Err(AdminError::Invalid(format!(
            "{} is not {}",
            found.label,
            role_name(role)
        )));
    }
    actor
        .record(
            &mut tx,
            "role_revoked",
            Some(found.id),
            json!({"role": role_name(role)}),
        )
        .await?;
    tx.commit().await?;
    Ok(found.label)
}

/// The most worlds the web admin gives an account through its quota.
pub const MAX_REALM_QUOTA: i32 = 100;

/// Sets the quota of worlds, `0..=`[`MAX_REALM_QUOTA`], and writes `realm_quota_set`.
pub async fn set_realm_quota(
    db: &PgPool,
    actor: &Actor,
    target: Target<'_>,
    quota: i32,
) -> Result<String, AdminError> {
    if !(0..=MAX_REALM_QUOTA).contains(&quota) {
        return Err(AdminError::Invalid(format!(
            "A quota of worlds is 0 to {MAX_REALM_QUOTA}."
        )));
    }
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    sqlx::query("UPDATE accounts SET realm_quota = $2 WHERE id = $1")
        .bind(found.id)
        .bind(quota)
        .execute(&mut *tx)
        .await?;
    actor
        .record(
            &mut tx,
            "realm_quota_set",
            Some(found.id),
            json!({"realm_quota": quota}),
        )
        .await?;
    tx.commit().await?;
    Ok(found.label)
}

/// Suspends an active account and revokes its sessions and certificates.
pub async fn suspend(db: &PgPool, user: &str, reason: &str) -> anyhow::Result<String> {
    let label = suspend_account(db, &Actor::cli(), Target::Name(user), reason).await?;
    Ok(format!("{label} is suspended"))
}

/// Suspends an active account, revokes its sessions and certificates, and writes `suspended`.
/// The reason is trimmed, and must not be empty. An admin cannot suspend themselves.
pub async fn suspend_account(
    db: &PgPool,
    actor: &Actor,
    target: Target<'_>,
    reason: &str,
) -> Result<String, AdminError> {
    let reason = reason.trim();
    if reason.is_empty() {
        return Err(AdminError::Invalid("give a reason".to_owned()));
    }
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    actor.not_on_self(found.id, "You cannot suspend yourself.")?;
    if found.status == "suspended" {
        return Err(AdminError::Invalid(format!(
            "{} is already suspended",
            found.label
        )));
    }
    sqlx::query("UPDATE accounts SET status = 'suspended', status_reason = $2 WHERE id = $1")
        .bind(found.id)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    revoke_access(&mut tx, found.id, None).await?;
    actor
        .record(
            &mut tx,
            "suspended",
            Some(found.id),
            json!({"reason": reason}),
        )
        .await?;
    tx.commit().await?;
    Ok(found.label)
}

pub async fn unsuspend(db: &PgPool, user: &str) -> anyhow::Result<String> {
    let label = unsuspend_account(db, &Actor::cli(), Target::Name(user)).await?;
    Ok(format!("{label} is active again"))
}

/// Lets a suspended account log in again, and writes `unsuspended`.
pub async fn unsuspend_account(
    db: &PgPool,
    actor: &Actor,
    target: Target<'_>,
) -> Result<String, AdminError> {
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    if found.status != "suspended" {
        return Err(AdminError::Invalid(format!(
            "{} is not suspended",
            found.label
        )));
    }
    sqlx::query("UPDATE accounts SET status = 'active', status_reason = NULL WHERE id = $1")
        .bind(found.id)
        .execute(&mut *tx)
        .await?;
    actor
        .record(&mut tx, "unsuspended", Some(found.id), json!({}))
        .await?;
    tx.commit().await?;
    Ok(found.label)
}

/// Sets a generated password and returns it in the message. Like a password change it ends the
/// sessions and revokes the certificates; the recovery code is left alone.
pub async fn reset_password(db: &PgPool, user: &str) -> anyhow::Result<String> {
    let reset =
        reset_account_password(db, &Hasher::new(1), &Actor::cli(), Target::Name(user)).await?;
    Ok(format!(
        "The password of {} is now {}",
        reset.label, reset.password
    ))
}

/// What [`reset_account_password`] did. Its `Debug` does not show the password.
pub struct PasswordReset {
    pub label: String,
    /// The temporary password, to show once.
    pub password: String,
}

impl fmt::Debug for PasswordReset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PasswordReset")
            .field("label", &self.label)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Sets a generated password, ends the sessions, revokes the certificates and writes
/// `password_reset`; the recovery code is left alone. An admin cannot reset their own.
pub async fn reset_account_password(
    db: &PgPool,
    hasher: &Hasher,
    actor: &Actor,
    target: Target<'_>,
) -> Result<PasswordReset, AdminError> {
    if let Target::Id(id) = target {
        // Before the costly hash.
        actor.not_on_self(id, "You cannot reset your own password here.")?;
    }
    let password = new_temporary_password();
    let hash = hasher
        .hash(password.clone())
        .await
        .map_err(|error| anyhow::anyhow!("hashing the password: {}", error.message))
        .map_err(AdminError::Internal)?;
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    actor.not_on_self(found.id, "You cannot reset your own password here.")?;
    let updated = sqlx::query(
        "UPDATE password_credentials SET password_hash = $2, updated_at = now() \
         WHERE account_id = $1",
    )
    .bind(found.id)
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(AdminError::Invalid(format!(
            "{} has no password",
            found.label
        )));
    }
    revoke_access(&mut tx, found.id, None).await?;
    actor
        .record(&mut tx, "password_reset", Some(found.id), json!({}))
        .await?;
    tx.commit().await?;
    Ok(PasswordReset {
        label: found.label,
        password,
    })
}

/// Ends every session of the account, revokes its certificates, and writes `sessions_ended`.
/// An admin cannot end their own.
pub async fn end_sessions(
    db: &PgPool,
    actor: &Actor,
    target: Target<'_>,
) -> Result<String, AdminError> {
    let mut tx = db.begin().await?;
    let found = find_live(&mut tx, target).await?;
    actor.not_on_self(found.id, "You cannot end your own sessions here.")?;
    revoke_access(&mut tx, found.id, None).await?;
    actor
        .record(&mut tx, "sessions_ended", Some(found.id), json!({}))
        .await?;
    tx.commit().await?;
    Ok(found.label)
}

fn no_such_world() -> AdminError {
    AdminError::NotFound("There is no such world.".to_owned())
}

/// Stops a world that is starting or running, in the background, and writes `realm_stopped`
/// on its owner.
pub async fn stop_realm(state: &AppState, actor: &Actor, id: Uuid) -> Result<(), AdminError> {
    let mut tx = state.db.begin().await?;
    let (owner, name, current): (Uuid, String, String) = sqlx::query_as(
        "SELECT owner_id, name, state FROM realms WHERE id = $1 AND state <> 'archived' \
         FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(no_such_world)?;
    if !matches!(current.as_str(), "starting" | "running" | "stopping") {
        return Err(AdminError::Invalid("The world is not running.".to_owned()));
    }
    actor
        .record(
            &mut tx,
            "realm_stopped",
            Some(owner),
            json!({"realm_id": id.to_string(), "name": name}),
        )
        .await?;
    tx.commit().await?;
    let state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = realms::stop(&state, id, None).await {
            tracing::warn!(%id, error = error.message, "could not stop a world");
        }
    });
    Ok(())
}

/// Archives a world, as its owner would: it leaves every list at once, and the agent moves its
/// volume aside. Writes `realm_archived` on its owner.
pub async fn archive_realm(state: &AppState, actor: &Actor, id: Uuid) -> Result<(), AdminError> {
    let mut tx = state.db.begin().await?;
    let (owner, name): (Uuid, String) = sqlx::query_as(&format!(
        "{MARK_ARCHIVED} WHERE id = $1 AND state <> 'archived' RETURNING owner_id, name"
    ))
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(no_such_world)?;
    actor
        .record(
            &mut tx,
            "realm_archived",
            Some(owner),
            json!({"realm_id": id.to_string(), "name": name}),
        )
        .await?;
    tx.commit().await?;
    realms::archive_volumes(state.clone(), vec![id]);
    Ok(())
}
