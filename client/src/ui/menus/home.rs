//! The main menu: the logo, the buttons to the other screens, and a confirmation before quitting.

use bevy::{
    input_focus::{tab_navigation::TabGroup, AutoFocus, InputFocus},
    prelude::*,
    ui_widgets::Activate,
};

use super::background::menu_background;
use super::MenuState;
use crate::ui::assets::{load_font, load_title_image, MONO_FONT_PATH};
use crate::ui::theme::{FONT_S, MUTED, SPACE_S, SPACE_XL, SPACE_XXL};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{spawn_dialog, Dialog, DialogClosed};
use crate::ui::widgets::focus::ArrowNav;
use crate::ui::widgets::hints::{hint_bar, Hint};

/// The buttons of the main menu.
#[derive(Component, Clone, Copy)]
pub enum MainMenuButton {
    Singleplayer,
    Multiplayer,
    Settings,
    Quit,
}

/// The title of the confirmation asked before quitting.
pub const QUIT_TITLE: &str = "Quit Rustcraft?";

/// The key hints of the main menu.
pub const MAIN_HINTS: [Hint; 3] = [
    Hint {
        keys: "Tab",
        action: "Move",
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

/// Spawns the main menu, the focus on Singleplayer.
pub fn home_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
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
                padding: UiRect::all(px(SPACE_XXL)),
                row_gap: px(SPACE_XXL),
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
            ..default()
        },
        ChildOf(content),
    ));
    // the right side is left to the card of the last world
    let row = commands.spawn((Node::default(), ChildOf(content))).id();
    let column = commands
        .spawn((
            ArrowNav,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                width: px(280),
                ..default()
            },
            ChildOf(row),
        ))
        .id();
    let buttons = [
        (MainMenuButton::Singleplayer, "Singleplayer"),
        (MainMenuButton::Multiplayer, "Multiplayer"),
        (MainMenuButton::Settings, "Settings"),
    ];
    for (index, (action, label)) in buttons.into_iter().enumerate() {
        let mut entity = commands.spawn((
            button(label, ButtonVariant::Default, font.clone()),
            action,
            ChildOf(column),
        ));
        if index == 0 {
            entity.insert(AutoFocus);
        }
    }
    commands
        .spawn((
            button("Quit game", ButtonVariant::Ghost, font.clone()),
            MainMenuButton::Quit,
            ChildOf(column),
        ))
        .entry::<Node>()
        .and_modify(|mut node| node.margin = UiRect::top(px(SPACE_XL)));
    commands.spawn((
        Text(format!("v{}", env!("CARGO_PKG_VERSION"))),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_S).into(),
            ..default()
        },
        TextColor(MUTED),
        Node {
            position_type: PositionType::Absolute,
            top: px(SPACE_XXL),
            right: px(SPACE_XXL),
            ..default()
        },
        ChildOf(root),
    ));
    commands.spawn((hint_bar(&MAIN_HINTS, font, glyph_font), ChildOf(root)));
}

/// Whether a Control key is held: Ctrl+Enter belongs to the card of the last world, not to the
/// focused button.
fn ctrl_held(keys: &ButtonInput<KeyCode>) -> bool {
    keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
}

/// Carries out the button of the main menu that was activated.
pub fn activate_main_menu_button(
    event: On<Activate>,
    buttons: Query<&MainMenuButton>,
    keys: Res<ButtonInput<KeyCode>>,
    mut menu_state: ResMut<NextState<MenuState>>,
    asset_server: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let Ok(action) = buttons.get(event.entity) else {
        return;
    };
    if ctrl_held(&keys) {
        return;
    }
    match action {
        MainMenuButton::Singleplayer => menu_state.set(MenuState::Solo),
        MainMenuButton::Multiplayer => menu_state.set(MenuState::Multi),
        MainMenuButton::Settings => menu_state.set(MenuState::Settings),
        MainMenuButton::Quit => open_quit_dialog(&mut commands, &asset_server, &focus),
    }
}

/// Asks "Quit Rustcraft?" when Escape is pressed on the main menu with no dialog open.
pub fn open_quit_dialog_on_escape(
    keys: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    if keys.just_pressed(KeyCode::Escape) {
        open_quit_dialog(&mut commands, &asset_server, &focus);
    }
}

/// Opens the quit confirmation, Cancel focused; its Quit exits the game.
fn open_quit_dialog(commands: &mut Commands, asset_server: &Res<AssetServer>, focus: &InputFocus) {
    let dialog = Dialog {
        title: QUIT_TITLE,
        body: "",
        actions: &[
            ("Cancel", ButtonVariant::Default),
            ("Quit", ButtonVariant::Danger),
        ],
        focused: 0,
    };
    let root = spawn_dialog(commands, &dialog, load_font(asset_server), focus);
    commands.entity(root).observe(
        |closed: On<DialogClosed>, mut exit: MessageWriter<AppExit>| {
            if closed.action == Some(1) {
                exit.write(AppExit::Success);
            }
        },
    );
}
