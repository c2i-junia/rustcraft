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
    assert_eq!(document.protocol_id, rustcraft_hub::PROTOCOL_ID);
    assert_eq!(document.keys.len(), 1);
    assert_eq!(document.keys[0].id, 1);
    assert_eq!(
        document.keys[0].public_key,
        key_to_hex(&hub.state.signing_key.verifying_key().to_bytes())
    );
    hub.finish().await;
}

#[tokio::test]
async fn healthz_answers_while_the_database_does() {
    let hub = TestHub::start().await;
    assert_eq!(hub.get("/healthz").await.status(), 200);
    hub.finish().await;
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
    hub.finish().await;
}
