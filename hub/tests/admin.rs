mod common;

use axum::{body::Body, http::Response};
use common::TestHub;
use http_body_util::BodyExt;
use rustcraft_hub::{admin, config::RoleArg};
use serde_json::{json, Value};

const PASSWORD: &str = "correct horse battery staple";
const PEER: &str = "203.0.113.1:4000";

async fn body(response: Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn new_account(hub: &TestHub, username: &str) -> String {
    let response = hub
        .send(
            "POST",
            "/v1/accounts",
            PEER,
            None,
            Some(json!({"username": username, "password": PASSWORD})),
        )
        .await;
    assert_eq!(response.status(), 201);
    body(response).await["session"].as_str().unwrap().to_owned()
}

async fn log_in(hub: &TestHub, username: &str, password: &str) -> Response<Body> {
    hub.send(
        "POST",
        "/v1/sessions",
        PEER,
        None,
        Some(json!({"username": username, "password": password})),
    )
    .await
}

async fn me(hub: &TestHub, token: &str) -> Response<Body> {
    hub.send("GET", "/v1/me", PEER, Some(token), None).await
}

async fn audit(hub: &TestHub) -> Vec<(String, String)> {
    sqlx::query_as("SELECT actor, action FROM audit_log WHERE actor = 'admin-cli' ORDER BY id")
        .fetch_all(&hub.state.db)
        .await
        .unwrap()
}

#[tokio::test]
async fn granting_and_revoking_roles_changes_what_me_returns() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    let token = new_account(&hub, "Alex").await;

    admin::grant(db, "alex", RoleArg::Host, None).await.unwrap();
    let info = body(me(&hub, &token).await).await;
    assert_eq!(info["roles"], json!(["host"]));
    assert_eq!(info["realm_quota"], 1);

    admin::grant(db, "Alex", RoleArg::Host, Some(3))
        .await
        .unwrap();
    admin::grant(db, "Alex", RoleArg::Admin, None)
        .await
        .unwrap();
    let info = body(me(&hub, &token).await).await;
    assert_eq!(info["roles"], json!(["admin", "host"]));
    assert_eq!(info["realm_quota"], 3);

    admin::revoke(db, "Alex", RoleArg::Host).await.unwrap();
    assert_eq!(
        body(me(&hub, &token).await).await["roles"],
        json!(["admin"])
    );
    assert!(admin::revoke(db, "Alex", RoleArg::Host).await.is_err());
    assert!(admin::grant(db, "Alex", RoleArg::Admin, Some(2))
        .await
        .is_err());
    assert!(admin::grant(db, "Nobody", RoleArg::Host, None)
        .await
        .is_err());

    let actions: Vec<_> = audit(&hub).await.into_iter().map(|(_, a)| a).collect();
    assert_eq!(
        actions,
        [
            "role_granted",
            "role_granted",
            "role_granted",
            "role_revoked"
        ]
    );
}

#[tokio::test]
async fn suspending_ends_access_and_unsuspending_restores_login() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    let token = new_account(&hub, "Alex").await;
    let cert = hub
        .send(
            "POST",
            "/v1/certificates",
            PEER,
            Some(&token),
            Some(json!({"device_key": hex::encode(
                ed25519_dalek::SigningKey::from_bytes(&[5; 32]).verifying_key().to_bytes()
            )})),
        )
        .await;
    assert_eq!(cert.status(), 200);

    assert!(admin::suspend(db, "Alex", "  ").await.is_err());
    admin::suspend(db, "Alex", "griefing").await.unwrap();
    assert!(admin::suspend(db, "Alex", "again").await.is_err());
    assert_eq!(me(&hub, &token).await.status(), 401, "sessions are gone");
    let login = log_in(&hub, "Alex", PASSWORD).await;
    assert_eq!(login.status(), 403);
    assert!(body(login).await["message"]
        .as_str()
        .unwrap()
        .contains("griefing"));
    let revoked = body(hub.get("/v1/revoked").await).await;
    assert_eq!(revoked["ids"].as_array().unwrap().len(), 1);

    admin::unsuspend(db, "Alex").await.unwrap();
    assert!(admin::unsuspend(db, "Alex").await.is_err());
    assert_eq!(log_in(&hub, "Alex", PASSWORD).await.status(), 200);
    let reason: Option<String> =
        sqlx::query_scalar("SELECT status_reason FROM accounts WHERE username = 'Alex'")
            .fetch_one(db)
            .await
            .unwrap();
    assert_eq!(reason, None);
    let actions: Vec<_> = audit(&hub).await.into_iter().map(|(_, a)| a).collect();
    assert_eq!(actions, ["suspended", "unsuspended"]);
}

#[tokio::test]
async fn resetting_a_password_prints_a_new_one_and_ends_the_sessions() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    let token = new_account(&hub, "Alex").await;

    let message = admin::reset_password(db, "alex").await.unwrap();
    let password = message.rsplit(' ').next().unwrap();
    assert_eq!(password.chars().count(), 16);
    assert_eq!(me(&hub, &token).await.status(), 401);
    assert_eq!(log_in(&hub, "Alex", PASSWORD).await.status(), 401);
    assert_eq!(log_in(&hub, "Alex", password).await.status(), 200);
    assert!(admin::reset_password(db, "Nobody").await.is_err());
    assert_eq!(
        audit(&hub).await,
        [("admin-cli".to_owned(), "password_reset".to_owned())]
    );
}

#[tokio::test]
async fn the_users_table_lists_roles_quota_and_status() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    new_account(&hub, "Alex").await;
    new_account(&hub, "Sam").await;
    admin::grant(db, "Alex", RoleArg::Host, Some(2))
        .await
        .unwrap();
    admin::grant(db, "Alex", RoleArg::Admin, None)
        .await
        .unwrap();
    admin::suspend(db, "Sam", "spam").await.unwrap();

    let table = admin::users(db).await.unwrap();
    let rows: Vec<Vec<&str>> = table
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(rows[0], ["NAME", "ROLES", "QUOTA", "STATUS", "CREATED"]);
    assert_eq!(&rows[1][..4], ["Alex", "admin,host", "2", "active"]);
    assert_eq!(&rows[2][..4], ["Sam", "-", "0", "suspended"]);
}
