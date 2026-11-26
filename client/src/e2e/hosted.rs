//! The Hosted worlds tab of the Multiplayer screen, against a stand-in for the hub.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use accesskit::Role;
use bevy::math::Vec2;
use rustcraft_protocol::api::{
    AccountSummary, MemberRole, Realm, RealmAddress, RealmMember, RealmState,
};
use serde_json::{json, Value};
use uuid::Uuid;

use super::account::{client, logged_in_folder, logged_out_folder};
use super::harness::{Harness, SCREEN};
use super::locator::Locator as L;
use crate::account::test_hub::{Canned, Recorded, Route, TestHub};
use crate::account::Account;
use crate::network::session::SessionOrigin;
use crate::network::status::{ServerStatus, StatusProbe};
use crate::network::TargetServer;
use crate::state::GameState;
use crate::ui::menus::account::{ACCOUNT_TITLE, SESSION_ENDED};
use crate::ui::menus::connect_error::TRY_AGAIN;
use crate::ui::menus::connecting::STARTING_HOSTED_WORLD;
use crate::ui::menus::multi::hosted::{HostedPane, CREATE_WORLD, HOSTED_TITLE, LOGGED_OUT};
use crate::ui::menus::multi::hosted_dialogs::{MemberAction, CREATE_TITLE};
use crate::ui::menus::multi::SERVERS_TITLE;
use crate::ui::menus::MenuState;

/// Steve, the player of `logged_in_folder`, may host two worlds.
const ME: &str = r#"{"id":"00000000-0000-0000-0000-000000000001","username":"Steve","roles":["host"],"realm_quota":2,"created_at":1}"#;

fn account(id: u128, username: &str) -> AccountSummary {
    AccountSummary {
        id: Uuid::from_u128(id),
        username: username.into(),
    }
}

fn member(id: u128, username: &str, role: MemberRole) -> RealmMember {
    RealmMember {
        id: Uuid::from_u128(id),
        username: username.into(),
        role,
    }
}

/// Home, Steve's own world, running.
fn home() -> Realm {
    Realm {
        id: Uuid::from_u128(0xa),
        name: "Home".into(),
        owner: account(1, "Steve"),
        state: RealmState::Running,
        address: Some(RealmAddress {
            host: "play.example".into(),
            port: 25000,
        }),
        members: vec![member(2, "Alex", MemberRole::Operator)],
    }
}

/// Castle, Alex's world, stopped, where Steve is an operator.
fn castle() -> Realm {
    Realm {
        id: Uuid::from_u128(0xb),
        name: "Castle".into(),
        owner: account(2, "Alex"),
        state: RealmState::Stopped,
        address: None,
        members: vec![member(1, "Steve", MemberRole::Operator)],
    }
}

/// Old farm, Notch's world, failed, where Steve is a member.
fn old_farm() -> Realm {
    Realm {
        id: Uuid::from_u128(0xc),
        name: "Old farm".into(),
        owner: account(3, "Notch"),
        state: RealmState::Failed,
        address: None,
        members: vec![member(1, "Steve", MemberRole::Member)],
    }
}

fn list(realms: &[Realm]) -> Canned {
    Canned::ok(&serde_json::to_string(realms).unwrap())
}

/// The routes of a hub that knows Steve and lists `realms`, then `others`.
fn hub_with(lists: Vec<Canned>, mut others: Vec<Route>) -> TestHub {
    let mut routes: Vec<Route> = vec![
        ("GET", "/v1/me".into(), vec![Canned::ok(ME)]),
        ("GET", "/v1/realms".into(), lists),
    ];
    routes.append(&mut others);
    TestHub::routes(routes)
}

fn realm_path(realm: &Realm, rest: &str) -> String {
    format!("/v1/realms/{}{rest}", realm.id)
}

/// A client logged in to `hub` on the Hosted worlds tab, which listed its worlds.
fn hosted_tab(hub: &TestHub, probe: StatusProbe) -> Harness {
    let mut h = client(logged_in_folder(&hub.url), &hub.url);
    h.world_mut().insert_resource(probe);
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::role(Role::Tab, HOSTED_TITLE), SCREEN);
    h.click(&L::role(Role::Tab, HOSTED_TITLE));
    h.wait_until("the Hosted worlds tab", SCREEN, |world| {
        *world.resource::<bevy::prelude::State<MenuState>>().get() == MenuState::Hosted
    });
    h
}

fn card(name: &str) -> L {
    L::role(Role::ListItem, name)
}

fn pane_button(label: &str) -> L {
    L::role(Role::Button, label).within(L::marker::<HostedPane>())
}

fn dialog(title: &str) -> L {
    L::role(Role::Dialog, title)
}

fn requests_to(hub: &TestHub, method: &str, path: &str) -> Vec<Recorded> {
    hub.requests()
        .into_iter()
        .filter(|request| request.method == method && request.path == path)
        .collect()
}

#[track_caller]
fn wait_for_request(h: &mut Harness, hub: &TestHub, method: &str, path: &str) -> Recorded {
    h.wait_until(&format!("{method} {path}"), SCREEN, |_| {
        !requests_to(hub, method, path).is_empty()
    });
    requests_to(hub, method, path).remove(0)
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_hosted_worlds_match_their_golden() {
    let hub = hub_with(vec![list(&[home(), castle(), old_farm()])], Vec::new());
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Old farm"), SCREEN);
    // the owner of the selected world has every button
    for label in ["Join", "Members", "Stop", "Delete"] {
        assert!(h.is_visible(&pane_button(label)), "{label}");
    }
    assert!(h.is_visible(&L::role(Role::Button, CREATE_WORLD)));
    assert!(h.is_visible(&L::text("Your world")));
    assert!(h.is_visible(&L::text("by Alex \u{b7} operator")));
    // the cursor in a corner, so that it hovers nothing
    h.hover_at(Vec2::ZERO);
    h.frames(2);
    h.assert_screenshot("menu_multi_hosted");
    let requests = hub.requests();
    assert!(requests
        .iter()
        .all(|request| request.header("authorization") == Some("Bearer tok")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_operator_reads_the_members_and_a_member_has_no_controls() {
    let hub = hub_with(vec![list(&[home(), castle(), old_farm()])], Vec::new());
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Castle"), SCREEN);

    h.click(&card("Castle"));
    h.wait_for(&L::text("Castle").within(L::marker::<HostedPane>()), SCREEN);
    assert!(h.is_visible(&pane_button("Members")));
    assert!(!h.is_visible(&pane_button("Stop")));
    assert!(!h.is_visible(&pane_button("Delete")));
    h.click(&pane_button("Members"));
    let members = || dialog("Members of \"Castle\"");
    h.wait_for(&members(), SCREEN);
    assert!(h.is_visible(&L::text("Alex").within(members())));
    assert!(h.is_visible(&L::text("Owner").within(members())));
    assert!(h
        .find(&L::role(Role::Button, "Remove").within(members()))
        .is_empty());
    assert!(h.find(&L::role(Role::TextInput, "Add a player")).is_empty());
    h.click(&L::role(Role::Button, "Close").within(members()));
    h.frames(3);
    assert!(!h.is_visible(&members()));

    h.click(&card("Old farm"));
    h.wait_for(&L::text("Failed").within(L::marker::<HostedPane>()), SCREEN);
    assert!(h.is_visible(&pane_button("Join")));
    assert!(!h.is_visible(&pane_button("Members")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_logged_out_player_is_offered_the_account_dialog() {
    let mut h = client(logged_out_folder(), "http://127.0.0.1:1");
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::role(Role::Tab, HOSTED_TITLE), SCREEN);
    h.click(&L::role(Role::Tab, HOSTED_TITLE));
    h.wait_for(&L::text(LOGGED_OUT), SCREEN);
    assert!(!h.is_visible(&L::role(Role::Button, CREATE_WORLD)));

    h.click(&L::role(Role::Button, "Log in"));
    h.wait_for(&dialog(ACCOUNT_TITLE), SCREEN);
    h.frames(2);
    h.press(bevy::input::keyboard::KeyCode::Escape);
    h.frames(3);
    // the tabs lead back to the servers
    h.click(&L::role(Role::Tab, SERVERS_TITLE));
    h.wait_until("the Servers tab", SCREEN, |world| {
        *world.resource::<bevy::prelude::State<MenuState>>().get() == MenuState::Multi
    });
}

/// Set once the world of `answers_once_open` may answer.
static CASTLE_UP: AtomicBool = AtomicBool::new(false);

/// Offline until `CASTLE_UP`, then online.
fn answers_once_open(_: &str) -> ServerStatus {
    if CASTLE_UP.load(Ordering::SeqCst) {
        ServerStatus::Online {
            players: 0,
            max_players: 10,
            latency: Duration::from_millis(5),
        }
    } else {
        ServerStatus::Offline
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn join_starts_a_stopped_world_waits_for_it_then_joins_it() {
    let start = (
        "POST",
        realm_path(&castle(), "/start"),
        vec![Canned::ok(
            r#"{"state":"starting","host":"127.0.0.1","port":25004}"#,
        )],
    );
    let hub = hub_with(vec![list(&[home(), castle()])], vec![start]);
    let mut h = hosted_tab(&hub, StatusProbe(answers_once_open));
    h.wait_for(&card("Castle"), SCREEN);
    h.click(&card("Castle"));
    h.click(&pane_button("Join"));

    h.wait_for(&L::text(STARTING_HOSTED_WORLD), SCREEN);
    assert!(h.is_visible(&L::text("Joining \"Castle\"")));
    let request = wait_for_request(&mut h, &hub, "POST", &realm_path(&castle(), "/start"));
    assert_eq!(request.header("authorization"), Some("Bearer tok"));
    // the world is asked until it answers: no connection before
    h.frames(10);
    assert_eq!(h.game_state(), GameState::PreGameLoading);
    assert_eq!(h.world().resource::<TargetServer>().address, None);

    CASTLE_UP.store(true, Ordering::SeqCst);
    let address: SocketAddr = "127.0.0.1:25004".parse().unwrap();
    h.wait_until("the join of the world", SCREEN, |world| {
        world.resource::<TargetServer>().address == Some(address)
    });
    let origin = h.world().resource::<SessionOrigin>().clone();
    assert_eq!(
        origin,
        SessionOrigin::Server {
            name: "Castle".into(),
            ip: "127.0.0.1:25004".into(),
            address,
            realm: Some(castle().id),
        }
    );

    // a cancel goes back to the tab, the card of the world focused
    h.press(bevy::input::keyboard::KeyCode::Escape);
    h.wait_for(&card("Castle"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Hosted);
    h.wait_for(&L::text("Castle").within(L::marker::<HostedPane>()), SCREEN);
    let castle_card = h.get(&card("Castle"));
    h.wait_until("the focus on the card of the world", SCREEN, |world| {
        world.resource::<bevy::input_focus::InputFocus>().get() == Some(castle_card)
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_world_the_hub_cannot_start_says_why_on_the_error_card() {
    let start = (
        "POST",
        realm_path(&castle(), "/start"),
        vec![Canned::error(
            503,
            "no_free_slot",
            "The hub runs as many worlds as it can. Try again later.",
        )],
    );
    let hub = hub_with(vec![list(&[home(), castle()])], vec![start]);
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Castle"), SCREEN);
    h.click(&card("Castle"));
    h.click(&pane_button("Join"));

    h.wait_for(&L::text("Cannot start \"Castle\""), SCREEN);
    assert!(h.is_visible(&L::text(
        "The hub runs as many worlds as it can. Try again later."
    )));
    assert_eq!(h.menu_state(), MenuState::ConnectError);
    h.click(&L::role(Role::Button, "Back to Hosted worlds"));
    h.wait_for(&card("Castle"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Hosted);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_create_form_matches_its_golden_and_creates_the_world() {
    let island = Realm {
        id: Uuid::from_u128(0xd),
        name: "Island".into(),
        owner: account(1, "Steve"),
        state: RealmState::Stopped,
        address: None,
        members: Vec::new(),
    };
    let created = (
        "POST",
        "/v1/realms".to_string(),
        vec![Canned {
            status: 201,
            body: serde_json::to_string(&island).unwrap(),
        }],
    );
    let hub = hub_with(
        vec![list(&[castle()]), list(&[castle(), island])],
        vec![created],
    );
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Castle"), SCREEN);
    h.click(&L::role(Role::Button, CREATE_WORLD));
    h.wait_for(&dialog(CREATE_TITLE), SCREEN);
    h.frames(2);
    h.type_text("Island");
    h.frames(3);
    h.hover_at(Vec2::ZERO);
    h.frames(2);
    h.assert_screenshot("hosted_create");

    h.click(&L::role(Role::RadioButton, "Creative"));
    h.click(&L::role(Role::RadioButton, "Archipelago"));
    h.click(&L::role(Role::TextInput, "Seed (optional)"));
    h.type_text("42");
    h.frames(3);
    h.click(&L::role(Role::Button, "Create").within(dialog(CREATE_TITLE)));

    let request = wait_for_request(&mut h, &hub, "POST", "/v1/realms");
    let body: Value = serde_json::from_str(&request.body).unwrap();
    assert_eq!(
        body,
        json!({"name": "Island", "settings": {
            "seed": 42, "world_type": "archipelago", "game_mode": "creative",
            "difficulty": "normal"
        }})
    );
    h.wait_for(&card("Island"), SCREEN);
    h.wait_for(&L::text("Created \"Island\"."), SCREEN);
    assert!(!h.is_visible(&dialog(CREATE_TITLE)));
    // the world made is selected
    h.wait_for(&L::text("Island").within(L::marker::<HostedPane>()), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_owner_adds_and_removes_members_stops_and_deletes_the_world() {
    let mut joined = home();
    joined.members.push(member(3, "Notch", MemberRole::Member));
    let no_content = || vec![Canned::empty(204)];
    let hub = hub_with(
        vec![list(&[home()]), list(&[joined.clone()]), list(&[home()])],
        vec![
            ("PUT", realm_path(&home(), "/members/Notch"), no_content()),
            ("DELETE", realm_path(&home(), "/members/Alex"), no_content()),
            (
                "POST",
                realm_path(&home(), "/stop"),
                vec![Canned::empty(202)],
            ),
            ("DELETE", realm_path(&home(), ""), no_content()),
        ],
    );
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Home"), SCREEN);

    h.click(&pane_button("Members"));
    let members = || dialog("Members of \"Home\"");
    h.wait_for(&members(), SCREEN);
    h.frames(2);
    h.type_text("Notch");
    h.frames(3);
    h.press(bevy::input::keyboard::KeyCode::Enter);
    let added = wait_for_request(&mut h, &hub, "PUT", &realm_path(&home(), "/members/Notch"));
    let body: Value = serde_json::from_str(&added.body).unwrap();
    assert_eq!(body, json!({"role": "member"}));
    // the list asked again shows the new member in the dialog
    h.wait_for(&L::text("Notch").within(members()), SCREEN);
    h.wait_for(&L::text("Added Notch."), SCREEN);
    let removes = h.find(&L::role(Role::Button, "Remove").within(members()));
    assert_eq!(removes.len(), 2);
    let alex = removes
        .into_iter()
        .find(|&button| {
            h.world()
                .get::<MemberAction>(button)
                .is_some_and(|action| action.username == "Alex")
        })
        .unwrap();
    h.click_entity(alex, bevy::input::mouse::MouseButton::Left);
    wait_for_request(
        &mut h,
        &hub,
        "DELETE",
        &realm_path(&home(), "/members/Alex"),
    );
    // the rows are built again once the worlds are listed after the removal
    h.wait_for(&L::text("Removed Alex."), SCREEN);
    h.wait_until("the list after the removal", SCREEN, |_| {
        requests_to(&hub, "GET", "/v1/realms").len() >= 3
    });
    h.frames(5);
    h.click(&L::role(Role::Button, "Close").within(members()));
    h.frames(3);

    h.click(&pane_button("Stop"));
    let stop = dialog("Stop \"Home\"?");
    h.wait_for(&stop, SCREEN);
    h.click(&L::role(Role::Button, "Stop").within(stop));
    wait_for_request(&mut h, &hub, "POST", &realm_path(&home(), "/stop"));
    // the pane is built again once the worlds are listed after the stop
    h.wait_until("the list after the stop", SCREEN, |_| {
        requests_to(&hub, "GET", "/v1/realms").len() >= 4
    });
    h.frames(5);

    h.click(&pane_button("Delete"));
    let delete = dialog("Delete \"Home\"?");
    h.wait_for(&delete, SCREEN);
    h.click(&L::role(Role::Button, "Delete").within(delete));
    wait_for_request(&mut h, &hub, "DELETE", &realm_path(&home(), ""));
    h.wait_for(&L::text("Deleted \"Home\"."), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_refused_world_is_said_under_its_name_and_a_refused_session_logs_out() {
    let created = (
        "POST",
        "/v1/realms".to_string(),
        vec![
            Canned::error(
                403,
                "quota_reached",
                "You already have as many worlds as you may.",
            ),
            Canned::error(401, "unauthorized", "Log in again."),
        ],
    );
    let hub = hub_with(vec![list(&[castle()])], vec![created]);
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Castle"), SCREEN);
    h.click(&L::role(Role::Button, CREATE_WORLD));
    h.wait_for(&dialog(CREATE_TITLE), SCREEN);
    h.frames(2);
    h.type_text("Island");
    h.frames(3);
    h.press(bevy::input::keyboard::KeyCode::Enter);
    wait_for_request(&mut h, &hub, "POST", "/v1/realms");
    // under the name, not in a toast: the form stays open to change it
    h.wait_for(
        &L::text("You already have as many worlds as you may.").within(dialog(CREATE_TITLE)),
        SCREEN,
    );

    h.click(&L::role(Role::Button, "Create").within(dialog(CREATE_TITLE)));
    h.wait_for(&L::text(SESSION_ENDED), SCREEN);
    h.wait_for(&L::text(LOGGED_OUT), SCREEN);
    assert!(matches!(
        h.world().resource::<Account>(),
        Account::LoggedOut
    ));
    h.frames(3);
    assert!(!h.is_visible(&dialog(CREATE_TITLE)));
    assert_eq!(requests_to(&hub, "POST", "/v1/realms").len(), 2);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_session_the_hub_refuses_at_the_start_of_a_world_offers_no_try_again() {
    let start = (
        "POST",
        realm_path(&castle(), "/start"),
        vec![Canned::error(401, "unauthorized", "Log in again.")],
    );
    let hub = hub_with(vec![list(&[home(), castle()])], vec![start]);
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Castle"), SCREEN);
    h.click(&card("Castle"));
    h.click(&pane_button("Join"));

    h.wait_for(&L::text("Cannot start \"Castle\""), SCREEN);
    assert!(h.find(&L::role(Role::Button, TRY_AGAIN)).is_empty());
    assert!(matches!(
        h.world().resource::<Account>(),
        Account::LoggedOut
    ));
    h.click(&L::role(Role::Button, "Back to Hosted worlds"));
    h.wait_for(&L::text(LOGGED_OUT), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_confirmation_open_when_the_account_changes_closes_without_its_action() {
    let hub = hub_with(
        vec![list(&[home()])],
        vec![
            ("DELETE", realm_path(&home(), ""), vec![Canned::empty(204)]),
            (
                "POST",
                realm_path(&home(), "/stop"),
                vec![Canned::empty(202)],
            ),
        ],
    );
    let mut h = hosted_tab(&hub, StatusProbe::default());
    h.wait_for(&card("Home"), SCREEN);
    h.click(&pane_button("Delete"));
    let delete = || dialog("Delete \"Home\"?");
    h.wait_for(&delete(), SCREEN);

    // another account logs in while the confirmation is open
    let Account::LoggedIn(mut file) = h.world().resource::<Account>().clone() else {
        panic!("not logged in");
    };
    file.account = Uuid::from_u128(9);
    file.username = "Alex".into();
    file.session = "other".into();
    h.world_mut().insert_resource(Account::LoggedIn(file));
    h.frames(5);
    assert!(!h.is_visible(&delete()));
    h.wait_until("the worlds listed for the new account", SCREEN, |_| {
        hub.requests()
            .iter()
            .any(|request| request.header("authorization") == Some("Bearer other"))
    });
    h.frames(10);
    assert!(requests_to(&hub, "DELETE", &realm_path(&home(), "")).is_empty());
    assert!(requests_to(&hub, "POST", &realm_path(&home(), "/stop")).is_empty());
}
