mod common;

use std::{net::SocketAddr, time::Duration};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Request, Response},
    Router,
};
use common::TestHub;
use http_body_util::BodyExt;
use rustcraft_hub::{admin, config::RoleArg};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

const PASSWORD: &str = "correct horse battery staple";
const PEER: &str = "203.0.113.1:4000";
/// The origin of the test hub's public URL.
const ORIGIN: &str = "https://hub.example";

async fn body(response: Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// The status and error code of a refusal.
async fn refusal(response: Response<Body>) -> (u16, String) {
    let status = response.status().as_u16();
    (
        status,
        body(response).await["error"].as_str().unwrap().to_owned(),
    )
}

/// A request as the interface sends it: with `X-Rustcraft-Admin: 1`, the cookie when given, and
/// the extra headers.
async fn call(
    app: &Router,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    headers: &[(&str, &str)],
    json: Option<Value>,
) -> Response<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("x-rustcraft-admin", "1");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    for (name, value) in headers {
        request = request.header(*name, *value);
    }
    let mut request = match json {
        Some(json) => request
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(json.to_string())),
        None => request.body(Body::empty()),
    }
    .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(PEER.parse::<SocketAddr>().unwrap()));
    app.clone().oneshot(request).await.unwrap()
}

async fn get(hub: &TestHub, path: &str, cookie: &str) -> Response<Body> {
    call(&hub.app, "GET", path, Some(cookie), &[], None).await
}

async fn send(
    hub: &TestHub,
    method: &str,
    path: &str,
    cookie: &str,
    json: Option<Value>,
) -> Response<Body> {
    call(&hub.app, method, path, Some(cookie), &[], json).await
}

/// Signs up and returns the account id and the game session.
async fn new_account(hub: &TestHub, username: &str) -> (Uuid, String) {
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
    let body = body(response).await;
    (
        body["account"]["id"].as_str().unwrap().parse().unwrap(),
        body["session"].as_str().unwrap().to_owned(),
    )
}

async fn new_admin(hub: &TestHub, username: &str) -> Uuid {
    let (id, _) = new_account(hub, username).await;
    admin::grant(&hub.state.db, username, RoleArg::Admin, None)
        .await
        .unwrap();
    id
}

async fn log_in(hub: &TestHub, username: &str, password: &str) -> Response<Body> {
    call(
        &hub.app,
        "POST",
        "/v1/admin/session",
        None,
        &[],
        Some(json!({"username": username, "password": password})),
    )
    .await
}

/// Logs in and returns the `Cookie` header to send.
async fn admin_cookie(hub: &TestHub, username: &str) -> String {
    let response = log_in(hub, username, PASSWORD).await;
    assert_eq!(response.status(), 200);
    let set_cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
    set_cookie.split(';').next().unwrap().to_owned()
}

fn token_of(cookie: &str) -> &str {
    cookie.split_once('=').unwrap().1
}

/// The audit rows, oldest first, as (actor, action, account, details).
async fn audit(hub: &TestHub) -> Vec<(String, String, Option<Uuid>, Value)> {
    sqlx::query_as("SELECT actor, action, account_id, details FROM audit_log ORDER BY id")
        .fetch_all(&hub.state.db)
        .await
        .unwrap()
}

/// The last audit row.
async fn last_audit(hub: &TestHub) -> (String, String, Option<Uuid>, Value) {
    audit(hub).await.pop().unwrap()
}

#[tokio::test]
async fn an_admin_logs_in_with_a_secure_cookie_and_out() {
    let hub = TestHub::start().await;
    let id = new_admin(&hub, "Alex").await;

    let response = log_in(&hub, "alex", PASSWORD).await;
    assert_eq!(response.status(), 200);
    let set_cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(body(response).await, json!({"username": "Alex"}));
    let (cookie, attributes) = set_cookie.split_once("; ").unwrap();
    let (name, token) = cookie.split_once('=').unwrap();
    assert_eq!(name, "__Host-rustcraft-admin");
    assert_eq!(token.len(), 43, "32 bytes of base64url");
    assert_eq!(
        attributes,
        "Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=86400"
    );
    let (actor, action, account, details) = last_audit(&hub).await;
    assert_eq!(
        (actor.as_str(), action.as_str()),
        ("admin:Alex", "admin_login")
    );
    assert_eq!(account, Some(id));
    assert_eq!(details["admin_id"], id.to_string());

    let me = get(&hub, "/v1/admin/me", cookie).await;
    assert_eq!(me.status(), 200);
    assert_eq!(me.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(me.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert_eq!(
        body(me).await,
        json!({"id": id.to_string(), "username": "Alex"})
    );
    let kind: String = sqlx::query_scalar(
        "SELECT kind FROM sessions WHERE account_id = $1 \
         AND device_name IS NULL ORDER BY created_at DESC LIMIT 1",
    )
    .bind(id)
    .fetch_one(&hub.state.db)
    .await
    .unwrap();
    assert_eq!(kind, "admin_web");

    let out = send(&hub, "DELETE", "/v1/admin/session", cookie, None).await;
    assert_eq!(out.status(), 204);
    assert_eq!(
        out.headers()[header::SET_COOKIE],
        "__Host-rustcraft-admin=; Path=/; Secure; HttpOnly; SameSite=Strict; Max-Age=0"
    );
    assert_eq!(get(&hub, "/v1/admin/me", cookie).await.status(), 401);
    let (actor, action, _, _) = last_audit(&hub).await;
    assert_eq!(
        (actor.as_str(), action.as_str()),
        ("admin:Alex", "admin_logout")
    );
    // Without a session, logging out still clears the cookie.
    assert_eq!(
        send(&hub, "DELETE", "/v1/admin/session", cookie, None)
            .await
            .status(),
        204
    );
}

#[tokio::test]
async fn a_login_replaces_the_session_the_browser_held() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    let old = admin_cookie(&hub, "Alex").await;
    let response = call(
        &hub.app,
        "POST",
        "/v1/admin/session",
        Some(&old),
        &[],
        Some(json!({"username": "Alex", "password": PASSWORD})),
    )
    .await;
    assert_eq!(response.status(), 200);
    assert_eq!(get(&hub, "/v1/admin/me", &old).await.status(), 401);
}

#[tokio::test]
async fn a_player_who_is_not_an_admin_is_refused_after_a_correct_password() {
    let hub = TestHub::start().await;
    let (id, _) = new_account(&hub, "Sam").await;
    let response = log_in(&hub, "Sam", PASSWORD).await;
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    let status = response.status();
    let body = body(response).await;
    assert_eq!(status, 403);
    assert_eq!(body["error"], "forbidden");
    assert_eq!(body["message"], "This account is not an admin.");
    let (actor, action, account, _) = last_audit(&hub).await;
    assert_eq!(
        (actor.as_str(), action.as_str()),
        ("self", "admin_login_refused")
    );
    assert_eq!(account, Some(id));
    let sessions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sessions WHERE kind = 'admin_web'")
            .fetch_one(&hub.state.db)
            .await
            .unwrap();
    assert_eq!(sessions, 0);
}

#[tokio::test]
async fn wrong_passwords_are_counted_by_the_login_limiter() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    for _ in 0..5 {
        assert_eq!(
            refusal(log_in(&hub, "Alex", "not the password at all").await).await,
            (401, "bad_credentials".to_owned())
        );
    }
    let limited = log_in(&hub, "Alex", PASSWORD).await;
    assert!(limited.headers().contains_key(header::RETRY_AFTER));
    assert_eq!(refusal(limited).await, (429, "rate_limited".to_owned()));
    let failures = audit(&hub)
        .await
        .into_iter()
        .filter(|(actor, action, _, _)| actor == "anonymous" && action == "login_failed")
        .count();
    assert_eq!(failures, 5);
    // The game's login shares the limit.
    let game = hub
        .send(
            "POST",
            "/v1/sessions",
            PEER,
            None,
            Some(json!({"username": "Alex", "password": PASSWORD})),
        )
        .await;
    assert_eq!(game.status(), 429);
    // An unknown name is refused like a wrong password.
    assert_eq!(
        refusal(log_in(&hub, "Nobody", PASSWORD).await).await,
        (401, "bad_credentials".to_owned())
    );
}

#[tokio::test]
async fn a_suspended_admin_is_told_why() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    admin::suspend(&hub.state.db, "Alex", "compromised")
        .await
        .unwrap();
    assert_eq!(get(&hub, "/v1/admin/me", &cookie).await.status(), 401);
    let response = log_in(&hub, "Alex", PASSWORD).await;
    assert_eq!(response.status(), 403);
    assert!(body(response).await["message"]
        .as_str()
        .unwrap()
        .contains("compromised"));
}

#[tokio::test]
async fn game_tokens_and_admin_cookies_are_not_interchangeable() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    let game: String = {
        let response = hub
            .send(
                "POST",
                "/v1/sessions",
                PEER,
                None,
                Some(json!({"username": "Alex", "password": PASSWORD})),
            )
            .await;
        body(response).await["session"].as_str().unwrap().to_owned()
    };
    // The game token, as a bearer or in the cookie, does not open the admin API.
    let bearer = call(
        &hub.app,
        "GET",
        "/v1/admin/me",
        None,
        &[("authorization", &format!("Bearer {game}"))],
        None,
    )
    .await;
    assert_eq!(refusal(bearer).await, (401, "unauthorized".to_owned()));
    let as_cookie = format!("__Host-rustcraft-admin={game}");
    assert_eq!(get(&hub, "/v1/admin/me", &as_cookie).await.status(), 401);
    // The admin token, as a bearer, does not open the game API.
    let me = hub
        .send("GET", "/v1/me", PEER, Some(token_of(&cookie)), None)
        .await;
    assert_eq!(me.status(), 401);
    let certificate = hub
        .send(
            "POST",
            "/v1/certificates",
            PEER,
            Some(token_of(&cookie)),
            Some(json!({"device_key": "00".repeat(32)})),
        )
        .await;
    assert_eq!(certificate.status(), 401);
    // Both still work where they belong.
    assert_eq!(get(&hub, "/v1/admin/me", &cookie).await.status(), 200);
    assert_eq!(
        hub.send("GET", "/v1/me", PEER, Some(&game), None)
            .await
            .status(),
        200
    );
    // A cookie among others is found; a malformed one is not a session.
    let among = format!("theme=dark; {cookie}; other=1");
    assert_eq!(get(&hub, "/v1/admin/me", &among).await.status(), 200);
    assert_eq!(
        get(&hub, "/v1/admin/me", "__Host-rustcraft-admin=abc")
            .await
            .status(),
        401
    );
}

#[tokio::test]
async fn requests_from_other_sites_are_refused() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    let app = &hub.app;

    // A mutation without the header, even with a valid cookie.
    let mut request = Request::delete("/v1/admin/session")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(PEER.parse::<SocketAddr>().unwrap()));
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(refusal(response).await, (403, "forbidden".to_owned()));
    assert_eq!(
        get(&hub, "/v1/admin/me", &cookie).await.status(),
        200,
        "not logged out"
    );
    // A login without the header.
    let request = Request::post("/v1/admin/session")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "Alex", "password": PASSWORD}).to_string(),
        ))
        .unwrap();
    assert_eq!(app.clone().oneshot(request).await.unwrap().status(), 403);
    // A wrong value of the header.
    let request = Request::delete("/v1/admin/session")
        .header(header::COOKIE, &cookie)
        .header("x-rustcraft-admin", "true")
        .body(Body::empty())
        .unwrap();
    assert_eq!(app.clone().oneshot(request).await.unwrap().status(), 403);

    for headers in [
        [("origin", "https://evil.example")],
        [("origin", "null")],
        [("origin", "http://hub.example")],
        [("sec-fetch-site", "cross-site")],
        [("sec-fetch-site", "same-site")],
    ] {
        let response = call(app, "GET", "/v1/admin/me", Some(&cookie), &headers, None).await;
        assert_eq!(
            refusal(response).await,
            (403, "forbidden".to_owned()),
            "{headers:?}"
        );
    }
    let response = call(
        app,
        "GET",
        "/v1/admin/me",
        Some(&cookie),
        &[("origin", ORIGIN), ("sec-fetch-site", "same-origin")],
        None,
    )
    .await;
    assert_eq!(response.status(), 200);
    let response = call(
        app,
        "GET",
        "/v1/admin/me",
        Some(&cookie),
        &[("sec-fetch-site", "none")],
        None,
    )
    .await;
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn a_session_ends_after_12_idle_hours_or_24_hours_in_all() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    let id = new_admin(&hub, "Alex").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    let expiry = || async {
        let (created, expires): (time::OffsetDateTime, time::OffsetDateTime) = sqlx::query_as(
            "SELECT created_at, expires_at FROM sessions \
             WHERE account_id = $1 AND kind = 'admin_web'",
        )
        .bind(id)
        .fetch_one(db)
        .await
        .unwrap();
        (created, expires)
    };
    let (created, expires) = expiry().await;
    let idle = expires - created;
    assert!(
        idle > time::Duration::hours(12) - time::Duration::minutes(1)
            && idle <= time::Duration::hours(12)
    );

    // Used 23 hours after its creation, it lives one more hour, not 12.
    sqlx::query(
        "UPDATE sessions SET created_at = now() - interval '23 hours' \
         WHERE account_id = $1 AND kind = 'admin_web'",
    )
    .bind(id)
    .execute(db)
    .await
    .unwrap();
    assert_eq!(get(&hub, "/v1/admin/me", &cookie).await.status(), 200);
    let (created, expires) = expiry().await;
    assert!(expires - created <= time::Duration::hours(24));

    // 24 hours after its creation it ends, whatever its expiry says.
    sqlx::query(
        "UPDATE sessions SET created_at = now() - interval '24 hours 1 second', \
             expires_at = now() + interval '1 hour' \
         WHERE account_id = $1 AND kind = 'admin_web'",
    )
    .bind(id)
    .execute(db)
    .await
    .unwrap();
    assert_eq!(get(&hub, "/v1/admin/me", &cookie).await.status(), 401);

    // Idle: an expired session ends.
    let cookie = admin_cookie(&hub, "Alex").await;
    sqlx::query(
        "UPDATE sessions SET expires_at = now() - interval '1 second' \
         WHERE account_id = $1 AND kind = 'admin_web' AND created_at > now() - interval '1 hour'",
    )
    .bind(id)
    .execute(db)
    .await
    .unwrap();
    assert_eq!(get(&hub, "/v1/admin/me", &cookie).await.status(), 401);
}

#[tokio::test]
async fn revoking_the_role_ends_access_at_once() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    admin::revoke(&hub.state.db, "Alex", RoleArg::Admin)
        .await
        .unwrap();
    let response = get(&hub, "/v1/admin/overview", &cookie).await;
    let status = response.status();
    let body = body(response).await;
    assert_eq!(status, 403);
    assert_eq!(body["message"], "This account is not an admin.");
    admin::grant(&hub.state.db, "Alex", RoleArg::Admin, None)
        .await
        .unwrap();
    assert_eq!(get(&hub, "/v1/admin/overview", &cookie).await.status(), 200);
}

/// Checks that the last audit row is `action` on `account`, by the admin, and returns its details.
async fn audited(hub: &TestHub, account: Uuid, admin_id: Uuid, action: &str) -> Value {
    let (actor, last, on, details) = last_audit(hub).await;
    assert_eq!(last, action);
    assert_eq!(actor, "admin:Alex", "{action}");
    assert_eq!(on, Some(account), "{action}");
    assert_eq!(details["admin_id"], admin_id.to_string(), "{action}");
    details
}

async fn log_in_game(hub: &TestHub, username: &str, password: &str) -> u16 {
    hub.send(
        "POST",
        "/v1/sessions",
        PEER,
        None,
        Some(json!({"username": username, "password": password})),
    )
    .await
    .status()
    .as_u16()
}

#[tokio::test]
async fn every_mutation_is_done_and_audited_as_the_admin() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    let admin_id = new_admin(&hub, "Alex").await;
    let (sam, sam_token) = new_account(&hub, "Sam").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    let account = format!("/v1/admin/accounts/{sam}");

    let grant = send(
        &hub,
        "PUT",
        &format!("{account}/roles/host"),
        &cookie,
        Some(json!({"realm_quota": 3})),
    )
    .await;
    assert_eq!(grant.status(), 204);
    let details = audited(&hub, sam, admin_id, "role_granted").await;
    assert_eq!(details["role"], "host");
    assert_eq!(details["realm_quota"], 3);
    let grant = send(
        &hub,
        "PUT",
        &format!("{account}/roles/admin"),
        &cookie,
        Some(json!({})),
    )
    .await;
    assert_eq!(grant.status(), 204);
    assert_eq!(
        audited(&hub, sam, admin_id, "role_granted").await["role"],
        "admin"
    );
    assert_eq!(
        refusal(
            send(
                &hub,
                "PUT",
                &format!("{account}/roles/admin"),
                &cookie,
                Some(json!({"realm_quota": 2}))
            )
            .await
        )
        .await,
        (400, "invalid_request".to_owned())
    );
    assert_eq!(
        refusal(
            send(
                &hub,
                "PUT",
                &format!("{account}/roles/host"),
                &cookie,
                Some(json!({"realm_quota": -1}))
            )
            .await
        )
        .await,
        (400, "invalid_request".to_owned())
    );
    assert_eq!(
        send(
            &hub,
            "PUT",
            &format!("{account}/roles/owner"),
            &cookie,
            Some(json!({}))
        )
        .await
        .status(),
        404
    );
    let detail = body(get(&hub, &account, &cookie).await).await;
    assert_eq!(detail["roles"], json!(["admin", "host"]));
    assert_eq!(detail["realm_quota"], 3);

    let revoke = send(
        &hub,
        "DELETE",
        &format!("{account}/roles/admin"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(revoke.status(), 204);
    assert_eq!(
        audited(&hub, sam, admin_id, "role_revoked").await["role"],
        "admin"
    );
    assert_eq!(
        refusal(
            send(
                &hub,
                "DELETE",
                &format!("{account}/roles/admin"),
                &cookie,
                None
            )
            .await
        )
        .await,
        (400, "invalid_request".to_owned())
    );

    let quota = send(
        &hub,
        "PUT",
        &format!("{account}/quota"),
        &cookie,
        Some(json!({"realm_quota": 7})),
    )
    .await;
    assert_eq!(quota.status(), 204);
    assert_eq!(
        audited(&hub, sam, admin_id, "realm_quota_set").await["realm_quota"],
        7
    );
    for wrong in [-1, 101] {
        assert_eq!(
            send(
                &hub,
                "PUT",
                &format!("{account}/quota"),
                &cookie,
                Some(json!({"realm_quota": wrong}))
            )
            .await
            .status(),
            400
        );
    }
    assert_eq!(
        send(
            &hub,
            "PUT",
            &format!("{account}/quota"),
            &cookie,
            Some(json!({"realm_quota": 100}))
        )
        .await
        .status(),
        204
    );

    // A world of Sam's, running, to stop and archive.
    let created = hub
        .send(
            "POST",
            "/v1/realms",
            PEER,
            Some(&sam_token),
            Some(json!({"name": "Home"})),
        )
        .await;
    assert_eq!(created.status(), 201);
    let realm: Uuid = body(created).await["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        refusal(
            send(
                &hub,
                "POST",
                &format!("/v1/admin/realms/{realm}/stop"),
                &cookie,
                None
            )
            .await
        )
        .await,
        (400, "invalid_request".to_owned()),
        "a stopped world is not stopped again"
    );
    let started = hub
        .send(
            "POST",
            &format!("/v1/realms/{realm}/start"),
            PEER,
            Some(&sam_token),
            None,
        )
        .await;
    assert_eq!(started.status(), 200);
    let stop = send(
        &hub,
        "POST",
        &format!("/v1/admin/realms/{realm}/stop"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(stop.status(), 202);
    let details = audited(&hub, sam, admin_id, "realm_stopped").await;
    assert_eq!(details["realm_id"], realm.to_string());
    assert_eq!(details["name"], "Home");
    for _ in 0..200 {
        let state: String = sqlx::query_scalar("SELECT state FROM realms WHERE id = $1")
            .bind(realm)
            .fetch_one(db)
            .await
            .unwrap();
        if state == "stopped" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let archive = send(
        &hub,
        "DELETE",
        &format!("/v1/admin/realms/{realm}"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(archive.status(), 204);
    assert_eq!(
        audited(&hub, sam, admin_id, "realm_archived").await["realm_id"],
        realm.to_string()
    );
    assert_eq!(
        send(
            &hub,
            "DELETE",
            &format!("/v1/admin/realms/{realm}"),
            &cookie,
            None
        )
        .await
        .status(),
        404
    );
    assert_eq!(
        send(
            &hub,
            "POST",
            "/v1/admin/realms/not-a-uuid/stop",
            &cookie,
            None
        )
        .await
        .status(),
        404
    );

    // Ending the sessions revokes the certificates too.
    let certificate = hub
        .send(
            "POST",
            "/v1/certificates",
            PEER,
            Some(&sam_token),
            Some(json!({"device_key": hex::encode(
                ed25519_dalek::SigningKey::from_bytes(&[5; 32]).verifying_key().to_bytes()
            )})),
        )
        .await;
    assert_eq!(certificate.status(), 200);
    assert_eq!(
        body(get(&hub, &account, &cookie).await).await["active_certificates"],
        1
    );
    let ended = send(
        &hub,
        "DELETE",
        &format!("{account}/sessions"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(ended.status(), 204);
    audited(&hub, sam, admin_id, "sessions_ended").await;
    assert_eq!(
        hub.send("GET", "/v1/me", PEER, Some(&sam_token), None)
            .await
            .status(),
        401
    );
    let detail = body(get(&hub, &account, &cookie).await).await;
    assert_eq!(detail["active_certificates"], 0);
    assert_eq!(detail["sessions"], json!([]));

    // Suspension.
    for reason in ["", "   ", &"x".repeat(201)] {
        assert_eq!(
            send(
                &hub,
                "POST",
                &format!("{account}/suspend"),
                &cookie,
                Some(json!({"reason": reason}))
            )
            .await
            .status(),
            400
        );
    }
    let suspend = send(
        &hub,
        "POST",
        &format!("{account}/suspend"),
        &cookie,
        Some(json!({"reason": &"x".repeat(200)})),
    )
    .await;
    assert_eq!(suspend.status(), 204);
    assert_eq!(
        audited(&hub, sam, admin_id, "suspended").await["reason"],
        "x".repeat(200)
    );
    assert_eq!(
        send(
            &hub,
            "POST",
            &format!("{account}/suspend"),
            &cookie,
            Some(json!({"reason": "again"}))
        )
        .await
        .status(),
        400
    );
    let detail = body(get(&hub, &account, &cookie).await).await;
    assert_eq!(detail["status"], "suspended");
    assert_eq!(detail["status_reason"], "x".repeat(200));
    let unsuspend = send(&hub, "POST", &format!("{account}/unsuspend"), &cookie, None).await;
    assert_eq!(unsuspend.status(), 204);
    audited(&hub, sam, admin_id, "unsuspended").await;
    assert_eq!(
        send(&hub, "POST", &format!("{account}/unsuspend"), &cookie, None)
            .await
            .status(),
        400
    );

    // A reset password works once shown, and the old one no more.
    let reset = send(
        &hub,
        "POST",
        &format!("{account}/reset-password"),
        &cookie,
        None,
    )
    .await;
    assert_eq!(reset.status(), 200);
    let password = body(reset).await["password"].as_str().unwrap().to_owned();
    assert_eq!(password.chars().count(), 16);
    audited(&hub, sam, admin_id, "password_reset").await;
    assert_eq!(log_in_game(&hub, "Sam", PASSWORD).await, 401);
    assert_eq!(log_in_game(&hub, "Sam", &password).await, 200);

    // An unknown account.
    let nobody = format!("/v1/admin/accounts/{}/unsuspend", Uuid::new_v4());
    assert_eq!(
        refusal(send(&hub, "POST", &nobody, &cookie, None).await).await,
        (404, "not_found".to_owned())
    );
    assert_eq!(
        get(&hub, "/v1/admin/accounts/nope", &cookie).await.status(),
        404
    );

    // Unknown fields are refused.
    assert_eq!(
        send(
            &hub,
            "PUT",
            &format!("{account}/quota"),
            &cookie,
            Some(json!({"realm_quota": 1, "x": 1}))
        )
        .await
        .status(),
        400
    );
    // Without a session, nothing.
    assert_eq!(
        call(
            &hub.app,
            "POST",
            &format!("{account}/unsuspend"),
            None,
            &[],
            None
        )
        .await
        .status(),
        401
    );
}

#[tokio::test]
async fn an_admin_cannot_lock_themselves_out() {
    let hub = TestHub::start().await;
    let id = new_admin(&hub, "Alex").await;
    let cookie = admin_cookie(&hub, "Alex").await;
    let me = format!("/v1/admin/accounts/{id}");
    for (method, path, json) in [
        ("DELETE", format!("{me}/roles/admin"), None),
        (
            "POST",
            format!("{me}/suspend"),
            Some(json!({"reason": "oops"})),
        ),
        ("POST", format!("{me}/reset-password"), None),
        ("DELETE", format!("{me}/sessions"), None),
    ] {
        let response = send(&hub, method, &path, &cookie, json).await;
        let status = response.status();
        let body = body(response).await;
        assert_eq!(status, 403, "{path}");
        assert_eq!(body["error"], "forbidden");
        assert!(body["message"].as_str().unwrap().starts_with("You cannot"));
    }
    assert_eq!(get(&hub, "/v1/admin/me", &cookie).await.status(), 200);
    let roles: Vec<String> =
        sqlx::query_scalar("SELECT role FROM account_roles WHERE account_id = $1")
            .bind(id)
            .fetch_all(&hub.state.db)
            .await
            .unwrap();
    assert_eq!(roles, ["admin"]);
    // What does not lock them out is allowed.
    assert_eq!(
        send(
            &hub,
            "PUT",
            &format!("{me}/roles/host"),
            &cookie,
            Some(json!({}))
        )
        .await
        .status(),
        204
    );
    assert_eq!(
        send(
            &hub,
            "PUT",
            &format!("{me}/quota"),
            &cookie,
            Some(json!({"realm_quota": 2}))
        )
        .await
        .status(),
        204
    );
}

#[tokio::test]
async fn a_deleted_account_is_refused_every_mutation() {
    let hub = TestHub::start().await;
    new_admin(&hub, "Alex").await;
    let (sam, sam_token) = new_account(&hub, "Sam").await;
    let deleted = hub
        .send(
            "DELETE",
            "/v1/me",
            PEER,
            Some(&sam_token),
            Some(json!({"password": PASSWORD})),
        )
        .await;
    assert_eq!(deleted.status(), 204);
    let cookie = admin_cookie(&hub, "Alex").await;
    let account = format!("/v1/admin/accounts/{sam}");
    let before = audit(&hub).await.len();
    for (method, path, json) in [
        ("PUT", format!("{account}/roles/host"), Some(json!({}))),
        ("DELETE", format!("{account}/roles/host"), None),
        (
            "PUT",
            format!("{account}/quota"),
            Some(json!({"realm_quota": 1})),
        ),
        (
            "POST",
            format!("{account}/suspend"),
            Some(json!({"reason": "x"})),
        ),
        ("POST", format!("{account}/unsuspend"), None),
        ("POST", format!("{account}/reset-password"), None),
        ("DELETE", format!("{account}/sessions"), None),
    ] {
        let response = send(&hub, method, &path, &cookie, json).await;
        assert_eq!(
            refusal(response).await,
            (403, "forbidden".to_owned()),
            "{method} {path}"
        );
    }
    assert_eq!(audit(&hub).await.len(), before, "nothing was done");
    let detail = body(get(&hub, &account, &cookie).await).await;
    assert_eq!(detail["status"], "deleted");
    assert!(detail["deleted_at"].is_i64());
}

#[tokio::test]
async fn the_lists_page_within_their_bounds() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    new_admin(&hub, "Alex").await;
    for name in ["Sam", "Sam_2", "Robin"] {
        new_account(&hub, name).await;
    }
    admin::grant(db, "Robin", RoleArg::Host, Some(2))
        .await
        .unwrap();
    admin::suspend(db, "Sam", "spam").await.unwrap();
    let cookie = admin_cookie(&hub, "Alex").await;

    let page = body(get(&hub, "/v1/admin/accounts", &cookie).await).await;
    assert_eq!(page["total"], 4);
    let names: Vec<&str> = page["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|account| account["username"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Robin", "Sam_2", "Sam", "Alex"], "newest first");
    let robin = &page["accounts"][0];
    assert_eq!(robin["roles"], json!(["host"]));
    assert_eq!(robin["realm_quota"], 2);
    assert_eq!(robin["status"], "active");
    assert!(robin["created_at"].is_i64());

    let page = body(get(&hub, "/v1/admin/accounts?limit=2&offset=1", &cookie).await).await;
    assert_eq!(page["total"], 4);
    assert_eq!(page["accounts"].as_array().unwrap().len(), 2);
    assert_eq!(page["accounts"][0]["username"], "Sam_2");
    let page = body(get(&hub, "/v1/admin/accounts?q=SAM", &cookie).await).await;
    assert_eq!(page["total"], 2);
    // `_` is a character, not a wildcard.
    let page = body(get(&hub, "/v1/admin/accounts?q=m_", &cookie).await).await;
    assert_eq!(page["total"], 1);
    let page = body(get(&hub, "/v1/admin/accounts?q=%25", &cookie).await).await;
    assert_eq!(page["total"], 0);
    let page = body(get(&hub, "/v1/admin/accounts?status=suspended", &cookie).await).await;
    assert_eq!(page["accounts"][0]["username"], "Sam");
    assert_eq!(page["accounts"][0]["status_reason"], "spam");
    let page = body(get(&hub, "/v1/admin/accounts?role=admin&status=", &cookie).await).await;
    assert_eq!(page["total"], 1);
    assert_eq!(
        get(&hub, "/v1/admin/accounts?limit=100", &cookie)
            .await
            .status(),
        200
    );
    for wrong in [
        "limit=0",
        "limit=101",
        "offset=-1",
        "status=gone",
        "role=owner",
        "limit=x",
        "sort=name",
    ] {
        let response = get(&hub, &format!("/v1/admin/accounts?{wrong}"), &cookie).await;
        assert_eq!(
            refusal(response).await,
            (400, "invalid_request".to_owned()),
            "{wrong}"
        );
    }

    // The audit log, by pages of ids.
    let all = body(get(&hub, "/v1/admin/audit?limit=200", &cookie).await).await;
    let ids: Vec<i64> = all["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_i64().unwrap())
        .collect();
    assert!(ids.len() >= 7);
    assert!(ids.windows(2).all(|pair| pair[0] > pair[1]), "newest first");
    assert_eq!(all["next_before"], Value::Null);
    let first = body(get(&hub, "/v1/admin/audit?limit=3", &cookie).await).await;
    assert_eq!(first["entries"].as_array().unwrap().len(), 3);
    assert_eq!(first["next_before"], ids[2]);
    let second = body(
        get(
            &hub,
            &format!("/v1/admin/audit?limit=3&before={}", ids[2]),
            &cookie,
        )
        .await,
    )
    .await;
    assert_eq!(second["entries"][0]["id"], ids[3]);
    let suspended = body(get(&hub, "/v1/admin/audit?action=suspended", &cookie).await).await;
    assert_eq!(suspended["entries"].as_array().unwrap().len(), 1);
    let entry = &suspended["entries"][0];
    assert_eq!(entry["actor"], "admin-cli");
    assert_eq!(entry["username"], "Sam");
    assert_eq!(entry["details"]["reason"], "spam");
    let sam = entry["account_id"].as_str().unwrap().to_owned();
    let by_id = body(get(&hub, &format!("/v1/admin/audit?account={sam}"), &cookie).await).await;
    let by_name = body(get(&hub, "/v1/admin/audit?account=sam", &cookie).await).await;
    assert_eq!(by_id, by_name);
    assert_eq!(
        by_id["entries"].as_array().unwrap().len(),
        2,
        "signup and suspended"
    );
    let nobody = body(get(&hub, "/v1/admin/audit?account=nobody", &cookie).await).await;
    assert_eq!(nobody["entries"], json!([]));
    assert_eq!(
        get(&hub, "/v1/admin/audit?limit=200", &cookie)
            .await
            .status(),
        200
    );
    for wrong in ["limit=0", "limit=201", "before=x", "actor=me"] {
        let response = get(&hub, &format!("/v1/admin/audit?{wrong}"), &cookie).await;
        assert_eq!(response.status(), 400, "{wrong}");
    }
}

#[tokio::test]
async fn the_overview_detail_and_worlds_show_the_hub() {
    let hub = TestHub::start().await;
    let db = &hub.state.db;
    let alex = new_admin(&hub, "Alex").await;
    let (_, robin_token) = new_account(&hub, "Robin").await;
    admin::grant(db, "Robin", RoleArg::Host, Some(2))
        .await
        .unwrap();
    new_account(&hub, "Sam").await;
    admin::suspend(db, "Sam", "spam").await.unwrap();
    let cookie = admin_cookie(&hub, "Alex").await;
    let mut realms = Vec::new();
    for name in ["One", "Two"] {
        let created = hub
            .send(
                "POST",
                "/v1/realms",
                PEER,
                Some(&robin_token),
                Some(json!({"name": name})),
            )
            .await;
        realms.push(body(created).await["id"].as_str().unwrap().to_owned());
    }
    let started = hub
        .send(
            "POST",
            &format!("/v1/realms/{}/start", realms[0]),
            PEER,
            Some(&robin_token),
            None,
        )
        .await;
    assert_eq!(started.status(), 200);
    assert_eq!(
        send(
            &hub,
            "DELETE",
            &format!("/v1/admin/realms/{}", realms[1]),
            &cookie,
            None
        )
        .await
        .status(),
        204
    );
    let _ = log_in(&hub, "Robin", "a wrong password here").await;

    let overview = body(get(&hub, "/v1/admin/overview", &cookie).await).await;
    assert_eq!(
        overview["accounts"],
        json!({"active": 2, "suspended": 1, "deleted": 0})
    );
    assert_eq!(overview["admins"], 1);
    assert_eq!(overview["hosts"], 1);
    // Alex and Robin's sign-ups; Sam's ended with the suspension.
    assert_eq!(overview["sessions"], json!({"game": 2, "admin_web": 1}));
    assert_eq!(overview["signups"], json!({"last_24h": 3, "last_7d": 3}));
    assert_eq!(overview["failed_logins_24h"], 1);
    assert_eq!(
        overview["realms"],
        json!({"stopped": 0, "starting": 1, "running": 0, "stopping": 0, "failed": 0, "archived": 1})
    );
    assert_eq!(overview["max_running"], common::MAX_RUNNING);
    assert!(overview["running"].as_i64().unwrap() >= 1);

    let list = body(get(&hub, "/v1/admin/realms", &cookie).await).await;
    let list = list["realms"].as_array().unwrap();
    assert_eq!(list.len(), 1, "archived worlds are hidden");
    let world = &list[0];
    assert_eq!(world["id"], realms[0]);
    assert_eq!(world["name"], "One");
    assert_eq!(world["owner"]["username"], "Robin");
    assert_eq!(world["state"], "starting");
    assert_eq!(world["slot"], 0);
    assert_eq!(world["port"], common::BASE_PORT);
    assert_eq!(world["players"], 0);
    assert_eq!(world["last_played_at"], Value::Null);
    let archived = body(get(&hub, "/v1/admin/realms?state=archived", &cookie).await).await;
    assert_eq!(archived["realms"][0]["id"], realms[1]);
    assert_eq!(
        get(&hub, "/v1/admin/realms?state=gone", &cookie)
            .await
            .status(),
        400
    );

    let detail = body(get(&hub, &format!("/v1/admin/accounts/{alex}"), &cookie).await).await;
    assert_eq!(detail["username"], "Alex");
    assert_eq!(detail["roles"], json!(["admin"]));
    assert_eq!(detail["deleted_at"], Value::Null);
    let sessions = detail["sessions"].as_array().unwrap();
    assert_eq!(sessions.len(), 2);
    let kinds: Vec<&str> = sessions
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["admin_web", "game"], "newest first");
    for session in sessions {
        let keys: Vec<&String> = session.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            [
                "created_at",
                "device_name",
                "expires_at",
                "id",
                "kind",
                "last_used_at"
            ],
            "never the token"
        );
    }
    assert!(!detail.to_string().contains(token_of(&cookie)));
    let actions: Vec<&str> = detail["audit"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["action"].as_str().unwrap())
        .collect();
    assert_eq!(actions, ["admin_login", "role_granted", "signup"]);
    let found = body(get(&hub, "/v1/admin/accounts?q=robin", &cookie).await).await;
    let robin = found["accounts"][0]["id"].as_str().unwrap();
    let robin = body(get(&hub, &format!("/v1/admin/accounts/{robin}"), &cookie).await).await;
    assert_eq!(
        robin["realms"].as_array().unwrap().len(),
        2,
        "archived ones included"
    );
}
