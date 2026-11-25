//! The Rustcraft hub: accounts, device certificates and hosted worlds.

pub mod accounts;
pub mod admin;
pub mod audit;
pub mod certificates;
pub mod config;
pub mod error;
pub mod maintenance;
pub mod password;
pub mod ratelimit;
pub mod routes;
pub mod sessions;

use std::{sync::Arc, time::Duration};

use ed25519_dalek::SigningKey;
use ipnet::IpNet;
use sqlx::{postgres::PgPoolOptions, PgPool};

use password::Hasher;
use ratelimit::{CertificateLimiter, LoginLimiter, SignupGate};
pub use routes::router;

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
    /// Sign-ups from these ranges have a higher limit.
    pub trusted_ranges: Arc<[IpNet]>,
    /// Peers whose `X-Forwarded-For` is believed: the reverse proxy.
    pub trusted_proxies: Arc<[IpNet]>,
    pub hasher: Hasher,
    pub logins: Arc<LoginLimiter>,
    pub signups: Arc<SignupGate>,
    pub certificates: Arc<CertificateLimiter>,
}

/// Connects to the database and applies the migrations.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let db = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
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
