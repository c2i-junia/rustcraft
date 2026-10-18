//! The main menu: the logo, the buttons to the other screens, the card of the last world played,
//! and a confirmation before quitting.

use std::time::SystemTime;

use bevy::{
    input_focus::{tab_navigation::TabGroup, AutoFocus, InputFocus},
    prelude::*,
    ui_widgets::{observe, Activate},
};

use shared::GameFolderPaths;

use super::background::menu_background;
use super::solo::SelectedWorld;
use super::worlds::{last_played_world, played_ago, start_world, thumbnail_color, WorldSummary};
use super::MenuState;
use crate::constants::SAVE_PATH;
use crate::state::GameState;
use crate::ui::assets::{load_font, load_title_image, MONO_FONT_PATH};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_L, FONT_S, MUTED, PANEL, SPACE_L, SPACE_M, SPACE_S,
    SPACE_XL, SPACE_XXL, TEXT,
};
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

/// The card of the last world played, on the right of the main menu: its Play button, and
/// Ctrl+Enter, play `world`.
#[derive(Component)]
pub struct ContinueCard {
    pub world: String,
}

/// The title of the confirmation asked before quitting.
pub const QUIT_TITLE: &str = "Quit Rustcraft?";

const TAB_HINT: Hint = Hint {
    keys: "Tab",
    action: "Move",
};
const ENTER_HINT: Hint = Hint {
    keys: "Enter",
    action: "Select",
};
const ESC_HINT: Hint = Hint {
    keys: "Esc",
    action: "Quit",
};

/// The key hints of the main menu.
pub const MAIN_HINTS: [Hint; 3] = [TAB_HINT, ENTER_HINT, ESC_HINT];

/// The key hint of the Continue card, shown only with the card.
pub const CONTINUE_HINT: Hint = Hint {
    keys: "Ctrl+Enter",
    action: "Continue",
};

/// The size of the title image, shown at its own size: its pixels stay crisp.
const LOGO_SIZE: Vec2 = Vec2::new(601.0, 94.0);

/// The width of the panel of the Continue card.
const CARD_WIDTH: f32 = 300.0;
/// The side of the thumbnail of the Continue card.
const THUMBNAIL_SIZE: f32 = 48.0;

/// Spawns the main menu, the focus on Singleplayer, and the Continue card when a world was played.
pub fn home_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    paths: Res<GameFolderPaths>,
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
    // the buttons on the left, the card of the last world on the right
    let row = commands
        .spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            },
            ChildOf(content),
        ))
        .id();
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
    let last_world = last_played_world(&paths.game_folder_path.join(SAVE_PATH));
    let hints: &[Hint] = match &last_world {
        Some(world) => {
            commands.spawn((continue_card(world, &font), ChildOf(row)));
            &[TAB_HINT, ENTER_HINT, CONTINUE_HINT, ESC_HINT]
        }
        None => &MAIN_HINTS,
    };
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
    commands.spawn((hint_bar(hints, font, glyph_font), ChildOf(root)));
}

/// The Continue card of `world`: its thumbnail, name, day and when it was played, and Play.
fn continue_card(world: &WorldSummary, font: &Handle<Font>) -> impl Bundle {
    let text = |content: String, size: f32, color: Color| {
        (
            Text::new(content),
            TextFont {
                font: font.clone().into(),
                font_size: px(size).into(),
                ..default()
            },
            TextColor(color),
            // a long name without spaces breaks rather than leave the panel
            TextLayout::linebreak(LineBreak::WordOrCharacter),
        )
    };
    let elapsed = SystemTime::now()
        .duration_since(world.played)
        .unwrap_or_default();
    let mut details = vec![text(world.name.clone(), FONT_L, TEXT)];
    if let Some(day) = world.day {
        details.push(text(format!("Day {day}"), FONT_S, TEXT));
    }
    details.push(text(played_ago(elapsed), FONT_S, MUTED));
    let summary = (
        Node {
            column_gap: px(SPACE_M),
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            (
                Node {
                    width: px(THUMBNAIL_SIZE),
                    height: px(THUMBNAIL_SIZE),
                    flex_shrink: 0.0,
                    border: UiRect::all(px(BEVEL_WIDTH)),
                    ..default()
                },
                BackgroundColor(thumbnail_color(world.seed)),
                BorderColor {
                    top: BEVEL_LO,
                    left: BEVEL_LO,
                    bottom: BEVEL_HI,
                    right: BEVEL_HI,
                },
            ),
            (
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                Children::spawn(SpawnIter(details.into_iter())),
            ),
        ],
    );
    let panel = (
        Node {
            width: px(CARD_WIDTH),
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_L),
            padding: UiRect::all(px(SPACE_L)),
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(PANEL),
        BorderColor {
            top: BEVEL_HI,
            left: BEVEL_HI,
            bottom: BEVEL_LO,
            right: BEVEL_LO,
        },
        Children::spawn((
            Spawn(summary),
            Spawn((
                button("Play", ButtonVariant::Primary, font.clone()),
                observe(play_last_world),
            )),
        )),
    );
    (
        ContinueCard {
            world: world.name.clone(),
        },
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_S),
            ..default()
        },
        children![text("Continue".to_string(), FONT_S, MUTED), panel],
    )
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

/// The Play button of the Continue card plays the last world.
fn play_last_world(
    _: On<Activate>,
    cards: Query<&ContinueCard>,
    mut selected: ResMut<SelectedWorld>,
    mut game: ResMut<NextState<GameState>>,
    mut menu: ResMut<NextState<MenuState>>,
) {
    if let Ok(card) = cards.single() {
        start_world(&card.world, &mut selected, &mut game, &mut menu);
    }
}

/// Ctrl+Enter plays the last world, wherever the focus is on the main menu.
pub fn continue_on_ctrl_enter(
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<&ContinueCard>,
    mut selected: ResMut<SelectedWorld>,
    mut game: ResMut<NextState<GameState>>,
    mut menu: ResMut<NextState<MenuState>>,
) {
    if !(keys.just_pressed(KeyCode::Enter) && ctrl_held(&keys)) {
        return;
    }
    if let Ok(card) = cards.single() {
        start_world(&card.world, &mut selected, &mut game, &mut menu);
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
