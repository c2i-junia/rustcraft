mod common;

use axum::{body::Body, http::Response};
use common::TestHub;
use futures::future::join_all;
use http_body_util::BodyExt;
use rustcraft_hub::{
    maintenance::purge_deleted_accounts,
    ratelimit::{check_signup, ip_key, purge_signups, record_signup},
};
use rustcraft_protocol::api::ErrorCode;
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const NEW_PASSWORD: &str = "violet tractor under moonlight";
const PEER: &str = "203.0.113.1:4000";

async fn body(response: Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// The status and error code of a refusal.
async fn refusal(response: Response<Body>) -> (u16, String) {
    let status = response.status().as_u16();
    let body = body(response).await;
    (status, body["error"].as_str().unwrap().to_owned())
}

async fn sign_up(hub: &TestHub, username: &str, peer: &str) -> Response<Body> {
    hub.send(
        "POST",
        "/v1/accounts",
        peer,
        None,
        Some(json!({"username": username, "password": PASSWORD})),
    )
    .await
}

/// Signs up and returns the body of the answer.
async fn new_account(hub: &TestHub, username: &str) -> Value {
    let response = sign_up(hub, username, PEER).await;
    assert_eq!(response.status(), 201);
    body(response).await
}

async fn log_in(hub: &TestHub, username: &str, password: &str, peer: &str) -> Response<Body> {
    hub.send(
        "POST",
        "/v1/sessions",
        peer,
        None,
        Some(json!({"username": username, "password": password})),
    )
    .await
}

async fn me(hub: &TestHub, token: &str) -> Response<Body> {
    hub.send("GET", "/v1/me", PEER, Some(token), None).await
}

async fn audit_actions(hub: &TestHub) -> Vec<String> {
    sqlx::query_scalar("SELECT action FROM audit_log ORDER BY id")
        .fetch_all(&hub.state.db)
        .await
        .unwrap()
}

async fn add_certificate(hub: &TestHub, account: &Value) -> Uuid {
    let id = Uuid::new_v4();
    let account: Uuid = account["account"]["id"].as_str().unwrap().parse().unwrap();
    sqlx::query(
        "INSERT INTO certificates (id, account_id, device_key, issued_at, expires_at) \
         VALUES ($1, $2, $3, now(), now() + interval '7 days')",
    )
    .bind(id)
    .bind(account)
    .bind(vec![0u8; 32])
    .execute(&hub.state.db)
    .await
    .unwrap();
    id
}

async fn is_revoked(hub: &TestHub, certificate: Uuid) -> bool {
    sqlx::query_scalar("SELECT revoked_at IS NOT NULL FROM certificates WHERE id = $1")
        .bind(certificate)
        .fetch_one(&hub.state.db)
        .await
        .unwrap()
}

fn token(account: &Value) -> &str {
    account["session"].as_str().unwrap()
}

#[tokio::test]
async fn each_invalid_sign_up_has_its_error_code() {
    let hub = TestHub::start().await;
    new_account(&hub, "Alex").await;
    let cases = [
        (
            json!({"username": "ab", "password": PASSWORD}),
            400,
            "invalid_username",
        ),
        (
            json!({"username": "a".repeat(21), "password": PASSWORD}),
            400,
            "invalid_username",
        ),
        (
            json!({"username": "a b c", "password": PASSWORD}),
            400,
            "invalid_username",
        ),
        (
            json!({"username": "Admin", "password": PASSWORD}),
            400,
            "invalid_username",
        ),
        (
            json!({"username": "alex", "password": PASSWORD}),
            409,
            "username_taken",
        ),
        (
            json!({"username": "Sam", "password": "too short"}),
            400,
            "invalid_password",
        ),
        (
            json!({"username": "Sam", "password": "x".repeat(129)}),
            400,
            "invalid_password",
        ),
        (
            json!({"username": "Sam", "password": "sam-is-my-name-1984"}),
            400,
            "weak_password",
        ),
        (
            json!({"username": "Sam", "password": "password123"}),
            400,
            "weak_password",
        ),
        (json!({"username": "Sam"}), 400, "invalid_request"),
        (
            json!({"username": "Sam", "password": PASSWORD, "device_name": "d".repeat(65)}),
            400,
            "invalid_request",
        ),
    ];
    for (request, status, code) in cases {
        let response = hub
            .send("POST", "/v1/accounts", PEER, None, Some(request.clone()))
            .await;
        assert_eq!(
            refusal(response).await,
            (status, code.to_owned()),
            "{request}"
        );
    }
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(&hub.state.db)
        .await
        .unwrap();
    assert_eq!(accounts, 1);
}

#[tokio::test]
async fn a_new_account_logs_in_with_its_name_in_any_case() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    assert_eq!(account["account"]["username"], "Alex");
    assert!(account["recovery_code"]
        .as_str()
        .unwrap()
        .starts_with("RCV-"));
    let response = me(&hub, token(&account)).await;
    assert_eq!(response.status(), 200);
    let me_body = body(response).await;
    assert_eq!(me_body["id"], account["account"]["id"]);
    assert_eq!(me_body["username"], "Alex");
    assert_eq!(me_body["roles"], json!([]));
    assert_eq!(me_body["realm_quota"], 0);

    let response = log_in(&hub, "ALEX", PASSWORD, PEER).await;
    assert_eq!(response.status(), 200);
    let session = body(response).await;
    assert_eq!(session["account"], account["account"]);
    assert_ne!(session["session"], account["session"]);
    assert_eq!(me(&hub, token(&session)).await.status(), 200);
    assert_eq!(audit_actions(&hub).await, ["signup", "login"]);
}

#[tokio::test]
async fn a_wrong_password_or_an_unknown_name_is_refused_alike() {
    let hub = TestHub::start().await;
    new_account(&hub, "Alex").await;
    let wrong = log_in(&hub, "Alex", "not the right password", PEER).await;
    assert_eq!(refusal(wrong).await, (401, "bad_credentials".to_owned()));
    let unknown = log_in(&hub, "Nobody", PASSWORD, PEER).await;
    assert_eq!(refusal(unknown).await, (401, "bad_credentials".to_owned()));
    assert_eq!(
        audit_actions(&hub).await,
        ["signup", "login_failed", "login_failed"]
    );
}

#[tokio::test]
async fn the_eleventh_sign_up_from_an_address_in_an_hour_is_refused() {
    let hub = TestHub::start().await;
    for i in 0..10 {
        let response = sign_up(&hub, &format!("player{i}"), "203.0.113.5:4000").await;
        assert_eq!(response.status(), 201);
    }
    let response = sign_up(&hub, "player10", "203.0.113.5:5000").await;
    let retry_after: u64 = response.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!((3590..=3600).contains(&retry_after), "{retry_after}");
    assert_eq!(refusal(response).await, (429, "rate_limited".to_owned()));
    let other = sign_up(&hub, "player10", "203.0.113.6:4000").await;
    assert_eq!(other.status(), 201);
}

#[tokio::test]
async fn an_ipv6_neighbour_in_the_same_64_counts_with_it() {
    let hub = TestHub::start().await;
    for i in 0..10 {
        let response = sign_up(&hub, &format!("player{i}"), "[2001:db8:1:2::1]:4000").await;
        assert_eq!(response.status(), 201);
    }
    let neighbour = sign_up(&hub, "player10", "[2001:db8:1:2:ab::cd]:4000").await;
    assert_eq!(refusal(neighbour).await, (429, "rate_limited".to_owned()));
    let elsewhere = sign_up(&hub, "player10", "[2001:db8:1:3::1]:4000").await;
    assert_eq!(elsewhere.status(), 201);
}

#[tokio::test]
async fn sign_ups_are_limited_per_day_per_trusted_address_and_on_the_whole_hub() {
    let hub = TestHub::start().await;
    let mut conn = hub.state.db.acquire().await.unwrap();
    let now = OffsetDateTime::now_utc();
    let key = |n: u8| ip_key(&hub.state.ip_secret, [203, 0, 113, n].into());

    // 30 over the last 20 hours, one of them in the last hour.
    for i in 1..=30 {
        record_signup(&mut conn, &key(1), now - Duration::minutes(40 * i))
            .await
            .unwrap();
    }
    let error = check_signup(&mut conn, &key(1), false, now)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);
    assert_eq!(error.retry_after, Some(4 * 3600));
    check_signup(&mut conn, &key(1), false, now + Duration::hours(4))
        .await
        .unwrap();
    // A trusted address has no daily limit, but 60 an hour.
    check_signup(&mut conn, &key(1), true, now).await.unwrap();
    for _ in 0..59 {
        record_signup(&mut conn, &key(1), now - Duration::minutes(1))
            .await
            .unwrap();
    }
    let error = check_signup(&mut conn, &key(1), true, now)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::RateLimited);

    // With those 60, 140 more make 200 in the last hour: the hub closes to everyone, until the
    // oldest of them, 40 minutes old, leaves the hour.
    for i in 0..140 {
        record_signup(&mut conn, &key(2 + i % 100), now - Duration::minutes(30))
            .await
            .unwrap();
    }
    let error = check_signup(&mut conn, &key(200), true, now)
        .await
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::SignupsClosed);
    assert_eq!(error.retry_after, Some(20 * 60));
    check_signup(&mut conn, &key(200), true, now + Duration::minutes(20))
        .await
        .unwrap();
}

#[tokio::test]
async fn sign_ups_are_forgotten_after_30_days() {
    let hub = TestHub::start().await;
    let mut conn = hub.state.db.acquire().await.unwrap();
    let now = OffsetDateTime::now_utc();
    let key = ip_key(&hub.state.ip_secret, [203, 0, 113, 1].into());
    record_signup(&mut conn, &key, now - Duration::days(31))
        .await
        .unwrap();
    record_signup(&mut conn, &key, now - Duration::days(29))
        .await
        .unwrap();
    assert_eq!(purge_signups(&mut conn, now).await.unwrap(), 1);
    assert_eq!(purge_signups(&mut conn, now).await.unwrap(), 0);
}

#[tokio::test]
async fn the_sixth_failed_login_for_a_name_is_refused() {
    let hub = TestHub::start().await;
    new_account(&hub, "Alex").await;
    for _ in 0..5 {
        let response = log_in(&hub, "Alex", "not the right password", PEER).await;
        assert_eq!(response.status(), 401);
    }
    let response = log_in(&hub, "alex", PASSWORD, PEER).await;
    assert!(response.headers().contains_key("retry-after"));
    assert_eq!(refusal(response).await, (429, "rate_limited".to_owned()));
    let elsewhere = log_in(&hub, "Alex", PASSWORD, "198.51.100.9:4000").await;
    assert_eq!(elsewhere.status(), 200);
}

/// The statuses of the answers, sorted.
fn statuses(responses: &[Response<Body>]) -> Vec<u16> {
    let mut statuses: Vec<u16> = responses.iter().map(|r| r.status().as_u16()).collect();
    statuses.sort();
    statuses
}

#[tokio::test]
async fn parallel_wrong_logins_count_before_they_fail() {
    let hub = TestHub::start().await;
    new_account(&hub, "Alex").await;
    let responses =
        join_all((0..12).map(|_| log_in(&hub, "Alex", "not the right password", PEER))).await;
    assert_eq!(
        statuses(&responses),
        [[401; 5].as_slice(), &[429; 7]].concat()
    );
}

#[tokio::test]
async fn parallel_sign_ups_from_one_address_stop_at_its_limit() {
    let hub = TestHub::start().await;
    let names: Vec<String> = (0..15).map(|i| format!("player{i}")).collect();
    let responses = join_all(names.iter().map(|name| sign_up(&hub, name, PEER))).await;
    assert_eq!(
        statuses(&responses),
        [[201; 10].as_slice(), &[429; 5]].concat()
    );
}

#[tokio::test]
async fn recovery_gives_a_new_code_and_the_old_one_stops_working() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    let certificate = add_certificate(&hub, &account).await;
    let old_code = account["recovery_code"].as_str().unwrap();
    let recover = |code: String, password: &'static str| {
        hub.send(
            "POST",
            "/v1/recovery",
            PEER,
            None,
            Some(json!({"username": "alex", "recovery_code": code, "new_password": password})),
        )
    };

    let typed = old_code.to_lowercase().replace('-', " ");
    let response = recover(typed.clone(), NEW_PASSWORD).await;
    assert_eq!(response.status(), 200);
    let recovered = body(response).await;
    assert_eq!(recovered["account"], account["account"]);
    let new_code = recovered["recovery_code"].as_str().unwrap();
    assert_ne!(new_code, old_code);
    assert_eq!(me(&hub, token(&recovered)).await.status(), 200);
    assert_eq!(me(&hub, token(&account)).await.status(), 401);
    assert!(is_revoked(&hub, certificate).await);

    let again = recover(typed, "another fine passphrase").await;
    assert_eq!(refusal(again).await, (401, "bad_credentials".to_owned()));
    assert_eq!(log_in(&hub, "Alex", PASSWORD, PEER).await.status(), 401);
    assert_eq!(log_in(&hub, "Alex", NEW_PASSWORD, PEER).await.status(), 200);
    assert_eq!(
        recover(new_code.to_owned(), "another fine passphrase")
            .await
            .status(),
        200
    );
    let weak = recover("RCV-00000-00000-00000".into(), "password123").await;
    assert_eq!(refusal(weak).await, (400, "weak_password".to_owned()));
    assert!(audit_actions(&hub)
        .await
        .contains(&"recovery_used".to_owned()));
}

#[tokio::test]
async fn changing_the_password_logs_out_the_other_sessions() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    let other = body(log_in(&hub, "Alex", PASSWORD, PEER).await).await;
    let certificate = add_certificate(&hub, &account).await;
    let change = |current: &'static str, new: &'static str| {
        hub.send(
            "PUT",
            "/v1/me/password",
            PEER,
            Some(token(&account)),
            Some(json!({"current_password": current, "new_password": new})),
        )
    };

    let wrong = change("not the right password", NEW_PASSWORD).await;
    assert_eq!(refusal(wrong).await, (401, "bad_credentials".to_owned()));
    let weak = change(PASSWORD, "alex-alex-alex-alex").await;
    assert_eq!(refusal(weak).await, (400, "weak_password".to_owned()));
    assert_eq!(me(&hub, token(&other)).await.status(), 200);

    assert_eq!(change(PASSWORD, NEW_PASSWORD).await.status(), 204);
    assert_eq!(me(&hub, token(&account)).await.status(), 200);
    assert_eq!(me(&hub, token(&other)).await.status(), 401);
    assert!(is_revoked(&hub, certificate).await);
    assert_eq!(log_in(&hub, "Alex", PASSWORD, PEER).await.status(), 401);
    assert_eq!(log_in(&hub, "Alex", NEW_PASSWORD, PEER).await.status(), 200);
    assert!(audit_actions(&hub)
        .await
        .contains(&"password_changed".to_owned()));
}

#[tokio::test]
async fn a_deleted_account_cannot_log_in_and_its_name_is_freed_after_30_days() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    let certificate = add_certificate(&hub, &account).await;
    let delete = |password: &'static str| {
        hub.send(
            "DELETE",
            "/v1/me",
            PEER,
            Some(token(&account)),
            Some(json!({"password": password})),
        )
    };

    let wrong = delete("not the right password").await;
    assert_eq!(refusal(wrong).await, (401, "bad_credentials".to_owned()));
    assert_eq!(delete(PASSWORD).await.status(), 204);
    assert_eq!(me(&hub, token(&account)).await.status(), 401);
    assert!(is_revoked(&hub, certificate).await);
    let login = log_in(&hub, "Alex", PASSWORD, PEER).await;
    assert_eq!(refusal(login).await, (401, "bad_credentials".to_owned()));
    let again = sign_up(&hub, "alex", PEER).await;
    assert_eq!(refusal(again).await, (409, "username_taken".to_owned()));
    assert!(audit_actions(&hub)
        .await
        .contains(&"account_deleted".to_owned()));

    let mut conn = hub.state.db.acquire().await.unwrap();
    let now = OffsetDateTime::now_utc();
    assert_eq!(
        purge_deleted_accounts(&mut conn, now + Duration::days(29))
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        purge_deleted_accounts(&mut conn, now + Duration::days(31))
            .await
            .unwrap(),
        1
    );
    let name: String = sqlx::query_scalar("SELECT username::text FROM accounts")
        .fetch_one(&mut *conn)
        .await
        .unwrap();
    assert_eq!(
        name,
        format!("deleted-{}", account["account"]["id"].as_str().unwrap())
    );
    assert_eq!(sign_up(&hub, "alex", PEER).await.status(), 201);
}

#[tokio::test]
async fn logging_out_ends_this_session_or_all_of_them() {
    let hub = TestHub::start().await;
    let first = new_account(&hub, "Alex").await;
    let second = body(log_in(&hub, "Alex", PASSWORD, PEER).await).await;
    let third = body(log_in(&hub, "Alex", PASSWORD, PEER).await).await;
    let log_out = |path, account| hub.send("DELETE", path, PEER, Some(token(account)), None);

    assert_eq!(log_out("/v1/sessions/current", &first).await.status(), 204);
    assert_eq!(me(&hub, token(&first)).await.status(), 401);
    assert_eq!(me(&hub, token(&second)).await.status(), 200);
    assert_eq!(log_out("/v1/sessions", &second).await.status(), 204);
    assert_eq!(me(&hub, token(&second)).await.status(), 401);
    assert_eq!(me(&hub, token(&third)).await.status(), 401);
}

#[tokio::test]
async fn a_session_is_extended_by_its_use_and_ends_when_it_expires() {
    let hub = TestHub::start().await;
    let account = new_account(&hub, "Alex").await;
    let set_expiry = |interval: &'static str| {
        sqlx::query("UPDATE sessions SET expires_at = now() + $1::interval")
            .bind(interval)
            .execute(&hub.state.db)
    };

    set_expiry("1 day").await.unwrap();
    assert_eq!(me(&hub, token(&account)).await.status(), 200);
    let extended: bool =
        sqlx::query_scalar("SELECT expires_at > now() + interval '89 days' FROM sessions")
            .fetch_one(&hub.state.db)
            .await
            .unwrap();
    assert!(extended);

    set_expiry("-1 second").await.unwrap();
    let expired = me(&hub, token(&account)).await;
    assert_eq!(refusal(expired).await, (401, "unauthorized".to_owned()));
    for bad in ["", "not-base64!", "AAAA"] {
        assert_eq!(me(&hub, bad).await.status(), 401, "{bad}");
    }
    let stored: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM sessions")
        .fetch_one(&hub.state.db)
        .await
        .unwrap();
    assert_eq!(stored.len(), 32);
    assert_ne!(
        stored,
        base64::Engine::decode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            token(&account)
        )
        .unwrap(),
        "the hub keeps only a hash of the token"
    );
}
