//! The Rustcraft hub: accounts, device certificates and hosted worlds.

pub mod config;
pub mod error;
pub mod routes;

use std::sync::Arc;

use ed25519_dalek::SigningKey;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub use routes::router;

/// The network protocol the game speaks, announced in the well-known document so that a client
/// can tell a hub from another build. It mirrors `PROTOCOL_ID` of the game's `shared` crate,
/// which the hub cannot depend on (it pulls in Bevy): bump both together.
pub const PROTOCOL_ID: u64 = 44;

/// The id of the signing key in certificates; there is one key for now.
pub const SIGNING_KEY_ID: u8 = 1;

/// What the routes share.
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub name: Arc<str>,
    pub public_url: Arc<str>,
    pub signing_key: Arc<SigningKey>,
    /// Keys the sign-up address hashes.
    pub ip_secret: Arc<[u8]>,
}

/// Connects to the database and applies the migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(database_url)
        .await?;
    migrate(&db).await?;
    Ok(db)
}

/// Applies the migrations of `hub/migrations`.
pub async fn migrate(db: &PgPool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(db).await?;
    Ok(())
}
