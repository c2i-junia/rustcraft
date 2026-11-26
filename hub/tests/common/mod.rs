//! A router over a fresh database, dropped with the `TestHub` when the test passes. A failing
//! test leaves it behind (the Cranelift dev build runs no cleanup on a panic), and the next run
//! drops it.

// Each test file uses some of the helpers only.
#![allow(dead_code)]

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
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
use rustcraft_agent_protocol::{RealmState, Request as AgentRequest, Response as AgentResponse};
use rustcraft_hub::{
    migrate,
    realms::{
        agent::{AgentFuture, RealmAgent},
        probe::{ProbeFuture, StatusProbe},
        Hosting,
    },
    router, AppState,
};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    Connection, PgConnection,
};
use tokio::sync::Notify;
use tower::ServiceExt;
use uuid::Uuid;

/// The host the test hub gives for its worlds.
pub const REALM_HOST: &str = "play.example";
/// The game port of slot 0 of the test hub.
pub const BASE_PORT: u16 = 25000;
/// The slots of the test hub, and how many worlds it runs at once.
pub const SLOTS: u16 = 4;
pub const MAX_RUNNING: u16 = 3;

/// An agent that keeps the containers in a map and records the requests.
#[derive(Default)]
pub struct FakeAgent {
    pub requests: Mutex<Vec<AgentRequest>>,
    pub containers: Mutex<HashMap<Uuid, RealmState>>,
    /// The answer to every `start`, instead of starting.
    pub start_answer: Mutex<Option<AgentResponse>>,
    /// The answer to every `stop`, instead of stopping.
    pub stop_answer: Mutex<Option<AgentResponse>>,
    /// When set, the next `status` notifies the first and waits for the second before it
    /// reads the containers.
    pub status_gate: Mutex<Option<(Arc<Notify>, Arc<Notify>)>>,
}

impl FakeAgent {
    pub fn requests(&self) -> Vec<AgentRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub fn container(&self, realm: Uuid) -> RealmState {
        let containers = self.containers.lock().unwrap();
        containers
            .get(&realm)
            .copied()
            .unwrap_or(RealmState::Absent)
    }

    pub fn set_container(&self, realm: Uuid, state: RealmState) {
        self.containers.lock().unwrap().insert(realm, state);
    }
}

impl RealmAgent for FakeAgent {
    fn call(&self, request: AgentRequest) -> AgentFuture<'_> {
        self.requests.lock().unwrap().push(request.clone());
        if let AgentRequest::Status { realm } = request {
            if let Some((reached, release)) = self.status_gate.lock().unwrap().take() {
                return Box::pin(async move {
                    reached.notify_one();
                    release.notified().await;
                    Ok(AgentResponse::Status {
                        state: self.container(realm),
                    })
                });
            }
        }
        let mut containers = self.containers.lock().unwrap();
        let response = match request {
            AgentRequest::Start { realm, .. } => match self.start_answer.lock().unwrap().clone() {
                Some(answer) => answer,
                None => {
                    containers.insert(realm, RealmState::Running);
                    AgentResponse::Done
                }
            },
            AgentRequest::Stop { realm } => match self.stop_answer.lock().unwrap().clone() {
                Some(answer) => answer,
                None => {
                    containers.remove(&realm);
                    AgentResponse::Done
                }
            },
            AgentRequest::Archive { realm } => {
                containers.remove(&realm);
                AgentResponse::Done
            }
            AgentRequest::Status { realm } => AgentResponse::Status {
                state: containers
                    .get(&realm)
                    .copied()
                    .unwrap_or(RealmState::Absent),
            },
            AgentRequest::Purge => AgentResponse::Purged {
                volumes: Vec::new(),
            },
        };
        Box::pin(async move { Ok(response) })
    }
}

/// Worlds that answer on their status port, with their players, by status port.
#[derive(Default)]
pub struct FakeProbe {
    pub players: Mutex<HashMap<u16, u32>>,
}

impl FakeProbe {
    /// Makes the world of `slot` answer with `players`, or stop answering.
    pub fn answer(&self, slot: u16, players: Option<u32>) {
        let port = BASE_PORT + 2 * slot + 1;
        let mut answers = self.players.lock().unwrap();
        match players {
            Some(players) => answers.insert(port, players),
            None => answers.remove(&port),
        };
    }
}

impl StatusProbe for FakeProbe {
    fn players(&self, port: u16) -> ProbeFuture<'_> {
        let players = self.players.lock().unwrap().get(&port).copied();
        Box::pin(async move { players })
    }
}

pub struct TestHub {
    pub app: Router,
    pub state: AppState,
    pub agent: Arc<FakeAgent>,
    pub probe: Arc<FakeProbe>,
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
        let agent = Arc::new(FakeAgent::default());
        let probe = Arc::new(FakeProbe::default());
        let realms = Hosting::new(
            agent.clone(),
            probe.clone(),
            REALM_HOST.into(),
            BASE_PORT,
            SLOTS,
            MAX_RUNNING,
        )
        .unwrap();
        let state = AppState {
            db,
            name: "Test hub".into(),
            public_url: "https://hub.example".into(),
            signing_key: Arc::new(SigningKey::generate(&mut OsRng)),
            ip_secret: Arc::from(vec![7u8; 32]),
            trusted_ranges: Arc::from([]),
            trusted_proxies: Arc::from([
                "127.0.0.1/32".parse().unwrap(),
                "::1/128".parse().unwrap(),
            ]),
            hasher: Default::default(),
            logins: Default::default(),
            admin_logins: Default::default(),
            signups: Default::default(),
            certificates: Default::default(),
            realms: Arc::new(realms),
            admin_ui: None,
        };
        Self {
            app: router(state.clone()),
            state,
            agent,
            probe,
            admin_options,
            database,
        }
    }

    pub async fn get(&self, path: &str) -> Response<Body> {
        let request = Request::get(path).body(Body::empty()).unwrap();
        self.app.clone().oneshot(request).await.unwrap()
    }

    /// A GET with this `Authorization` header.
    pub async fn get_with(&self, path: &str, authorization: &str) -> Response<Body> {
        let request = Request::get(path)
            .header(header::AUTHORIZATION, authorization)
            .body(Body::empty())
            .unwrap();
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
