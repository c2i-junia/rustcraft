//! The Account dialog of the main menu, against a stand-in for the hub.

use accesskit::Role;
use bevy::clipboard::Clipboard;
use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui::InteractionDisabled;

use super::game::slot;
use super::harness::{Harness, Options, SCREEN, SERVER, STARTUP};
use super::helpers::ctrl;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::account::file::{unix_now, AccountFile};
use crate::account::test_hub::{Canned, TestHub};
use crate::account::Account;
use crate::constants::{ACCOUNT_PATH, PLAYER_NAME_PATH};
use crate::network::{hash_string_to_u64, CurrentPlayerProfile};
use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::ui::menus::account::{ACCOUNT_TITLE, RECOVERY_CODE_TITLE, RECOVER_TITLE};
use crate::ui::menus::player_name::PLAYER_NAME_TITLE;
use crate::ui::menus::skins::SkinPreviewModel;
use rustcraft_protocol::api::AccountSummary;
use rustcraft_protocol::identity::player_id;
use shared::players::Player;
use shared::world::ItemId;
use uuid::Uuid;

const SESSION: &str = r#"{"account":{"id":"00000000-0000-0000-0000-000000000001","username":"Steve"},"session":"tok"}"#;
const CREATED: &str = r#"{"account":{"id":"00000000-0000-0000-0000-000000000001","username":"Steve"},"session":"tok","recovery_code":"RCV-7K2QF-M9XWA-4HTPE"}"#;
const ME: &str = r#"{"id":"00000000-0000-0000-0000-000000000001","username":"Steve","roles":[],"realm_quota":0,"created_at":1}"#;
const PASSWORD: &str = "correct horse battery";
/// In 2100: a certificate that needs no renewal.
const FAR: u64 = 4_102_444_800;

fn certificate_json() -> String {
    format!(
        r#"{{"certificate":"AQID","signature":"{}==","expires_at":{FAR}}}"#,
        "A".repeat(86)
    )
}

/// A client with a saved name, so that it asks for none, on the hub at `hub`.
pub(super) fn client(folder: GameFolder, hub: &str) -> Harness {
    let options = Options {
        player_name: None,
        hub: Some(hub.to_string()),
        ..Options::default()
    };
    let mut h = Harness::start(folder, options);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h
}

pub(super) fn logged_out_folder() -> GameFolder {
    GameFolder::new().with_file(PLAYER_NAME_PATH, "(name: \"Alex\")")
}

/// A folder of a player logged in as Steve to `hub`, with a certificate that needs no renewal.
pub(super) fn logged_in_folder(hub: &str) -> GameFolder {
    let folder = GameFolder::new().with_file(PLAYER_NAME_PATH, "(name: \"Alex\")");
    let summary = AccountSummary {
        id: Uuid::from_u128(1),
        username: "Steve".into(),
    };
    let mut file = AccountFile::new(hub, &summary, "tok".into());
    file.certificate = Some("AQID".into());
    file.signature = Some(format!("{}==", "A".repeat(86)));
    file.expires_at = Some(FAR);
    file.save(&folder.path().join(ACCOUNT_PATH)).unwrap();
    folder
}

fn dialog(title: &str) -> L {
    L::role(Role::Dialog, title)
}

fn button(label: &str, title: &str) -> L {
    L::role(Role::Button, label).within(dialog(title))
}

/// Checks the screen against the golden `name`, the model of the skin facing the player rather
/// than however far it turned.
#[track_caller]
fn golden(h: &mut Harness, name: &str) {
    let world = h.world_mut();
    let mut models = world.query_filtered::<(Entity, &mut Transform), With<SkinPreviewModel>>();
    if let Ok((model, mut transform)) = models.single_mut(world) {
        transform.rotation = Quat::IDENTITY;
        world.entity_mut(model).remove::<SkinPreviewModel>();
    }
    h.assert_screenshot(name);
}

/// Opens the Account dialog from the name button, named `name`.
#[track_caller]
fn open_account(h: &mut Harness, name: &str) {
    h.click(&L::role(Role::Button, name));
    h.wait_for(&dialog(ACCOUNT_TITLE), SCREEN);
    h.frames(2);
}

/// Types `username` and `password` in the log-in form, the focus being on its first field.
fn fill_in(h: &mut Harness, username: &str, password: &str) {
    h.type_text(username);
    h.press(KeyCode::Tab);
    h.type_text(password);
    h.frames(2);
}

fn value_of_field(h: &mut Harness, label: &str) -> String {
    let field = h.get(&L::role(Role::TextInput, label));
    h.world()
        .get::<EditableText>(field)
        .unwrap()
        .value()
        .to_string()
}

fn account(h: &Harness) -> Account {
    h.world().resource::<Account>().clone()
}

/// Runs frames until the hub has received `count` requests.
#[track_caller]
fn wait_for_requests(h: &mut Harness, hub: &TestHub, count: usize) {
    h.wait_until("the requests to the hub", SCREEN, |_| {
        hub.requests().len() >= count
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn logging_in_makes_the_account_the_player_and_asks_for_a_certificate() {
    let hub = TestHub::start(vec![Canned::ok(SESSION), Canned::ok(&certificate_json())]);
    let mut h = client(logged_out_folder(), &hub.url);
    open_account(&mut h, "Alex");
    golden(&mut h, "account_login");
    let log_in = h.get(&button("Log in", ACCOUNT_TITLE));
    assert!(h.world().get::<InteractionDisabled>(log_in).is_some());
    assert!(h
        .find(&L::role(Role::TextInput, "Confirm password"))
        .is_empty());

    h.type_text("Steve");
    h.press(KeyCode::Tab);
    h.type_text(PASSWORD);
    h.frames(3);
    // the field keeps the password and the screen shows bullets
    assert_eq!(value_of_field(&mut h, "Password"), PASSWORD);
    assert!(h.is_visible(&L::text(&"•".repeat(PASSWORD.chars().count()))));
    assert!(h.world().get::<InteractionDisabled>(log_in).is_none());
    h.press(KeyCode::Enter);

    h.wait_for(&L::role(Role::Button, "Steve"), SCREEN);
    assert!(!h.is_visible(&dialog(ACCOUNT_TITLE)));
    assert_eq!(h.world().resource::<CurrentPlayerProfile>().name, "Steve");
    let Account::LoggedIn(file) = account(&h) else {
        panic!("not logged in");
    };
    assert_eq!(file.username, "Steve");
    assert_eq!(file.hub, hub.url);
    let saved = AccountFile::load(&h.folder().join(ACCOUNT_PATH))
        .unwrap()
        .unwrap();
    assert_eq!(saved.session, "tok");
    // a certificate is asked at once
    wait_for_requests(&mut h, &hub, 2);
    let requests = hub.requests();
    assert_eq!(requests[0].path, "/v1/sessions");
    assert!(requests[0].body.contains(PASSWORD));
    assert_eq!(requests[1].path, "/v1/certificates");
    assert_eq!(requests[1].header("authorization"), Some("Bearer tok"));
    h.wait_until("the certificate", SCREEN, |world| {
        matches!(world.resource::<Account>(), Account::LoggedIn(file) if file.expires_at == Some(FAR))
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn copying_or_cutting_a_password_leaves_the_clipboard_alone() {
    let mut h = client(logged_out_folder(), "http://127.0.0.1:1");
    open_account(&mut h, "Alex");
    let sentinel = "not the password";
    let has_clipboard = h
        .world_mut()
        .get_resource_mut::<Clipboard>()
        .is_some_and(|mut clipboard| clipboard.set_text(sentinel).is_ok());
    fill_in(&mut h, "Steve", PASSWORD);
    for key in [KeyCode::KeyA, KeyCode::KeyC, KeyCode::KeyX] {
        ctrl(&mut h, key);
        h.frames(2);
    }
    // nothing was cut, and the clipboard was not written
    assert_eq!(value_of_field(&mut h, "Password"), PASSWORD);
    if has_clipboard {
        let mut read = h.world_mut().resource_mut::<Clipboard>().fetch_text();
        h.frames(2);
        if let Some(Ok(text)) = read.poll_result() {
            assert_eq!(text, sentinel);
        }
    }
    // typing still works
    h.press(KeyCode::End);
    h.type_text("!");
    assert_eq!(value_of_field(&mut h, "Password"), format!("{PASSWORD}!"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_wrong_password_is_said_under_the_field_and_the_dialog_stays() {
    let hub = TestHub::start(vec![Canned::error(
        401,
        "bad_credentials",
        "Wrong username or password.",
    )]);
    let mut h = client(logged_out_folder(), &hub.url);
    open_account(&mut h, "Alex");
    fill_in(&mut h, "Steve", PASSWORD);
    h.press(KeyCode::Enter);

    h.wait_for(&L::text("Wrong username or password."), SCREEN);
    assert!(h.is_visible(&dialog(ACCOUNT_TITLE)));
    assert_eq!(account(&h), Account::LoggedOut);
    assert!(!h.folder().join(ACCOUNT_PATH).exists());
    // the form is usable again
    let log_in = h.get(&button("Log in", ACCOUNT_TITLE));
    assert!(h.world().get::<InteractionDisabled>(log_in).is_none());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_hub_that_cannot_be_reached_is_said_in_a_toast() {
    // a port that was free a moment ago
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let mut h = client(logged_out_folder(), &format!("http://127.0.0.1:{port}"));
    open_account(&mut h, "Alex");
    fill_in(&mut h, "Steve", PASSWORD);
    h.press(KeyCode::Enter);

    h.wait_for(
        &L::text(&format!("Cannot reach the hub at 127.0.0.1:{port}.")),
        SCREEN,
    );
    assert!(h.is_visible(&dialog(ACCOUNT_TITLE)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn creating_an_account_shows_the_recovery_code_until_it_is_written_down() {
    let hub = TestHub::start(vec![Canned::ok(CREATED), Canned::ok(&certificate_json())]);
    let mut h = client(logged_out_folder(), &hub.url);
    open_account(&mut h, "Alex");
    h.click(&L::role(Role::Tab, "Create account"));
    h.wait_for(&L::role(Role::TextInput, "Confirm password"), SCREEN);
    let create = h.get(&button("Create account", ACCOUNT_TITLE));
    assert!(!h.is_visible(&L::text("Forgot password?")));

    h.click(&L::role(Role::TextInput, "Username"));
    h.type_text("Steve");
    h.press(KeyCode::Tab);
    h.type_text(PASSWORD);
    h.press(KeyCode::Tab);
    h.type_text("correct horse");
    h.frames(3);
    h.wait_for(&L::text("Passwords differ"), SCREEN);
    assert!(h.world().get::<InteractionDisabled>(create).is_some());
    h.type_text(" battery");
    h.frames(3);
    assert!(h.world().get::<InteractionDisabled>(create).is_none());
    h.press(KeyCode::Enter);

    h.wait_for(&dialog(RECOVERY_CODE_TITLE), SCREEN);
    h.wait_for(&L::text("RCV-7K2QF-M9XWA-4HTPE"), SCREEN);
    h.frames(2);
    golden(&mut h, "account_created");
    assert!(matches!(account(&h), Account::LoggedIn(_)));
    // the code is shown again until the button is pressed
    h.press(KeyCode::Escape);
    h.frames(4);
    h.wait_for(&L::text("RCV-7K2QF-M9XWA-4HTPE"), SCREEN);
    h.click(&button("I wrote it down", RECOVERY_CODE_TITLE));
    h.frames(4);
    assert!(!h.is_visible(&dialog(RECOVERY_CODE_TITLE)));
    assert!(!h.is_visible(&L::text("RCV-7K2QF-M9XWA-4HTPE")));
    let requests = hub.requests();
    assert_eq!(requests[0].path, "/v1/accounts");
    assert!(requests[0].body.contains("correct horse battery"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_forgotten_password_is_recovered_with_the_code() {
    let hub = TestHub::start(vec![Canned::ok(CREATED), Canned::ok(&certificate_json())]);
    let mut h = client(logged_out_folder(), &hub.url);
    open_account(&mut h, "Alex");
    h.click(&L::text("Forgot password?"));
    h.wait_for(&dialog(RECOVER_TITLE), SCREEN);
    h.frames(2);
    h.type_text("Steve");
    h.press(KeyCode::Tab);
    h.type_text("RCV-OLD00-OLD00-OLD00");
    h.press(KeyCode::Tab);
    h.type_text("a brand new password");
    h.frames(3);
    h.press(KeyCode::Enter);

    // a new code comes with the new password
    h.wait_for(&dialog(RECOVERY_CODE_TITLE), SCREEN);
    assert!(matches!(account(&h), Account::LoggedIn(_)));
    let requests = hub.requests();
    assert_eq!(requests[0].path, "/v1/recovery");
    assert!(requests[0].body.contains("a brand new password"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_name_dialog_is_reached_from_the_account_dialog() {
    let mut h = client(logged_out_folder(), "http://127.0.0.1:1");
    open_account(&mut h, "Alex");
    h.click(&button("Play offline as...", ACCOUNT_TITLE));
    h.wait_for(&dialog(PLAYER_NAME_TITLE), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_logged_in_account_shows_its_hub_and_logs_out() {
    let hub = TestHub::start(vec![
        Canned::ok(ME),
        Canned {
            status: 204,
            body: String::new(),
        },
    ]);
    let mut h = client(logged_in_folder(&hub.url), &hub.url);
    assert!(h.is_visible(&L::role(Role::Button, "Steve")));
    open_account(&mut h, "Steve");
    // the check of the session is the first request: the log-out is the second
    wait_for_requests(&mut h, &hub, 1);
    assert_eq!(hub.requests()[0].path, "/v1/me");
    h.wait_for(
        &L::text("Certificate valid until 2100-01-01 00:00 UTC"),
        SCREEN,
    );
    h.wait_for(
        &L::text(&format!(
            "Signed in to {}",
            hub.url.trim_start_matches("http://")
        )),
        SCREEN,
    );

    h.click(&button("Log out", ACCOUNT_TITLE));
    h.wait_for(&L::role(Role::Button, "Alex"), SCREEN);
    assert_eq!(account(&h), Account::LoggedOut);
    assert!(!h.folder().join(ACCOUNT_PATH).exists());
    h.wait_for(&L::text("Logged out."), SCREEN);
    wait_for_requests(&mut h, &hub, 2);
    let requests = hub.requests();
    assert_eq!(requests[1].method, "DELETE");
    assert_eq!(requests[1].path, "/v1/sessions/current");
    assert_eq!(requests[1].header("authorization"), Some("Bearer tok"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_signed_in_dialog_matches_its_golden() {
    let hub = "http://127.0.0.1:1";
    let mut h = client(logged_in_folder(hub), hub);
    open_account(&mut h, "Steve");
    h.wait_for(
        &L::text("Certificate valid until 2100-01-01 00:00 UTC"),
        SCREEN,
    );
    golden(&mut h, "account_signed_in");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_session_the_hub_no_longer_knows_logs_out() {
    let hub = TestHub::start(vec![Canned::error(401, "unauthorized", "Log in again.")]);
    let mut h = client(logged_in_folder(&hub.url), &hub.url);
    h.click(&L::role(Role::Button, "Steve"));

    h.wait_for(&L::text("Your session ended. Log in again."), SCREEN);
    assert_eq!(account(&h), Account::LoggedOut);
    assert!(!h.folder().join(ACCOUNT_PATH).exists());
    h.frames(3);
    assert!(!h.is_visible(&dialog(ACCOUNT_TITLE)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_logged_in_player_is_in_its_solo_worlds_the_player_of_its_name() {
    let hub = "http://127.0.0.1:1";
    let folder = logged_in_folder(hub).with_world("Kept").with_items(
        "Kept",
        "Steve",
        &[(0, ItemId::Stick, 5)],
    );
    let mut h = client(folder, hub);
    assert_eq!(
        h.world().resource::<CurrentPlayerProfile>().id,
        player_id(&Uuid::from_u128(1))
    );
    h.click(&L::text("Singleplayer"));
    h.play_world("Kept");
    h.wait_for_game_state(GameState::Game, SERVER);
    h.wait_until("the player of the name Steve", SERVER, |w| {
        w.try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
            .and_then(|mut players| players.single(w).ok().map(|player| player.id))
            == Some(hash_string_to_u64("Steve"))
    });
    h.wait_until("the inventory saved before the log-in", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Stick, 5))
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_session_the_hub_no_longer_knows_at_renewal_logs_out() {
    let hub = TestHub::start(vec![Canned::error(401, "unauthorized", "Log in again.")]);
    let folder = logged_in_folder(&hub.url);
    let path = folder.path().join(ACCOUNT_PATH);
    let mut file = AccountFile::load(&path).unwrap().unwrap();
    // a certificate past half of its life, which the game renews at start
    file.expires_at = Some(unix_now() + 60);
    file.save(&path).unwrap();
    let mut h = client(folder, &hub.url);

    h.wait_for(&L::text("Your session ended. Log in again."), SCREEN);
    assert_eq!(hub.requests()[0].path, "/v1/certificates");
    assert_eq!(account(&h), Account::LoggedOut);
    assert!(!h.folder().join(ACCOUNT_PATH).exists());
    h.wait_for(&L::role(Role::Button, "Alex"), SCREEN);
}
