//! The main menu: its buttons lead to the other screens, the skin shows beside them, and quitting
//! asks first.

use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::prelude::*;
use bevy::ui::{InteractionDisabled, UiGlobalTransform};

use super::harness::{Harness, Options, SCREEN, STARTUP};
use super::helpers::{main_menu, main_menu_with};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::PLAYER_NAME_PATH;
use crate::network::{hash_string_to_u64, CurrentPlayerProfile};
use crate::player::skin::built_in_skin_path;
use crate::ui::assets::TITLE_IMAGE_PATH;
use crate::ui::credits::Credits;
use crate::ui::menus::multi::SERVERS_TITLE;
use crate::ui::menus::player_name::{
    PlayerNameInput, CHOOSE_NAME_TITLE, DEFAULT_PLAYER_NAME, PLAYER_NAME_TITLE,
};
use crate::ui::menus::skins::{SkinPreview, IMPORT_NOTE, SKINS_TITLE};
use crate::ui::menus::solo::WORLDS_TITLE;
use crate::ui::menus::MenuState;
use crate::ui::menus::{BUTTON_COLUMN_WIDTH, CREDITS_LABEL, QUIT_TITLE, SKIN_SHIFT_VH};
use crate::ui::quit::QuittingCard;
use crate::ui::theme;
use crate::ui::widgets::hints::HintBar;

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn singleplayer_opens_the_world_list() {
    let mut h = main_menu();
    h.click(&L::text("Singleplayer"));
    h.wait_for(&L::text(WORLDS_TITLE), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Solo);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn multiplayer_opens_the_server_list() {
    let mut h = main_menu();
    h.click(&L::text("Multiplayer"));
    h.wait_for(&L::text(SERVERS_TITLE), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Multi);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn skin_opens_the_skin_screen() {
    let mut h = main_menu();
    h.click(&L::text(SKINS_TITLE));
    h.wait_for(&L::text(IMPORT_NOTE), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Skins);
    h.click(&L::role(Role::Button, "Back"));
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Main);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_skin_shows_beside_the_buttons_and_fits_every_window() {
    let folder = GameFolder::new().with_file("skin.ron", "(skin: BuiltIn(\"Sailor\"))");
    let mut h = main_menu_with(folder);
    h.wait_for_ui_images();
    for (width, height) in [(800, 600), (1280, 720), (1920, 1080), (2560, 1440)] {
        h.resize(width, height);
        h.frames(5);
        h.snapshot("home");
        let window = Rect::from_corners(Vec2::ZERO, Vec2::new(width as f32, height as f32));
        let preview = h.rect_of(&L::marker::<SkinPreview>());
        let skin = h.rect_of(&L::role(Role::Button, crate::i18n::tr(SKINS_TITLE)));
        let top = h.rect_of(&L::role(Role::Button, "Singleplayer"));
        let bottom = h.rect_of(&L::role(Role::Button, "Quit game"));
        let hints = h.rect_of(&L::marker::<HintBar>());
        let name = h.rect_of(&L::role(Role::Button, "Tester"));
        let at = format!("at {width}x{height}");
        assert!(
            window.contains(preview.min) && window.contains(skin.max),
            "{at}"
        );
        // beside the column, in the middle of the window, and level with it but for its shift
        // down
        assert!(
            (top.center().x - width as f32 / 2.0).abs() < 2.0,
            "{top:?} {at}"
        );
        assert!(top.max.x < preview.min.x, "{top:?} and {preview:?} {at}");
        let skin_middle = (name.min.y + skin.max.y) / 2.0;
        let column_middle = (top.min.y + bottom.max.y) / 2.0;
        let shift = SKIN_SHIFT_VH.y * height as f32 / 100.0;
        assert!(
            (skin_middle - column_middle - shift).abs() < 2.0,
            "{skin_middle} {column_middle} {at}"
        );
        assert!(
            name.max.y <= preview.min.y && preview.max.y <= skin.min.y,
            "{at}"
        );
        assert!(name.min.y > logo_bottom(&mut h), "{name:?} {at}");
        assert!(
            skin.max.y < hints.min.y && bottom.max.y < hints.min.y,
            "{at}"
        );
    }

    // Tab reaches the button, and the preview follows a change of skin
    h.resize(1280, 720);
    while h.focus_label().as_deref() != Some(crate::i18n::tr(SKINS_TITLE)) {
        h.press(KeyCode::Tab);
    }
    h.press(KeyCode::Enter);
    h.wait_for(&L::text(IMPORT_NOTE), SCREEN);
    h.click(&L::text("Miner"));
    h.frames(2);
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    h.wait_for_ui_images();
    h.frames(5);
    h.snapshot("home_as_a_miner");
    let world = h.world_mut();
    let miner: Handle<Image> = world
        .resource::<AssetServer>()
        .load(built_in_skin_path("Miner"));
    let worn = world
        .query_filtered::<&MeshMaterial3d<StandardMaterial>, With<Mesh3d>>()
        .iter(world)
        .next()
        .expect("the model")
        .0
        .clone();
    let worn = world
        .resource::<Assets<StandardMaterial>>()
        .get(&worn)
        .unwrap();
    assert_eq!(worn.base_color_texture.as_ref(), Some(&miner));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn settings_opens_the_settings() {
    let mut h = main_menu();
    h.click(&L::text("Settings"));
    h.wait_for(&L::text("Controls"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Settings);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn quit_exits_the_game() {
    let mut h = main_menu();
    h.click(&L::text("Quit game"));
    h.wait_for(&L::text(crate::i18n::tr(QUIT_TITLE)), SCREEN);
    assert!(!h.has_exited());
    h.click(&L::role(Role::Button, "Quit"));
    assert!(h.has_exited());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn closing_the_window_in_the_menus_quits_at_once() {
    let mut h = main_menu();
    h.request_close();
    assert!(h.has_exited());
    assert!(h.find(&L::marker::<QuittingCard>()).is_empty());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_asks_before_quitting_with_cancel_focused() {
    let mut h = main_menu();
    h.press(KeyCode::Escape);
    h.wait_for(&L::text(crate::i18n::tr(QUIT_TITLE)), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));
    assert!(!h.has_exited());
    // opened from the keyboard, the dialog shows where Enter goes
    h.frames(1);
    let cancel = h.get(&L::role(Role::Button, "Cancel"));
    assert_eq!(
        h.world().get::<Outline>(cancel).unwrap().color,
        theme::accent()
    );
    // and its own key hints, in place of the menu's
    let hints = L::marker::<HintBar>();
    assert_eq!(h.text_of(&hints), "Tab Move Enter Select Esc Cancel");

    // Escape closes the dialog, and the same key does not open it again
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::text(crate::i18n::tr(QUIT_TITLE))));
    h.frames(2);
    assert!(
        !h.is_visible(&L::text(crate::i18n::tr(QUIT_TITLE))),
        "the dialog reopened"
    );
    assert!(!h.has_exited());
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));
    assert!(
        h.text_of(&hints).ends_with("Esc Quit"),
        "{}",
        h.text_of(&hints)
    );

    // Enter on Cancel closes it
    h.press(KeyCode::Escape);
    h.wait_for(&L::text(crate::i18n::tr(QUIT_TITLE)), SCREEN);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert!(!h.is_visible(&L::text(crate::i18n::tr(QUIT_TITLE))));
    assert!(!h.has_exited());

    // Tab then Enter quits
    h.press(KeyCode::Escape);
    h.wait_for(&L::text(crate::i18n::tr(QUIT_TITLE)), SCREEN);
    h.press(KeyCode::Tab);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert!(h.has_exited());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_credits_open_from_the_main_menu_and_escape_leaves_them() {
    for (width, height) in [(1280, 720), (800, 600)] {
        let mut h = main_menu();
        h.resize(width, height);
        h.snapshot("menu");
        h.click_when_ready(&L::role(Role::Button, crate::i18n::tr(CREDITS_LABEL)));
        h.wait_for(&L::marker::<Credits>(), SCREEN);
        // no key reaches the menu under them
        for key in [KeyCode::Tab, KeyCode::ArrowDown, KeyCode::Enter] {
            h.press(key);
        }
        h.frames(2);
        assert_eq!(h.menu_state(), MenuState::Main);
        assert!(!h.is_visible(&L::text(crate::i18n::tr(QUIT_TITLE))));
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            h.frames(5);
        }
        h.snapshot("credits");

        h.press(KeyCode::Escape);
        h.frames(2);
        assert!(h.find(&L::marker::<Credits>()).is_empty());
        assert!(
            !h.is_visible(&L::text(crate::i18n::tr(QUIT_TITLE))),
            "Escape asked to quit"
        );
        assert_eq!(
            h.focus_label().as_deref(),
            Some(crate::i18n::tr(CREDITS_LABEL))
        );
        assert_eq!(h.menu_state(), MenuState::Main);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_opens_a_screen() {
    let mut h = main_menu();
    h.frames(2);
    let ring = |h: &mut Harness, label: &str| {
        let button = h.get(&L::role(Role::Button, label));
        h.world().get::<Outline>(button).unwrap().color
    };
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));
    assert_eq!(ring(&mut h, "Singleplayer"), Color::NONE);

    // the arrows walk the column, then go across to the skin
    for (key, label) in [
        (KeyCode::ArrowDown, "Multiplayer"),
        (KeyCode::ArrowDown, crate::i18n::tr(CREDITS_LABEL)),
        (KeyCode::ArrowDown, "Settings"),
        (KeyCode::ArrowRight, "Quit game"),
        (KeyCode::ArrowRight, crate::i18n::tr(SKINS_TITLE)),
        (KeyCode::ArrowLeft, "Quit game"),
        (KeyCode::ArrowUp, crate::i18n::tr(CREDITS_LABEL)),
    ] {
        h.press(key);
        assert_eq!(h.focus_label().as_deref(), Some(label), "after {key:?}");
    }
    // Tab takes them in the order they read
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Settings"));
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Quit game"));
    h.press(KeyCode::Tab);
    assert_eq!(h.focus_label().as_deref(), Some("Tester"));
    h.press(KeyCode::Tab);
    assert_eq!(
        h.focus_label().as_deref(),
        Some(crate::i18n::tr(SKINS_TITLE))
    );
    h.key_down(KeyCode::ShiftLeft);
    for _ in 0..3 {
        h.press(KeyCode::Tab);
    }
    h.key_up(KeyCode::ShiftLeft);
    assert_eq!(h.focus_label().as_deref(), Some("Settings"));
    assert_eq!(ring(&mut h, "Settings"), theme::accent());

    h.press(KeyCode::Enter);
    h.wait_for(&L::text("Controls"), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Settings);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_button_lightens_it() {
    let mut h = main_menu();
    let singleplayer = L::role(Role::Button, "Singleplayer");
    h.hover(&singleplayer);
    h.frames(1);
    let button = h.get(&singleplayer);
    assert_eq!(
        h.world().get::<BackgroundColor>(button).unwrap().0,
        theme::raise_hover()
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_main_menu_shows_the_version_and_the_key_hints() {
    let mut h = main_menu();
    let version = L::text(&format!("v{}", env!("CARGO_PKG_VERSION")));
    assert!(h.is_visible(&version));
    let center = h.center_of(&version);
    // at the right end of the bar of the key hints
    let hints = h.rect_of(&L::marker::<HintBar>());
    assert!(center.x > 1280.0 * 3.0 / 4.0, "the version is at {center}");
    assert!(
        (center.y - hints.center().y).abs() < 2.0,
        "the version is at {center}"
    );
    assert!(h.is_visible(&L::text("Esc").within(L::marker::<HintBar>())));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn two_harnesses_run_side_by_side() {
    let mut first = main_menu();
    let mut second = main_menu();
    first.click(&L::text("Settings"));
    second.click(&L::text("Multiplayer"));
    first.wait_for(&L::text("Controls"), SCREEN);
    second.wait_for(&L::text(SERVERS_TITLE), SCREEN);
    assert_eq!(first.menu_state(), MenuState::Settings);
    assert_eq!(second.menu_state(), MenuState::Multi);
    assert_ne!(first.folder(), second.folder());
}

/// Where the title image of the main menu is.
fn logo_rect(h: &mut Harness) -> Rect {
    let world = h.world_mut();
    let title: Handle<Image> = world.resource::<AssetServer>().load(TITLE_IMAGE_PATH);
    world
        .query::<(&ImageNode, &ComputedNode, &UiGlobalTransform)>()
        .iter(world)
        .find(|(node, ..)| node.image.id() == title.id())
        .map(|(_, node, transform)| Rect::from_center_size(transform.translation, node.size()))
        .expect("the logo")
}

fn logo_bottom(h: &mut Harness) -> f32 {
    logo_rect(h).max.y
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_logo_and_the_buttons_share_the_middle_of_the_window() {
    let full_hd = Options {
        width: 1920,
        height: 1080,
        ..default()
    };
    let mut h = Harness::start(GameFolder::new(), full_hd);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h.wait_for_ui_images();
    h.frames(1);
    let logo = logo_rect(&mut h);
    let wide = h.rect_of(&L::role(Role::Button, "Singleplayer"));
    let settings = h.rect_of(&L::role(Role::Button, "Settings"));
    let quit = h.rect_of(&L::role(Role::Button, "Quit game"));
    assert!((logo.center().x - 960.0).abs() < 1.0, "{logo:?}");
    assert!((wide.center().x - 960.0).abs() < 1.0, "{wide:?}");
    assert!(logo.max.y < wide.min.y);
    // the column is as wide at every scale of the UI, and the last row is two halves of it
    assert!(
        (wide.width() - BUTTON_COLUMN_WIDTH * 1.5).abs() < 1.0,
        "{wide:?}"
    );
    assert!((settings.min.x - wide.min.x).abs() < 1.0 && (quit.max.x - wide.max.x).abs() < 1.0);
    assert!((settings.width() - quit.width()).abs() < 1.0);
}

/// The name saved in the game folder of `h`.
fn saved_name(h: &Harness) -> Option<String> {
    std::fs::read_to_string(h.folder().join(PLAYER_NAME_PATH)).ok()
}

/// A client started as on a first launch: no name given.
fn first_launch(folder: GameFolder) -> Harness {
    let options = Options {
        player_name: None,
        ..Options::default()
    };
    let mut h = Harness::start(folder, options);
    h.wait_for(&L::text("Singleplayer"), STARTUP);
    h
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_first_launch_asks_for_a_name_and_saves_it() {
    let mut h = first_launch(GameFolder::new());
    let dialog = || L::role(Role::Dialog, crate::i18n::tr(CHOOSE_NAME_TITLE));
    h.wait_for(&dialog(), SCREEN);
    h.frames(2);
    h.snapshot("choose_your_name");
    let save = h.get(&L::role(Role::Button, "Save").within(dialog()));
    assert!(h.world().get::<InteractionDisabled>(save).is_some());

    h.type_text("Steve");
    h.frames(2);
    assert!(h.world().get::<InteractionDisabled>(save).is_none());
    h.press(KeyCode::Enter);
    h.frames(3);

    assert!(!h.is_visible(&dialog()));
    assert_eq!(h.world().resource::<CurrentPlayerProfile>().name, "Steve");
    assert!(h.is_visible(&L::role(Role::Button, "Steve")));
    assert!(saved_name(&h).unwrap().contains("\"Steve\""));
    assert_eq!(h.focus_label().as_deref(), Some("Singleplayer"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_first_launch_dismissed_plays_as_player_and_saves_nothing() {
    let mut h = first_launch(GameFolder::new());
    h.wait_for(
        &L::role(Role::Dialog, crate::i18n::tr(CHOOSE_NAME_TITLE)),
        SCREEN,
    );
    h.press(KeyCode::Escape);
    h.frames(3);
    assert!(!h.is_visible(&L::role(Role::Dialog, crate::i18n::tr(CHOOSE_NAME_TITLE))));
    assert_eq!(
        h.world().resource::<CurrentPlayerProfile>().name,
        DEFAULT_PLAYER_NAME
    );
    assert_eq!(saved_name(&h), None);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_saved_name_is_used_without_asking() {
    let folder = GameFolder::new().with_file(PLAYER_NAME_PATH, "(name: \"Alex\")");
    let mut h = first_launch(folder);
    h.frames(5);
    assert!(h
        .find(&L::role(Role::Dialog, crate::i18n::tr(CHOOSE_NAME_TITLE)))
        .is_empty());
    assert_eq!(h.world().resource::<CurrentPlayerProfile>().name, "Alex");
    assert!(h.is_visible(&L::role(Role::Button, "Alex")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_name_given_on_the_command_line_wins_over_the_saved_one() {
    let folder = GameFolder::new().with_file(PLAYER_NAME_PATH, "(name: \"Alex\")");
    let h = main_menu_with(folder);
    assert_eq!(h.world().resource::<CurrentPlayerProfile>().name, "Tester");
    assert!(saved_name(&h).unwrap().contains("\"Alex\""));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_name_button_changes_the_name() {
    let mut h = main_menu();
    h.click(&L::role(Role::Button, "Tester"));
    let dialog = || L::role(Role::Dialog, crate::i18n::tr(PLAYER_NAME_TITLE));
    h.wait_for(&dialog(), SCREEN);
    h.frames(2);
    assert_eq!(h.value_of::<PlayerNameInput>(), "Tester");
    let save = h.get(&L::role(Role::Button, "Save").within(dialog()));
    assert!(h.world().get::<InteractionDisabled>(save).is_some());

    h.type_text("Notch");
    h.frames(2);
    h.press(KeyCode::Enter);
    h.frames(3);

    assert!(!h.is_visible(&dialog()));
    let profile = h.world().resource::<CurrentPlayerProfile>();
    assert_eq!(profile.name, "Notch");
    assert_eq!(profile.id, hash_string_to_u64("Notch"));
    assert!(h.is_visible(&L::role(Role::Button, "Notch")));
    assert!(saved_name(&h).unwrap().contains("\"Notch\""));
}
