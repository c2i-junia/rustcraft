//! A router over a fresh database, dropped with the `TestHub` when the test passes. A failing
//! test leaves it behind (the Cranelift dev build runs no cleanup on a panic), and the next run
//! drops it.

// Each test file uses some of the helpers only.
#![allow(dead_code)]

use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Request, Response},
    Router,
};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rustcraft_hub::{migrate, router, AppState};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, PgConnection,
};
use tower::ServiceExt;

pub struct TestHub {
    pub app: Router,
    pub state: AppState,
    admin_options: PgConnectOptions,
    database: String,
}

impl TestHub {
    /// Creates a uniquely named database from `HUB_TEST_DATABASE_URL`, migrates it and builds
    /// the router. The database is dropped when the `TestHub` is. The first one of a process
    /// also drops the databases that earlier, failed runs left.
    pub async fn start() -> Self {
        let url = std::env::var("HUB_TEST_DATABASE_URL")
            .expect("HUB_TEST_DATABASE_URL is not set: run hub/scripts/test-db.sh start");
        let admin_options: PgConnectOptions = url.parse().unwrap();
        let process = format!("hub_test_{}_", std::process::id());
        let database = format!("{process}{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect_with(&admin_options).await.unwrap();
        static CLEANED: AtomicBool = AtomicBool::new(false);
        if !CLEANED.swap(true, Ordering::SeqCst) {
            drop_stale_databases(&mut admin, &process).await;
        }
        sqlx::query(&format!("CREATE DATABASE {database}"))
            .execute(&mut admin)
            .await
            .unwrap();
        let db = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(admin_options.clone().database(&database))
            .await
            .unwrap();
        migrate(&db).await.unwrap();
        let state = AppState {
            db,
            name: "Test hub".into(),
            public_url: "https://hub.example".into(),
            signing_key: Arc::new(SigningKey::generate(&mut OsRng)),
            ip_secret: Arc::from(vec![7u8; 32]),
            trusted_ranges: Arc::from([]),
            hasher: Default::default(),
            logins: Default::default(),
            signups: Default::default(),
            certificates: Default::default(),
        };
        Self {
            app: router(state.clone()),
            state,
            admin_options,
            database,
        }
    }

    pub async fn get(&self, path: &str) -> Response<Body> {
        let request = Request::get(path).body(Body::empty()).unwrap();
        self.app.clone().oneshot(request).await.unwrap()
    }

    /// A request from the TCP peer `peer` (such as `"203.0.113.1:4000"`), with a JSON body and a
    /// bearer token when given.
    pub async fn send(
        &self,
        method: &str,
        path: &str,
        peer: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> Response<Body> {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let mut request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(peer.parse::<SocketAddr>().unwrap()));
        self.app.clone().oneshot(request).await.unwrap()
    }
}

/// Drops the test databases of other processes that nobody is connected to.
async fn drop_stale_databases(admin: &mut PgConnection, ours: &str) {
    let stale: Vec<String> = sqlx::query_scalar(
        "SELECT datname::text FROM pg_database d \
         WHERE starts_with(datname, 'hub_test_') AND NOT starts_with(datname, $1) \
         AND NOT EXISTS (SELECT 1 FROM pg_stat_activity a WHERE a.datname = d.datname)",
    )
    .bind(ours)
    .fetch_all(&mut *admin)
    .await
    .unwrap();
    for database in stale {
        // Another run may have connected to it since: then it stays.
        let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS \"{database}\""))
            .execute(&mut *admin)
            .await;
    }
}

impl Drop for TestHub {
    fn drop(&mut self) {
        let options = self.admin_options.clone();
        let sql = format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", self.database);
        // A thread of its own, because a test's runtime cannot block on this.
        let _ = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let mut admin = PgConnection::connect_with(&options).await.unwrap();
                    sqlx::query(&sql).execute(&mut admin).await.unwrap();
                })
        })
        .join();
    }
}
