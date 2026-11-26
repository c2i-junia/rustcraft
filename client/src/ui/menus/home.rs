//! The main menu: the logo, a column of buttons to the other screens, beside it the model wearing
//! the chosen skin between the player's name and the Skin button, and a confirmation before
//! quitting.

use bevy::{
    input::keyboard::KeyboardInput,
    input_focus::{
        tab_navigation::TabGroup, AutoFocus, FocusCause, FocusedInput, InputFocus,
        InputFocusVisible,
    },
    prelude::*,
    ui_widgets::{observe, Activate},
};

use super::background::menu_background;
use super::player_name::{change_name_on_activate, PlayerNameButton};
use super::skins::{
    spawn_home_preview, HomePreview, HOME_PREVIEW_MAX_WIDTH, HOME_PREVIEW_WIDTH_VH, SKINS_TITLE,
};
use super::MenuState;
use crate::network::leave::QuitRequested;
use crate::network::CurrentPlayerProfile;
use crate::ui::assets::{load_font, load_title_image, MONO_FONT_PATH};
use crate::ui::credits::{spawn_credits, Credits};
use crate::ui::hud::progress::PlayerProgress;
use crate::ui::theme::{FONT_S, MUTED, SPACE_L, SPACE_S, SPACE_XXL};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{spawn_dialog, Dialog, DialogClosed};
use crate::ui::widgets::focus::{keep_focus_inside, ArrowNav};
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};

/// The buttons of the main menu.
#[derive(Component, Clone, Copy)]
pub enum MainMenuButton {
    Singleplayer,
    Multiplayer,
    Skin,
    Settings,
    Credits,
    Quit,
}

/// The title of the confirmation asked before quitting.
pub const QUIT_TITLE: &str = "home.quit_title";
/// The button of the credits.
pub const CREDITS_LABEL: &str = "home.credits";

/// The key hints of the main menu.
pub const MAIN_HINTS: [Hint; 4] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "↑↓ ←→",
        action: "Choose",
    },
    Hint {
        keys: "Enter",
        action: "Select",
    },
    Hint {
        keys: "Esc",
        action: "Quit",
    },
];

/// The size of the title image, shown at its own size: its pixels stay crisp.
const LOGO_SIZE: Vec2 = Vec2::new(601.0, 94.0);
/// The space above the logo, in percent of the height of the window.
const LOGO_TOP_VH: f32 = 9.0;

/// The width of the column of buttons, and the space between two of its rows.
pub const BUTTON_COLUMN_WIDTH: f32 = 400.0;
const BUTTON_GAP: f32 = SPACE_S;
/// The space between the column and the skin beside it.
const SIDE_GAP: f32 = SPACE_XXL;

/// Spawns the main menu, the focus on Singleplayer: the logo at the top, under it a column of
/// wide buttons in the middle of the window, the skin beside it on the right, balanced by as much
/// room on the left.
pub fn home_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    profile: Res<CurrentPlayerProfile>,
    mut preview: HomePreview,
) {
    let font = load_font(&asset_server);
    let glyph_font = asset_server.load(MONO_FONT_PATH);

    let root = commands
        .spawn((
            menu_background(&asset_server),
            DespawnOnExit(MenuState::Main),
            TabGroup::new(0),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect {
                    top: vh(LOGO_TOP_VH),
                    bottom: px(HINT_BAR_HEIGHT),
                    ..default()
                },
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        ImageNode::new(load_title_image(&asset_server)),
        Node {
            width: px(LOGO_SIZE.x),
            height: px(LOGO_SIZE.y),
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(content),
    ));
    // the buttons and the skin, in the middle of the room under the logo, a little above it
    let room = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                padding: UiRect::bottom(vh(10)),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    let row = commands
        .spawn((
            ArrowNav,
            Node {
                column_gap: px(SIDE_GAP),
                align_items: AlignItems::Center,
                ..default()
            },
            ChildOf(room),
        ))
        .id();
    // as wide as the skin, so that the column stays in the middle of the window
    commands.spawn((
        Node {
            width: vh(HOME_PREVIEW_WIDTH_VH),
            max_width: px(HOME_PREVIEW_MAX_WIDTH),
            ..default()
        },
        ChildOf(row),
    ));
    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(BUTTON_GAP),
                width: px(BUTTON_COLUMN_WIDTH),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    let wide = |commands: &mut Commands, label: &str, action: MainMenuButton| {
        commands
            .spawn((
                button(label, ButtonVariant::Default, font.clone()),
                action,
                ChildOf(column),
            ))
            .id()
    };
    let singleplayer = wide(&mut commands, "Singleplayer", MainMenuButton::Singleplayer);
    commands.entity(singleplayer).insert(AutoFocus);
    wide(&mut commands, "Multiplayer", MainMenuButton::Multiplayer);
    wide(&mut commands, crate::i18n::tr(CREDITS_LABEL), MainMenuButton::Credits);
    // Settings and Quit, half as wide each, a little apart from the rows above
    let pair = commands
        .spawn((
            Node {
                column_gap: px(BUTTON_GAP),
                margin: UiRect::top(px(SPACE_S)),
                ..default()
            },
            ChildOf(column),
        ))
        .id();
    for (label, action) in [
        ("Settings", MainMenuButton::Settings),
        ("Quit game", MainMenuButton::Quit),
    ] {
        commands
            .spawn((
                button(label, ButtonVariant::Default, font.clone()),
                action,
                ChildOf(pair),
            ))
            .entry::<Node>()
            .and_modify(|mut node| {
                node.flex_grow = 1.0;
                node.flex_basis = px(0);
            });
    }
    // the player's name over the model, as a name tag, and the Skin button under it
    let skin = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_S),
                flex_shrink: 0.0,
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    commands.spawn((
        button(&profile.name, ButtonVariant::Ghost, font.clone()),
        PlayerNameButton,
        observe(change_name_on_activate),
        ChildOf(skin),
    ));
    spawn_home_preview(&mut commands, skin, &asset_server, &mut preview);
    commands
        .spawn((
            button(SKINS_TITLE, ButtonVariant::Default, font.clone()),
            MainMenuButton::Skin,
            ChildOf(skin),
        ))
        .entry::<Node>()
        .and_modify(|mut node| node.align_self = AlignSelf::Stretch);
    commands.spawn((
        hint_bar(&MAIN_HINTS, font.clone(), glyph_font),
        ChildOf(root),
    ));
    // the version, at the right end of the bar of the key hints
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            right: px(SPACE_L),
            bottom: px(0),
            height: px(HINT_BAR_HEIGHT),
            align_items: AlignItems::Center,
            ..default()
        },
        Pickable::IGNORE,
        ChildOf(root),
        children![(
            Text(format!("v{}", env!("CARGO_PKG_VERSION"))),
            TextFont {
                font: font.into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(MUTED),
        )],
    ));
}

/// Carries out the button of the main menu that was activated.
pub fn activate_main_menu_button(
    event: On<Activate>,
    buttons: Query<&MainMenuButton>,
    mut menu_state: ResMut<NextState<MenuState>>,
    asset_server: Res<AssetServer>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    let Ok(action) = buttons.get(event.entity) else {
        return;
    };
    match action {
        MainMenuButton::Singleplayer => menu_state.set(MenuState::Solo),
        MainMenuButton::Multiplayer => menu_state.set(MenuState::Multi),
        MainMenuButton::Skin => menu_state.set(MenuState::Skins),
        MainMenuButton::Settings => menu_state.set(MenuState::Settings),
        MainMenuButton::Credits => {
            open_credits(&mut commands, &asset_server, &mut focus, event.entity);
        }
        MainMenuButton::Quit => open_quit_dialog(&mut commands, &asset_server, &focus),
    }
}

/// Opens the credits over the main menu. They hold the focus, so that no key reaches the buttons
/// under them, and give it back to `button` once they are gone.
fn open_credits(
    commands: &mut Commands,
    asset_server: &AssetServer,
    focus: &mut InputFocus,
    button: Entity,
) {
    // No game, no statistics
    let root = spawn_credits(commands, asset_server, &PlayerProgress::default());
    commands
        .entity(root)
        .insert(DespawnOnExit(MenuState::Main))
        .observe(|mut event: On<FocusedInput<KeyboardInput>>| event.propagate(false))
        .observe(
            move |_: On<Remove, Credits>,
                  buttons: Query<(), With<MainMenuButton>>,
                  mut focus: ResMut<InputFocus>| {
                if buttons.contains(button) {
                    focus.set(button, FocusCause::Navigated);
                }
            },
        );
    keep_focus_inside(commands, root, Some(root));
    focus.set(root, FocusCause::Navigated);
}

/// Asks "Quit Rustcraft?" when Escape is pressed on the main menu with no dialog open. Opened
/// from the keyboard, the dialog shows the focus ring on Cancel, where Enter goes.
pub fn open_quit_dialog_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        open_quit_dialog(&mut commands, &asset_server, &focus);
        focus_visible.0 = true;
    }
}

/// Opens the quit confirmation, Cancel focused; its Quit asks to quit, as the window's close
/// button does: a world still saving is waited for.
fn open_quit_dialog(commands: &mut Commands, asset_server: &Res<AssetServer>, focus: &InputFocus) {
    let dialog = Dialog {
        title: crate::i18n::tr(QUIT_TITLE),
        body: "",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Quit", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, load_font(asset_server), focus);
    commands
        .entity(root)
        .observe(|closed: On<DialogClosed>, mut commands: Commands| {
            if closed.action == Some(1) {
                commands.trigger(QuitRequested);
            }
        });
}
