//! The `rustcraft-hub admin` commands. Each takes the pool and returns the text to print, so
//! that tests run them against a test database. They write `audit_log` rows as `admin-cli`.

use std::fmt::Write;

use anyhow::{bail, Context};
use serde_json::json;
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    audit,
    config::{AdminCommand, RoleArg},
    password::{new_temporary_password, Hasher},
    sessions::revoke_access,
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

fn role_name(role: RoleArg) -> &'static str {
    match role {
        RoleArg::Admin => "admin",
        RoleArg::Host => "host",
    }
}

/// The id and status of the account named `user`.
async fn find(conn: &mut PgConnection, user: &str) -> anyhow::Result<(Uuid, String)> {
    sqlx::query_as("SELECT id, status FROM accounts WHERE username = $1::citext FOR UPDATE")
        .bind(user)
        .fetch_optional(conn)
        .await?
        .with_context(|| format!("no account is named {user}"))
}

/// Like [`find`], and refuses a deleted account.
async fn find_live(conn: &mut PgConnection, user: &str) -> anyhow::Result<(Uuid, String)> {
    let (id, status) = find(conn, user).await?;
    if status == "deleted" {
        bail!("{user} is deleted");
    }
    Ok((id, status))
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
    let mut tx = db.begin().await?;
    let (id, _) = find_live(&mut tx, user).await?;
    sqlx::query(
        "INSERT INTO account_roles (account_id, role) VALUES ($1, $2) \
         ON CONFLICT (account_id, role) DO NOTHING",
    )
    .bind(id)
    .bind(role_name(role))
    .execute(&mut *tx)
    .await?;
    let mut details = json!({"role": role_name(role)});
    let mut message = format!("{user} is now {}", role_name(role));
    if role == RoleArg::Host {
        let quota = realms.unwrap_or(DEFAULT_REALMS);
        sqlx::query("UPDATE accounts SET realm_quota = $2 WHERE id = $1")
            .bind(id)
            .bind(quota)
            .execute(&mut *tx)
            .await?;
        details["realm_quota"] = quota.into();
        message.push_str(&format!(" with {quota} world(s)"));
    }
    audit::record(&mut tx, audit::ADMIN_CLI, "role_granted", Some(id), details).await?;
    tx.commit().await?;
    Ok(message)
}

/// Takes `role` away. The quota of worlds stays, and does nothing without the role.
pub async fn revoke(db: &PgPool, user: &str, role: RoleArg) -> anyhow::Result<String> {
    let mut tx = db.begin().await?;
    let (id, _) = find_live(&mut tx, user).await?;
    let removed = sqlx::query("DELETE FROM account_roles WHERE account_id = $1 AND role = $2")
        .bind(id)
        .bind(role_name(role))
        .execute(&mut *tx)
        .await?;
    if removed.rows_affected() == 0 {
        bail!("{user} is not {}", role_name(role));
    }
    audit::record(
        &mut tx,
        audit::ADMIN_CLI,
        "role_revoked",
        Some(id),
        json!({"role": role_name(role)}),
    )
    .await?;
    tx.commit().await?;
    Ok(format!("{user} is no longer {}", role_name(role)))
}

/// Suspends an active account and revokes its sessions and certificates.
pub async fn suspend(db: &PgPool, user: &str, reason: &str) -> anyhow::Result<String> {
    let reason = reason.trim();
    if reason.is_empty() {
        bail!("give a reason");
    }
    let mut tx = db.begin().await?;
    let (id, status) = find_live(&mut tx, user).await?;
    if status == "suspended" {
        bail!("{user} is already suspended");
    }
    sqlx::query("UPDATE accounts SET status = 'suspended', status_reason = $2 WHERE id = $1")
        .bind(id)
        .bind(reason)
        .execute(&mut *tx)
        .await?;
    revoke_access(&mut tx, id, None).await?;
    audit::record(
        &mut tx,
        audit::ADMIN_CLI,
        "suspended",
        Some(id),
        json!({"reason": reason}),
    )
    .await?;
    tx.commit().await?;
    Ok(format!("{user} is suspended"))
}

pub async fn unsuspend(db: &PgPool, user: &str) -> anyhow::Result<String> {
    let mut tx = db.begin().await?;
    let (id, status) = find_live(&mut tx, user).await?;
    if status != "suspended" {
        bail!("{user} is not suspended");
    }
    sqlx::query("UPDATE accounts SET status = 'active', status_reason = NULL WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        audit::ADMIN_CLI,
        "unsuspended",
        Some(id),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(format!("{user} is active again"))
}

/// Sets a generated password and returns it in the message. Like a password change it ends the
/// sessions and revokes the certificates; the recovery code is left alone.
pub async fn reset_password(db: &PgPool, user: &str) -> anyhow::Result<String> {
    let password = new_temporary_password();
    let hash = Hasher::new(1)
        .hash(password.clone())
        .await
        .map_err(|error| anyhow::anyhow!("hashing the password: {}", error.message))?;
    let mut tx = db.begin().await?;
    let (id, _) = find_live(&mut tx, user).await?;
    let updated = sqlx::query(
        "UPDATE password_credentials SET password_hash = $2, updated_at = now() \
         WHERE account_id = $1",
    )
    .bind(id)
    .bind(hash)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        bail!("{user} has no password");
    }
    revoke_access(&mut tx, id, None).await?;
    audit::record(
        &mut tx,
        audit::ADMIN_CLI,
        "password_reset",
        Some(id),
        json!({}),
    )
    .await?;
    tx.commit().await?;
    Ok(format!("The password of {user} is now {password}"))
}
