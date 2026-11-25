mod common;

use axum::{body::Body, http::Response};
use common::TestHub;
use http_body_util::BodyExt;
use rustcraft_protocol::{
    api::{WellKnown, API_VERSION},
    identity::key_to_hex,
};

async fn json<T: serde::de::DeserializeOwned>(response: Response<Body>) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn the_well_known_document_lists_the_key_and_the_protocol_id() {
    let hub = TestHub::start().await;
    let response = hub.get("/.well-known/rustcraft-hub").await;
    assert_eq!(response.status(), 200);
    let document: WellKnown = json(response).await;
    assert_eq!(document.name, "Test hub");
    assert_eq!(document.url, "https://hub.example");
    assert_eq!(document.api_version, API_VERSION);
    assert_eq!(document.protocol_id, rustcraft_protocol::PROTOCOL_ID);
    assert_eq!(document.keys.len(), 1);
    assert_eq!(document.keys[0].id, 1);
    assert_eq!(
        document.keys[0].public_key,
        key_to_hex(&hub.state.signing_key.verifying_key().to_bytes())
    );
}

#[tokio::test]
async fn healthz_answers_while_the_database_does() {
    let hub = TestHub::start().await;
    assert_eq!(hub.get("/healthz").await.status(), 200);
}

#[tokio::test]
async fn the_migration_creates_the_account_tables() {
    let hub = TestHub::start().await;
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name::text FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations' ORDER BY 1",
    )
    .fetch_all(&hub.state.db)
    .await
    .unwrap();
    assert_eq!(
        tables,
        [
            "account_identities",
            "account_roles",
            "accounts",
            "audit_log",
            "certificates",
            "password_credentials",
            "sessions",
            "signups"
        ]
    );
}

#[tokio::test]
async fn usernames_are_unique_whatever_their_case() {
    let hub = TestHub::start().await;
    let insert = |name: &'static str| {
        sqlx::query("INSERT INTO accounts (id, username, display_name) VALUES ($1, $2, $2)")
            .bind(uuid::Uuid::new_v4())
            .bind(name)
            .execute(&hub.state.db)
    };
    insert("Bob").await.unwrap();
    let error = insert("bob").await.unwrap_err();
    assert!(error
        .as_database_error()
        .is_some_and(|e| e.is_unique_violation()));
}

#[tokio::test]
async fn keys_and_hashes_must_be_32_bytes() {
    let hub = TestHub::start().await;
    let insert = sqlx::query("INSERT INTO signups (ip_key) VALUES ($1)");
    insert
        .bind(vec![0u8; 31])
        .execute(&hub.state.db)
        .await
        .unwrap_err();
    sqlx::query("INSERT INTO signups (ip_key) VALUES ($1)")
        .bind(vec![0u8; 32])
        .execute(&hub.state.db)
        .await
        .unwrap();
}
