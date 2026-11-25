//! A router over a fresh database, dropped with the `TestHub`, even when the test fails.

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, Response},
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
    /// the router. The database is dropped when the `TestHub` is.
    pub async fn start() -> Self {
        let url = std::env::var("HUB_TEST_DATABASE_URL")
            .expect("HUB_TEST_DATABASE_URL is not set: run hub/scripts/test-db.sh start");
        let admin_options: PgConnectOptions = url.parse().unwrap();
        let database = format!("hub_test_{}", uuid::Uuid::new_v4().simple());
        let mut admin = PgConnection::connect_with(&admin_options).await.unwrap();
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
