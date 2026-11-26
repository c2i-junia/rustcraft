mod common;

use std::time::Duration;

use axum::{body::Body, http::Response};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use common::{FakeAgent, FakeProbe, TestHub, BASE_PORT, REALM_HOST};
use http_body_util::BodyExt;
use rustcraft_agent_protocol::{RealmState as ContainerState, Request, Response as AgentResponse};
use rustcraft_hub::{
    admin,
    config::RoleArg,
    realms::monitor::{self, Timings},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

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

/// A new account with the host role and room for `realms` worlds.
async fn new_host(hub: &TestHub, username: &str, realms: i32) -> String {
    let token = new_account(hub, username).await;
    admin::grant(&hub.state.db, username, RoleArg::Host, Some(realms))
        .await
        .unwrap();
    token
}

async fn post(hub: &TestHub, path: &str, token: &str, body: Option<Value>) -> Response<Body> {
    hub.send("POST", path, PEER, Some(token), body).await
}

async fn create(hub: &TestHub, token: &str, name: &str) -> Uuid {
    let response = post(hub, "/v1/realms", token, Some(json!({"name": name}))).await;
    assert_eq!(response.status(), 201);
    body(response).await["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn start(hub: &TestHub, token: &str, id: Uuid) -> Response<Body> {
    post(hub, &format!("/v1/realms/{id}/start"), token, None).await
}

async fn list(hub: &TestHub, token: &str) -> Value {
    let response = hub.send("GET", "/v1/realms", PEER, Some(token), None).await;
    assert_eq!(response.status(), 200);
    body(response).await
}

async fn set_member(hub: &TestHub, token: &str, id: Uuid, name: &str, role: &str) -> u16 {
    hub.send(
        "PUT",
        &format!("/v1/realms/{id}/members/{name}"),
        PEER,
        Some(token),
        Some(json!({"role": role})),
    )
    .await
    .status()
    .as_u16()
}

/// The state and slot the database holds for the world.
async fn row(hub: &TestHub, id: Uuid) -> (String, Option<i16>) {
    sqlx::query_as("SELECT state, slot FROM realms WHERE id = $1")
        .bind(id)
        .fetch_one(&hub.state.db)
        .await
        .unwrap()
}

/// Waits for what the hub does in the background.
async fn eventually(hub: &TestHub, id: Uuid, state: &str) {
    for _ in 0..200 {
        if row(hub, id).await.0 == state {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the world never became {state}: {:?}", row(hub, id).await);
}

/// The token the agent was given at the last start of `id`.
fn token_of(hub: &TestHub, id: Uuid) -> String {
    hub.agent
        .requests()
        .into_iter()
        .rev()
        .find_map(|request| match request {
            Request::Start { realm, token, .. } if realm == id => Some(token),
            _ => None,
        })
        .expect("the world was started")
}

#[tokio::test]
async fn a_host_creates_worlds_within_its_quota() {
    let hub = TestHub::start().await;
    let plain = new_account(&hub, "Plain").await;
    let response = post(&hub, "/v1/realms", &plain, Some(json!({"name": "Home"}))).await;
    assert_eq!(response.status(), 403);
    assert_eq!(body(response).await["error"], "forbidden");

    let host = new_host(&hub, "Alex", 2).await;
    let response = post(
        &hub,
        "/v1/realms",
        &host,
        Some(
            json!({"name": " Home ", "settings": {"seed": 7, "world_type": "archipelago",
            "game_mode": "creative", "difficulty": "hard"}}),
        ),
    )
    .await;
    assert_eq!(response.status(), 201);
    let realm = body(response).await;
    assert_eq!(realm["name"], "Home");
    assert_eq!(realm["state"], "stopped");
    assert_eq!(realm["owner"]["username"], "Alex");
    assert_eq!(realm["members"], json!([]));
    assert!(realm.get("address").is_none());
    create(&hub, &host, "Second").await;

    let response = post(&hub, "/v1/realms", &host, Some(json!({"name": "Third"}))).await;
    assert_eq!(response.status(), 403);
    assert_eq!(body(response).await["error"], "quota_reached");
    assert_eq!(list(&hub, &host).await.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn a_world_with_a_bad_name_or_bad_settings_is_refused() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 5).await;
    for request in [
        json!({"name": ""}),
        json!({"name": "a".repeat(33)}),
        json!({"name": "Line\nbreak"}),
        json!({"name": "Home", "settings": {"seed": 4294967296u64}}),
        json!({"name": "Home", "settings": {"world_type": "single-biome"}}),
        json!({"name": "Home", "settings": {"game_mode": "spectator"}}),
        json!({"name": "Home", "settings": {"difficulty": "nightmare"}}),
        json!({"settings": {}}),
    ] {
        let response = post(&hub, "/v1/realms", &host, Some(request.clone())).await;
        assert_eq!(response.status(), 400, "{request}");
        assert_eq!(body(response).await["error"], "invalid_request");
    }
    assert_eq!(list(&hub, &host).await, json!([]));
}

#[tokio::test]
async fn a_start_takes_the_lowest_free_slot_and_gives_its_port() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 3).await;
    let first = create(&hub, &host, "First").await;
    let second = create(&hub, &host, "Second").await;

    let response = start(&hub, &host, first).await;
    assert_eq!(response.status(), 200);
    assert_eq!(
        body(response).await,
        json!({"state": "starting", "host": REALM_HOST, "port": BASE_PORT})
    );
    let response = start(&hub, &host, second).await;
    assert_eq!(body(response).await["port"], BASE_PORT + 2);
    // A second start of a starting world changes nothing
    let response = start(&hub, &host, first).await;
    assert_eq!(body(response).await["port"], BASE_PORT);
    let starts = hub
        .agent
        .requests()
        .into_iter()
        .filter(|request| matches!(request, Request::Start { .. }))
        .count();
    assert_eq!(starts, 2);

    let Request::Start {
        slot,
        token,
        settings,
        ..
    } = hub.agent.requests()[0].clone()
    else {
        panic!("not a start");
    };
    assert_eq!(slot, 0);
    assert_eq!(URL_SAFE_NO_PAD.decode(&token).unwrap().len(), 32);
    assert_eq!(settings, Default::default());
    let hash: Vec<u8> = sqlx::query_scalar("SELECT token_hash FROM realms WHERE id = $1")
        .bind(first)
        .fetch_one(&hub.state.db)
        .await
        .unwrap();
    assert_eq!(
        hash,
        Sha256::digest(URL_SAFE_NO_PAD.decode(&token).unwrap()).to_vec(),
        "the SHA-256 of the token, and only that"
    );

    let realms = list(&hub, &host).await;
    assert_eq!(realms[0]["state"], "starting");
    assert_eq!(
        realms[0]["address"],
        json!({"host": REALM_HOST, "port": BASE_PORT})
    );
}

#[tokio::test]
async fn no_more_worlds_run_than_the_hub_allows() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 4).await;
    let mut ids = Vec::new();
    for name in ["A", "B", "C", "D"] {
        ids.push(create(&hub, &host, name).await);
    }
    for id in &ids[..3] {
        assert_eq!(start(&hub, &host, *id).await.status(), 200);
    }
    let response = start(&hub, &host, ids[3]).await;
    assert_eq!(response.status(), 503);
    assert_eq!(body(response).await["error"], "no_free_slot");
    assert_eq!(row(&hub, ids[3]).await, ("stopped".into(), None));

    // Once one stops, its slot goes to the next
    let stop = post(&hub, &format!("/v1/realms/{}/stop", ids[1]), &host, None).await;
    assert_eq!(stop.status(), 202);
    eventually(&hub, ids[1], "stopped").await;
    assert!(hub
        .agent
        .requests()
        .contains(&Request::Stop { realm: ids[1] }));
    assert_eq!(
        body(start(&hub, &host, ids[3]).await).await["port"],
        BASE_PORT + 2
    );
}

#[tokio::test]
async fn a_world_is_started_at_most_ten_times_an_hour() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    for _ in 0..10 {
        assert_eq!(start(&hub, &host, id).await.status(), 200);
        rustcraft_hub::realms::stop(&hub.state, id, None)
            .await
            .unwrap();
    }
    let response = start(&hub, &host, id).await;
    assert_eq!(response.status(), 429);
    assert!(response.headers().contains_key("retry-after"));
}

#[tokio::test]
async fn a_world_is_hidden_from_who_is_not_a_member() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let stranger = new_account(&hub, "Sam").await;
    let id = create(&hub, &host, "Home").await;

    assert_eq!(list(&hub, &stranger).await, json!([]));
    for (method, path) in [
        ("POST", format!("/v1/realms/{id}/start")),
        ("POST", format!("/v1/realms/{id}/stop")),
        ("DELETE", format!("/v1/realms/{id}")),
        ("DELETE", format!("/v1/realms/{id}/members/Alex")),
        ("POST", format!("/v1/realms/{}/start", Uuid::new_v4())),
        ("POST", "/v1/realms/nonsense/start".to_owned()),
    ] {
        let response = hub.send(method, &path, PEER, Some(&stranger), None).await;
        assert_eq!(response.status(), 404, "{method} {path}");
        assert_eq!(body(response).await["error"], "not_found");
    }
    assert_eq!(set_member(&hub, &stranger, id, "Sam", "member").await, 404);
    assert!(hub.agent.requests().is_empty());
    assert_eq!(
        hub.send("GET", "/v1/realms", PEER, None, None)
            .await
            .status(),
        401
    );
}

#[tokio::test]
async fn the_owner_manages_the_members_and_any_member_starts() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let bo = new_account(&hub, "Bob").await;
    let cy = new_account(&hub, "Cyd").await;
    let id = create(&hub, &host, "Home").await;

    assert_eq!(set_member(&hub, &host, id, "bob", "member").await, 204);
    assert_eq!(set_member(&hub, &host, id, "Cyd", "operator").await, 204);
    assert_eq!(set_member(&hub, &host, id, "Nobody", "member").await, 404);
    assert_eq!(set_member(&hub, &host, id, "Alex", "member").await, 400);
    assert_eq!(set_member(&hub, &host, id, "Cyd", "owner").await, 400);
    // A member is not the owner
    assert_eq!(set_member(&hub, &bo, id, "Cyd", "member").await, 403);
    let response = post(&hub, &format!("/v1/realms/{id}/stop"), &bo, None).await;
    assert_eq!(response.status(), 403);
    assert_eq!(
        hub.send("DELETE", &format!("/v1/realms/{id}"), PEER, Some(&bo), None)
            .await
            .status(),
        403
    );

    let seen = list(&hub, &bo).await;
    assert_eq!(seen[0]["id"], id.to_string());
    assert_eq!(
        seen[0]["members"],
        json!([
            {"id": seen[0]["members"][0]["id"], "username": "Bob", "role": "member"},
            {"id": seen[0]["members"][1]["id"], "username": "Cyd", "role": "operator"},
        ])
    );

    let response = start(&hub, &bo, id).await;
    assert_eq!(response.status(), 200);
    let token = token_of(&hub, id);
    let config = hub
        .get_with(
            &format!("/v1/realms/{id}/config"),
            &format!("Realm {token}"),
        )
        .await;
    assert_eq!(config.status(), 200);
    let config = body(config).await;
    assert_eq!(config["owner"]["username"], "Alex");
    assert_eq!(config["members"][1]["role"], "operator");

    assert_eq!(set_member(&hub, &host, id, "Cyd", "member").await, 204);
    let removed = hub
        .send(
            "DELETE",
            &format!("/v1/realms/{id}/members/BOB"),
            PEER,
            Some(&host),
            None,
        )
        .await;
    assert_eq!(removed.status(), 204);
    let again = hub
        .send(
            "DELETE",
            &format!("/v1/realms/{id}/members/Bob"),
            PEER,
            Some(&host),
            None,
        )
        .await;
    assert_eq!(again.status(), 404);
    let config = hub
        .get_with(
            &format!("/v1/realms/{id}/config"),
            &format!("Realm {token}"),
        )
        .await;
    assert_eq!(
        body(config).await["members"],
        json!([{"id": seen[0]["members"][1]["id"], "username": "Cyd", "role": "member"}])
    );
    assert_eq!(list(&hub, &bo).await, json!([]));
    assert_eq!(list(&hub, &cy).await.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn the_config_needs_the_token_of_the_current_start() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    let path = format!("/v1/realms/{id}/config");
    assert_eq!(
        hub.get_with(&path, &format!("Realm {}", URL_SAFE_NO_PAD.encode([0; 32])))
            .await
            .status(),
        401
    );
    start(&hub, &host, id).await;
    let token = token_of(&hub, id);
    for authorization in [
        format!("Bearer {token}"),
        format!("Realm {}", URL_SAFE_NO_PAD.encode([1; 32])),
        "Realm not-base64!".to_owned(),
        "Realm".to_owned(),
    ] {
        let response = hub.get_with(&path, &authorization).await;
        assert_eq!(response.status(), 401, "{authorization}");
        assert_eq!(body(response).await["error"], "unauthorized");
    }
    let elsewhere = format!("/v1/realms/{}/config", Uuid::new_v4());
    assert_eq!(
        hub.get_with(&elsewhere, &format!("Realm {token}"))
            .await
            .status(),
        401
    );
    assert_eq!(
        hub.get_with(&path, &format!("Realm {token}"))
            .await
            .status(),
        200
    );

    // A stop ends the token, and the next start brings another
    rustcraft_hub::realms::stop(&hub.state, id, None)
        .await
        .unwrap();
    assert_eq!(
        hub.get_with(&path, &format!("Realm {token}"))
            .await
            .status(),
        401
    );
    start(&hub, &host, id).await;
    assert_ne!(token_of(&hub, id), token);
}

fn timings() -> Timings {
    Timings::default()
}

fn later(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::now_utc() + time::Duration::seconds(seconds)
}

#[tokio::test]
async fn a_world_that_answers_runs_and_stops_after_ten_minutes_without_players() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    start(&hub, &host, id).await;

    monitor::check_starting(&hub.state, &timings(), later(2))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await.0, "starting");
    hub.probe.answer(0, Some(0));
    monitor::check_starting(&hub.state, &timings(), later(5))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await, ("running".into(), Some(0)));
    assert_eq!(list(&hub, &host).await[0]["state"], "running");

    // Players come and go: the ten minutes count from when the last one left
    hub.probe.answer(0, Some(2));
    monitor::check_running(&hub.state, &timings(), later(300))
        .await
        .unwrap();
    hub.probe.answer(0, Some(0));
    monitor::check_running(&hub.state, &timings(), later(330))
        .await
        .unwrap();
    monitor::check_running(&hub.state, &timings(), later(900))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await.0, "running");
    let (players, last_played): (i32, Option<OffsetDateTime>) =
        sqlx::query_as("SELECT players, last_played_at FROM realms WHERE id = $1")
            .bind(id)
            .fetch_one(&hub.state.db)
            .await
            .unwrap();
    assert_eq!(players, 0);
    assert!(last_played.is_some());

    monitor::check_running(&hub.state, &timings(), later(930))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await, ("stopped".into(), None));
    assert!(hub.agent.requests().contains(&Request::Stop { realm: id }));
    assert_eq!(hub.agent.container(id), ContainerState::Absent);
}

#[tokio::test]
async fn a_world_that_does_not_answer_within_a_minute_fails() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    start(&hub, &host, id).await;
    monitor::check_starting(&hub.state, &timings(), later(55))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await.0, "starting");
    monitor::check_starting(&hub.state, &timings(), later(61))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await, ("failed".into(), None));
    assert!(hub.agent.requests().contains(&Request::Stop { realm: id }));
    let realms = list(&hub, &host).await;
    assert_eq!(realms[0]["state"], "failed");
    assert!(realms[0].get("address").is_none());
    // A failed world starts again
    assert_eq!(start(&hub, &host, id).await.status(), 200);
    assert_eq!(row(&hub, id).await.0, "starting");
}

#[tokio::test]
async fn a_start_the_agent_refuses_fails_and_frees_the_slot() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    *hub.agent.start_answer.lock().unwrap() = Some(AgentResponse::Failed {
        error: "port is already allocated".into(),
    });
    let response = start(&hub, &host, id).await;
    assert_eq!(response.status(), 500);
    assert_eq!(body(response).await["error"], "internal");
    assert_eq!(row(&hub, id).await, ("failed".into(), None));
}

#[tokio::test]
async fn a_world_whose_server_exits_ends_stopped_or_failed() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 2).await;
    let good = create(&hub, &host, "Good").await;
    let bad = create(&hub, &host, "Bad").await;
    for (id, slot) in [(good, 0), (bad, 1)] {
        start(&hub, &host, id).await;
        hub.probe.answer(slot, Some(1));
    }
    monitor::check_starting(&hub.state, &timings(), later(5))
        .await
        .unwrap();
    hub.agent
        .set_container(good, ContainerState::Exited { exit_code: Some(0) });
    hub.agent.set_container(
        bad,
        ContainerState::Exited {
            exit_code: Some(101),
        },
    );
    monitor::check_running(&hub.state, &timings(), later(30))
        .await
        .unwrap();
    assert_eq!(row(&hub, good).await, ("stopped".into(), None));
    assert_eq!(row(&hub, bad).await, ("failed".into(), None));
    let reason: Option<String> =
        sqlx::query_scalar("SELECT failed_reason FROM realms WHERE id = $1")
            .bind(bad)
            .fetch_one(&hub.state.db)
            .await
            .unwrap();
    assert_eq!(reason.as_deref(), Some("The server exited with code 101."));
}

#[tokio::test]
async fn a_stop_the_agent_misses_is_tried_again() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    start(&hub, &host, id).await;
    *hub.agent.stop_answer.lock().unwrap() = Some(AgentResponse::Failed {
        error: "Docker is busy".into(),
    });
    assert!(rustcraft_hub::realms::stop(&hub.state, id, None)
        .await
        .is_err());
    assert_eq!(row(&hub, id).await, ("stopping".into(), Some(0)));
    assert_eq!(start(&hub, &host, id).await.status(), 400);

    *hub.agent.stop_answer.lock().unwrap() = None;
    monitor::check_running(&hub.state, &timings(), later(30))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await, ("stopped".into(), None));
}

#[tokio::test]
async fn at_start_the_hub_takes_the_agent_s_view() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 3).await;
    let running = create(&hub, &host, "Running").await;
    let vanished = create(&hub, &host, "Vanished").await;
    let stopping = create(&hub, &host, "Stopping").await;
    for id in [running, vanished, stopping] {
        start(&hub, &host, id).await;
    }
    hub.probe.answer(0, Some(0));
    hub.probe.answer(1, Some(0));
    monitor::check_starting(&hub.state, &timings(), later(5))
        .await
        .unwrap();
    sqlx::query("UPDATE realms SET state = 'stopping' WHERE id = $1")
        .bind(stopping)
        .execute(&hub.state.db)
        .await
        .unwrap();
    hub.agent.containers.lock().unwrap().remove(&vanished);

    monitor::reconcile(&hub.state).await.unwrap();
    assert_eq!(row(&hub, running).await, ("running".into(), Some(0)));
    assert_eq!(row(&hub, vanished).await, ("failed".into(), None));
    assert_eq!(row(&hub, stopping).await, ("stopped".into(), None));
    assert_eq!(hub.agent.container(stopping), ContainerState::Absent);
}

#[tokio::test]
async fn an_archived_world_leaves_the_lists_and_its_volume_is_archived() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let bo = new_account(&hub, "Bob").await;
    let id = create(&hub, &host, "Home").await;
    set_member(&hub, &host, id, "Bob", "member").await;
    start(&hub, &host, id).await;
    let token = token_of(&hub, id);

    let response = hub
        .send(
            "DELETE",
            &format!("/v1/realms/{id}"),
            PEER,
            Some(&host),
            None,
        )
        .await;
    assert_eq!(response.status(), 204);
    assert_eq!(list(&hub, &host).await, json!([]));
    assert_eq!(list(&hub, &bo).await, json!([]));
    assert_eq!(
        hub.get_with(
            &format!("/v1/realms/{id}/config"),
            &format!("Realm {token}")
        )
        .await
        .status(),
        401
    );
    assert_eq!(start(&hub, &bo, id).await.status(), 404);
    for _ in 0..200 {
        if row(&hub, id).await.1.is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(row(&hub, id).await, ("archived".into(), None));
    assert!(hub
        .agent
        .requests()
        .contains(&Request::Archive { realm: id }));
    // The quota counts the worlds that are not archived
    create(&hub, &host, "Next").await;

    rustcraft_hub::realms::daily(&hub.state).await.unwrap();
    let archives = hub
        .agent
        .requests()
        .into_iter()
        .filter(|request| matches!(request, Request::Archive { .. }))
        .count();
    assert_eq!(archives, 1, "an archived world is not archived again");
    assert_eq!(hub.agent.requests().last(), Some(&Request::Purge));
}

#[tokio::test]
async fn deleting_an_account_archives_its_worlds_and_leaves_the_others() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let other = new_host(&hub, "Bob", 1).await;
    let mine = create(&hub, &host, "Mine").await;
    let theirs = create(&hub, &other, "Theirs").await;
    set_member(&hub, &other, theirs, "Alex", "operator").await;

    let response = hub
        .send(
            "DELETE",
            "/v1/me",
            PEER,
            Some(&host),
            Some(json!({"password": PASSWORD})),
        )
        .await;
    assert_eq!(response.status(), 204);
    eventually(&hub, mine, "archived").await;
    for _ in 0..200 {
        if hub
            .agent
            .requests()
            .contains(&Request::Archive { realm: mine })
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(hub
        .agent
        .requests()
        .contains(&Request::Archive { realm: mine }));
    assert_eq!(row(&hub, theirs).await.0, "stopped");
    assert_eq!(list(&hub, &other).await[0]["members"], json!([]));
}

#[tokio::test]
async fn the_admin_lists_the_worlds() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 2).await;
    let home = create(&hub, &host, "Home").await;
    create(&hub, &host, "Away").await;
    start(&hub, &host, home).await;
    hub.probe.answer(0, Some(3));
    monitor::check_starting(&hub.state, &timings(), later(5))
        .await
        .unwrap();
    let table = admin::realms(&hub.state.db).await.unwrap();
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines.len(), 3, "{table}");
    let columns = |line: &str| {
        line.split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        columns(lines[0]),
        ["ID", "NAME", "OWNER", "STATE", "SLOT", "PLAYERS", "LAST", "PLAYED"]
    );
    let home_row = columns(lines[1]);
    assert_eq!(
        home_row[..6],
        [
            home.to_string(),
            "Home".into(),
            "Alex".into(),
            "running".into(),
            "0".into(),
            "3".into()
        ]
    );
    assert_ne!(home_row[6], "-");
    assert_eq!(columns(lines[2])[2..], ["Alex", "stopped", "-", "0", "-"]);
}

#[tokio::test]
async fn concurrent_starts_never_share_a_slot_nor_pass_the_limit() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 4).await;
    let mut ids = Vec::new();
    for name in ["A", "B", "C", "D"] {
        ids.push(create(&hub, &host, name).await);
    }
    // Two places, two starts at once: two slots
    let (a, b) = tokio::join!(start(&hub, &host, ids[0]), start(&hub, &host, ids[1]));
    let (a, b) = (body(a).await, body(b).await);
    assert_eq!(a["state"], "starting");
    assert_eq!(b["state"], "starting");
    assert_ne!(a["port"], b["port"]);
    // One place left, two starts at once: one slot and one refusal
    let (c, d) = tokio::join!(start(&hub, &host, ids[2]), start(&hub, &host, ids[3]));
    let mut statuses = [c.status().as_u16(), d.status().as_u16()];
    statuses.sort();
    assert_eq!(statuses, [200, 503]);
    let slots: Vec<i16> =
        sqlx::query_scalar("SELECT slot FROM realms WHERE slot IS NOT NULL ORDER BY slot")
            .fetch_all(&hub.state.db)
            .await
            .unwrap();
    assert_eq!(slots, [0, 1, 2]);
}

#[tokio::test]
async fn a_stopping_world_whose_container_is_gone_ends_stopped() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    let id = create(&hub, &host, "Home").await;
    start(&hub, &host, id).await;
    hub.probe.answer(0, Some(0));
    monitor::check_starting(&hub.state, &timings(), later(5))
        .await
        .unwrap();
    // As while the owner's stop holds the agent: the world is stopping, its container gone
    sqlx::query("UPDATE realms SET state = 'stopping' WHERE id = $1")
        .bind(id)
        .execute(&hub.state.db)
        .await
        .unwrap();
    hub.agent.containers.lock().unwrap().remove(&id);
    monitor::check_running(&hub.state, &timings(), later(30))
        .await
        .unwrap();
    assert_eq!(row(&hub, id).await, ("stopped".into(), None));
    let reason: Option<String> =
        sqlx::query_scalar("SELECT failed_reason FROM realms WHERE id = $1")
            .bind(id)
            .fetch_one(&hub.state.db)
            .await
            .unwrap();
    assert_eq!(reason, None);
}

#[tokio::test]
async fn the_running_container_of_a_world_without_a_slot_is_stopped() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 2).await;
    let stray = create(&hub, &host, "Stray").await;
    let exited = create(&hub, &host, "Exited").await;
    hub.agent.set_container(stray, ContainerState::Running);
    hub.agent
        .set_container(exited, ContainerState::Exited { exit_code: Some(0) });
    rustcraft_hub::realms::daily(&hub.state).await.unwrap();
    assert!(hub
        .agent
        .requests()
        .contains(&Request::Stop { realm: stray }));
    assert!(!hub
        .agent
        .requests()
        .contains(&Request::Stop { realm: exited }));
    assert_eq!(hub.agent.container(stray), ContainerState::Absent);
    assert_eq!(row(&hub, stray).await, ("stopped".into(), None));
}

#[tokio::test]
async fn the_config_lists_the_active_members_only() {
    let hub = TestHub::start().await;
    let host = new_host(&hub, "Alex", 1).await;
    new_account(&hub, "Bob").await;
    new_account(&hub, "Cyd").await;
    let id = create(&hub, &host, "Home").await;
    set_member(&hub, &host, id, "Bob", "member").await;
    set_member(&hub, &host, id, "Cyd", "operator").await;
    admin::suspend(&hub.state.db, "Cyd", "griefing")
        .await
        .unwrap();
    start(&hub, &host, id).await;
    let token = token_of(&hub, id);
    let config = hub
        .get_with(
            &format!("/v1/realms/{id}/config"),
            &format!("Realm {token}"),
        )
        .await;
    let members = body(config).await["members"].clone();
    assert_eq!(members.as_array().unwrap().len(), 1);
    assert_eq!(members[0]["username"], "Bob");
    // The owner still sees the suspended member, to remove them
    assert_eq!(
        list(&hub, &host).await[0]["members"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn slots_must_fit_the_ports_and_a_smallint() {
    let hosting = |base_port, slots| {
        rustcraft_hub::realms::Hosting::new(
            std::sync::Arc::new(FakeAgent::default()),
            std::sync::Arc::new(FakeProbe::default()),
            REALM_HOST.into(),
            base_port,
            slots,
            3,
        )
    };
    assert!(hosting(25000, 20).is_ok());
    assert!(hosting(25000, 0).is_err());
    assert!(hosting(65000, 268).is_ok());
    assert!(hosting(65000, 269).is_err());
    assert!(hosting(0, 32767).is_ok());
    assert!(hosting(0, 32768).is_err());
}
