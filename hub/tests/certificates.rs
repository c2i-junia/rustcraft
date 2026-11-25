mod common;

use axum::{body::Body, http::Response};
use common::TestHub;
use ed25519_dalek::SigningKey;
use http_body_util::BodyExt;
use rand::rngs::OsRng;
use rustcraft_hub::{admin, SIGNING_KEY_ID};
use rustcraft_protocol::{
    api::{CertificateResponse, Profile, Revoked, WellKnown},
    identity::{key_from_hex, key_to_hex, CERTIFICATE_LIFETIME_SECS},
};
use serde_json::{json, Value};
use time::OffsetDateTime;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const NEW_PASSWORD: &str = "violet tractor under moonlight";
const PEER: &str = "203.0.113.1:4000";

async fn body(response: Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn new_account(hub: &TestHub, username: &str) -> Value {
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
    body(response).await
}

fn token(account: &Value) -> &str {
    account["session"].as_str().unwrap()
}

fn device_key() -> String {
    key_to_hex(&SigningKey::generate(&mut OsRng).verifying_key().to_bytes())
}

async fn certify(hub: &TestHub, token: &str, device_key: &str) -> Response<Body> {
    hub.send(
        "POST",
        "/v1/certificates",
        PEER,
        Some(token),
        Some(json!({"device_key": device_key})),
    )
    .await
}

/// Issues a certificate and returns its id.
async fn certificate_of(hub: &TestHub, token: &str) -> Uuid {
    let response = certify(hub, token, &device_key()).await;
    assert_eq!(response.status(), 200);
    let response: CertificateResponse = serde_json::from_value(body(response).await).unwrap();
    let signed = response.signed_certificate().unwrap();
    let keys = [(SIGNING_KEY_ID, hub.state.signing_key.verifying_key())];
    let now = OffsetDateTime::now_utc().unix_timestamp() as u64;
    signed.verify(&keys, now).unwrap().id
}

async fn revoked(hub: &TestHub) -> Vec<Uuid> {
    let response = hub.get("/v1/revoked").await;
    assert_eq!(response.status(), 200);
    serde_json::from_value::<Revoked>(body(response).await)
        .unwrap()
        .ids
}

#[tokio::test]
async fn a_certificate_from_the_api_verifies_with_the_well_known_key() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    let device = device_key();
    let before = OffsetDateTime::now_utc().unix_timestamp() as u64;

    let response = certify(&hub, token(&account), &device).await;
    assert_eq!(response.status(), 200);
    let response: CertificateResponse = serde_json::from_value(body(response).await).unwrap();

    let well_known: WellKnown =
        serde_json::from_value(body(hub.get("/.well-known/rustcraft-hub").await).await).unwrap();
    let keys: Vec<_> = well_known
        .keys
        .iter()
        .map(|key| {
            let bytes = key_from_hex(&key.public_key).unwrap();
            (
                key.id,
                ed25519_dalek::VerifyingKey::from_bytes(&bytes).unwrap(),
            )
        })
        .collect();
    let signed = response.signed_certificate().unwrap();
    let cert = signed.verify(&keys, before).unwrap();
    assert_eq!(cert.hub, "https://hub.example");
    assert_eq!(cert.account.to_string(), account["account"]["id"]);
    assert_eq!(cert.name, "Alex");
    assert_eq!(key_to_hex(&cert.device_key), device);
    assert_eq!(cert.expires_at - cert.issued_at, CERTIFICATE_LIFETIME_SECS);
    assert_eq!(cert.expires_at, response.expires_at);
    assert!(cert.issued_at >= before);

    let stored: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM certificates WHERE id = $1 AND revoked_at IS NULL AND device_key = $2",
    )
    .bind(cert.id)
    .bind(&cert.device_key[..])
    .fetch_one(&hub.state.db)
    .await
    .unwrap();
    assert_eq!(stored, 1);
}

#[tokio::test]
async fn a_certificate_needs_a_session_and_a_device_key() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    let anonymous = hub
        .send(
            "POST",
            "/v1/certificates",
            PEER,
            None,
            Some(json!({"device_key": device_key()})),
        )
        .await;
    assert_eq!(anonymous.status(), 401);
    for bad in [
        "",
        "zz",
        &"ab".repeat(31),
        &"ab".repeat(33),
        &"zz".repeat(32),
    ] {
        let response = certify(&hub, token(&account), bad).await;
        assert_eq!(response.status(), 400, "{bad}");
        assert_eq!(body(response).await["error"], "invalid_request");
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM certificates")
        .fetch_one(&hub.state.db)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn an_account_gets_thirty_certificates_an_hour() {
    let hub = TestHub::start().await;
    let alex = new_account(&hub, "Alex").await;
    let sam = new_account(&hub, "Sam").await;
    for _ in 0..30 {
        assert_eq!(
            certify(&hub, token(&alex), &device_key()).await.status(),
            200
        );
    }
    let refused = certify(&hub, token(&alex), &device_key()).await;
    assert_eq!(refused.status(), 429);
    assert!(refused.headers().contains_key("retry-after"));
    assert_eq!(body(refused).await["error"], "rate_limited");
    assert_eq!(
        certify(&hub, token(&sam), &device_key()).await.status(),
        200
    );
}

#[tokio::test]
async fn revoked_ids_appear_after_a_password_change_and_vanish_when_expired() {
    let hub = TestHub::start().await;
    let alex = new_account(&hub, "Alex").await;
    let sam = new_account(&hub, "Sam").await;
    let alex_cert = certificate_of(&hub, token(&alex)).await;
    let sam_cert = certificate_of(&hub, token(&sam)).await;
    assert!(revoked(&hub).await.is_empty());

    let changed = hub
        .send(
            "PUT",
            "/v1/me/password",
            PEER,
            Some(token(&alex)),
            Some(json!({"current_password": PASSWORD, "new_password": NEW_PASSWORD})),
        )
        .await;
    assert_eq!(changed.status(), 204);
    assert_eq!(revoked(&hub).await, vec![alex_cert]);

    sqlx::query("UPDATE certificates SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(alex_cert)
        .execute(&hub.state.db)
        .await
        .unwrap();
    assert!(revoked(&hub).await.is_empty());

    admin::suspend(&hub.state.db, "Sam", "spam").await.unwrap();
    assert_eq!(revoked(&hub).await, vec![sam_cert]);
}

#[tokio::test]
async fn a_deleted_account_has_its_certificates_revoked() {
    let hub = TestHub::start().await;
    let alex = new_account(&hub, "Alex").await;
    let cert = certificate_of(&hub, token(&alex)).await;
    let deleted = hub
        .send(
            "DELETE",
            "/v1/me",
            PEER,
            Some(token(&alex)),
            Some(json!({"password": PASSWORD})),
        )
        .await;
    assert_eq!(deleted.status(), 204);
    assert_eq!(revoked(&hub).await, vec![cert]);
}

#[tokio::test]
async fn a_profile_is_found_by_name_for_an_active_account_only() {
    let hub = TestHub::start().await;
    let alex = new_account(&hub, "Alex").await;
    new_account(&hub, "Sam").await;
    admin::suspend(&hub.state.db, "Sam", "spam").await.unwrap();

    let found = hub.get("/v1/profiles?name=aLEX").await;
    assert_eq!(found.status(), 200);
    let profile: Profile = serde_json::from_value(body(found).await).unwrap();
    assert_eq!(profile.id.to_string(), alex["account"]["id"]);
    assert_eq!(profile.username, "Alex");

    for (path, status) in [
        ("/v1/profiles?name=Nobody", 404),
        ("/v1/profiles?name=Sam", 404),
        ("/v1/profiles", 400),
    ] {
        assert_eq!(hub.get(path).await.status(), status, "{path}");
    }
}

#[tokio::test]
async fn a_suspended_account_is_told_why_at_login_and_on_every_route() {
    let hub = TestHub::start().await;
    let alex = new_account(&hub, "Alex").await;
    sqlx::query("UPDATE accounts SET status = 'suspended', status_reason = 'cheating'")
        .execute(&hub.state.db)
        .await
        .unwrap();

    let login = |password: &'static str| {
        hub.send(
            "POST",
            "/v1/sessions",
            PEER,
            None,
            Some(json!({"username": "Alex", "password": password})),
        )
    };
    let response = login(PASSWORD).await;
    assert_eq!(response.status(), 403);
    let error = body(response).await;
    assert_eq!(error["error"], "forbidden");
    assert!(error["message"].as_str().unwrap().contains("cheating"));
    // Only the password's owner learns the reason.
    let wrong = login("not the password at all").await;
    assert_eq!(wrong.status(), 401);

    // A session that outlived the suspension is refused the same way.
    for (method, path, request) in [
        (
            "POST",
            "/v1/certificates",
            Some(json!({"device_key": device_key()})),
        ),
        ("GET", "/v1/me", None),
    ] {
        let response = hub
            .send(method, path, PEER, Some(token(&alex)), request)
            .await;
        assert_eq!(response.status(), 403, "{path}");
        assert!(body(response).await["message"]
            .as_str()
            .unwrap()
            .contains("cheating"));
    }
}
