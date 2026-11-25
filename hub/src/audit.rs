//! The audit log: who did what to which account.

use serde_json::Value;
use sqlx::PgConnection;
use uuid::Uuid;

/// The actor of what an account does itself.
pub const SELF: &str = "self";
/// The actor of the `rustcraft-hub admin` commands.
pub const ADMIN_CLI: &str = "admin-cli";
/// The actor of a request with no account behind it, such as a failed login.
pub const ANONYMOUS: &str = "anonymous";

pub async fn record(
    conn: &mut PgConnection,
    actor: &str,
    action: &str,
    account_id: Option<Uuid>,
    details: Value,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_log (actor, action, account_id, details) VALUES ($1, $2, $3, $4)",
    )
    .bind(actor)
    .bind(action)
    .bind(account_id)
    .bind(details)
    .execute(conn)
    .await?;
    Ok(())
}
