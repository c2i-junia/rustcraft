//! The pause menu, over the game, which goes on beneath it: back to the game, the settings,
//! saving, and leaving. Escape opens it during gameplay and closes it.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{
        tab_navigation::TabGroup, AcquireFocus, AutoFocus, FocusCause, InputFocus,
        InputFocusVisible,
    },
    prelude::*,
    ui_widgets::{observe, Activate},
};
use bevy_renet::RenetClient;

use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::network::save::send_save_request_to_server;
use crate::network::session::{end_session, SessionOrigin};
use crate::state::{GameSet, GameState, KeyMap};
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::{HudBottom, InventoryRoot, UIMode, UiDialog};
use crate::ui::menus::settings::controls::KeyCapture;
use crate::ui::menus::settings::{settings_open, spawn_settings, SettingsContext, SettingsRoot};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_S, FONT_XL, MUTED, PANEL, SPACE_L, SPACE_S, SPACE_XL,
    SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::dialog_open;
use crate::ui::widgets::focus::ArrowNav;
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::text_field::text_field_focused;

/// The title of the pause menu. Not "Paused": the server keeps ticking beneath it.
pub const PAUSE_TITLE: &str = "Game menu";

/// The key hints of the pause menu.
pub const PAUSE_HINTS: [Hint; 3] = [
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
        action: "Back",
    },
];

/// The width of the panel of the pause menu.
const PANEL_WIDTH: f32 = 320.0;

/// The root of the pause menu, spawned when it opens and despawned when it closes.
#[derive(Component)]
pub struct PauseMenu;

/// A request to close the pause menu, on its root, applied in `PostUpdate`: the key or the click
/// that asked for it must not also act on the game.
#[derive(Component)]
struct ClosePause;

/// The buttons of the pause menu.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PauseButton {
    BackToGame,
    Settings,
    SaveNow,
    SaveAndQuit,
    Disconnect,
}

impl PauseButton {
    pub fn label(self) -> &'static str {
        match self {
            PauseButton::BackToGame => "Back to game",
            PauseButton::Settings => "Settings",
            PauseButton::SaveNow => "Save now",
            PauseButton::SaveAndQuit => "Save and quit to title",
            PauseButton::Disconnect => "Disconnect",
        }
    }

    /// Whether the button leaves the game: it sits apart from the others.
    fn leaves(self) -> bool {
        matches!(self, PauseButton::SaveAndQuit | PauseButton::Disconnect)
    }
}

/// The buttons of the pause menu, top to bottom: a world's, or a server's (Decision 2). A
/// server is not the player's to save, and ignores a client's save request.
pub fn pause_buttons(origin: Option<&SessionOrigin>) -> &'static [PauseButton] {
    match origin {
        Some(SessionOrigin::Server { .. }) => &[
            PauseButton::BackToGame,
            PauseButton::Settings,
            PauseButton::Disconnect,
        ],
        Some(SessionOrigin::World { .. }) | None => &[
            PauseButton::BackToGame,
            PauseButton::Settings,
            PauseButton::SaveNow,
            PauseButton::SaveAndQuit,
        ],
    }
}

pub fn pause_plugin(app: &mut App) {
    app.add_systems(
        Update,
        (
            toggle_pause_menu
                .after(GameSet::UiMode)
                .run_if(not(dialog_open))
                .run_if(not(text_field_focused)),
            hide_hud_under_menus,
        )
            .run_if(in_state(GameState::Game)),
    )
    .add_systems(PostUpdate, close_pause_menu);
}

/// Whether the pause menu is open.
pub fn pause_open(menus: Query<(), With<PauseMenu>>) -> bool {
    !menus.is_empty()
}

/// Escape opens the pause menu during gameplay, closes the inventory, or asks the pause menu to
/// close. The settings over the pause menu, and a key slot waiting for a key, take Escape for
/// themselves.
#[allow(clippy::too_many_arguments)]
fn toggle_pause_menu(
    keys: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    capture: Res<KeyCapture>,
    menus: Query<Entity, With<PauseMenu>>,
    settings: Query<(), With<SettingsRoot>>,
    mut inventory: Query<&mut Visibility, With<InventoryRoot>>,
    origin: Option<Res<SessionOrigin>>,
    assets: Res<AssetServer>,
    mut focus_visible: ResMut<InputFocusVisible>,
    mut commands: Commands,
) {
    if !is_action_just_pressed(GameAction::Escape, &keys, &key_map) {
        return;
    }
    if !settings.is_empty() || capture.is_active() {
        return;
    }
    if let Ok(menu) = menus.single() {
        commands.entity(menu).insert(ClosePause);
    } else if *ui_mode == UIMode::Closed {
        spawn_pause_menu(&mut commands, &assets, origin.as_deref());
        // opened from the keyboard: the ring shows where Enter goes
        focus_visible.0 = true;
    } else if let Ok(mut inventory) = inventory.single_mut() {
        // the chat closes itself
        *inventory = Visibility::Hidden;
    }
}

/// Spawns the pause menu of the session started from `origin`, the focus on Back to game, and
/// returns its root.
fn spawn_pause_menu(
    commands: &mut Commands,
    assets: &AssetServer,
    origin: Option<&SessionOrigin>,
) -> Entity {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            PauseMenu,
            UiDialog,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::Dialog)),
            AccessibleLabel::new(PAUSE_TITLE),
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(px(HINT_BAR_HEIGHT)),
                ..default()
            },
            BackgroundColor(BEVEL_LO.with_alpha(0.6)),
            Visibility::Visible,
            GlobalZIndex(5),
            DespawnOnExit(GameState::Game),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_L),
                padding: UiRect::all(px(SPACE_XL)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                width: px(PANEL_WIDTH),
                max_width: percent(90),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
            ChildOf(root),
        ))
        .id();
    let header = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    commands.spawn((
        Text::new(PAUSE_TITLE),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_XL).into(),
            ..default()
        },
        TextColor(TEXT),
        ChildOf(header),
    ));
    if let Some(origin) = origin {
        commands.spawn((
            Text::new(origin.name()),
            TextFont {
                font: font.clone().into(),
                font_size: px(FONT_S).into(),
                ..default()
            },
            TextColor(MUTED),
            // a long name without spaces breaks rather than leave the panel
            TextLayout::linebreak(LineBreak::WordOrCharacter),
            ChildOf(header),
        ));
    }
    let column = commands
        .spawn((
            ArrowNav,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let mut first = None;
    for &action in pause_buttons(origin) {
        let mut entity = commands.spawn((
            button(action.label(), ButtonVariant::Default, font.clone()),
            action,
            observe(act_on_pause_button),
            ChildOf(column),
        ));
        if first.is_none() {
            entity.insert(AutoFocus);
            first = Some(entity.id());
        }
        if action.leaves() {
            entity
                .entry::<Node>()
                .and_modify(|mut node| node.margin = UiRect::top(px(SPACE_L)));
        }
    }
    commands.spawn((
        hint_bar(&PAUSE_HINTS, font, assets.load(MONO_FONT_PATH)),
        ChildOf(root),
    ));
    // a click that no button took (a button stops the event) keeps the focus in the menu
    commands.entity(root).observe(
        move |mut event: On<AcquireFocus>,
              parents: Query<&ChildOf>,
              mut focus: ResMut<InputFocus>| {
            event.propagate(false);
            let inside =
                std::iter::successors(focus.get(), |e| parents.get(*e).ok().map(ChildOf::parent))
                    .any(|e| e == root);
            if let (false, Some(first)) = (inside, first) {
                focus.set(first, FocusCause::Pressed);
            }
        },
    );
    root
}

/// Carries out the button of the pause menu that was activated.
#[allow(clippy::too_many_arguments)]
fn act_on_pause_button(
    event: On<Activate>,
    buttons: Query<&PauseButton>,
    menus: Query<Entity, With<PauseMenu>>,
    settings: Query<(), With<SettingsRoot>>,
    origin: Option<Res<SessionOrigin>>,
    mut client: ResMut<RenetClient>,
    mut game: ResMut<NextState<GameState>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(action) = buttons.get(event.entity) else {
        return;
    };
    match action {
        PauseButton::BackToGame => {
            for menu in &menus {
                commands.entity(menu).insert(ClosePause);
            }
        }
        PauseButton::Settings => {
            if settings.is_empty() {
                let opener = Some(event.entity);
                spawn_settings(&mut commands, &assets, SettingsContext::Game, opener);
            }
        }
        PauseButton::SaveNow => send_save_request_to_server(&mut client),
        PauseButton::SaveAndQuit => game.set(GameState::Menu),
        PauseButton::Disconnect => {
            if let Some(origin) = origin {
                end_session(&mut commands, &mut game, &origin, None);
            }
        }
    }
}

/// Closes the pause menu asked to close; nothing has the focus in the game.
fn close_pause_menu(
    menus: Query<Entity, (With<PauseMenu>, With<ClosePause>)>,
    mut focus: ResMut<InputFocus>,
    mut commands: Commands,
) {
    for menu in &menus {
        commands.entity(menu).despawn();
        focus.clear();
    }
}

/// Hides the hotbar and its item name under the pause menu and the settings: nothing there
/// concerns them, and the hint bar would cover them in part.
fn hide_hud_under_menus(
    menus: Query<(), With<PauseMenu>>,
    settings: Query<(), With<SettingsRoot>>,
    mut hud: Query<&mut Visibility, With<HudBottom>>,
) {
    let shown = if pause_open(menus) || settings_open(settings) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut hud {
        visibility.set_if_neq(shown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::state::app::StatesPlugin;
    use std::collections::BTreeMap;

    fn app(ui_mode: UIMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<Font>()
            .init_state::<GameState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<InputFocus>()
            .init_resource::<InputFocusVisible>()
            .init_resource::<KeyCapture>()
            .insert_resource(KeyMap {
                map: BTreeMap::from([(GameAction::Escape, vec![KeyCode::Escape])]),
            })
            .insert_resource(ui_mode)
            .add_systems(Update, toggle_pause_menu)
            .add_systems(PostUpdate, close_pause_menu);
        app.world_mut().spawn((InventoryRoot, Visibility::Visible));
        app
    }

    /// Presses Escape anew: the input is not cleared between the updates of a bare app.
    fn press_escape(app: &mut App) {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.reset_all();
        keys.press(KeyCode::Escape);
    }

    fn menus(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<(), With<PauseMenu>>()
            .iter(app.world())
            .count()
    }

    fn inventory(app: &mut App) -> Visibility {
        *app.world_mut()
            .query_filtered::<&Visibility, With<InventoryRoot>>()
            .single(app.world())
            .unwrap()
    }

    #[test]
    fn a_world_and_a_server_offer_their_own_buttons() {
        use PauseButton::*;
        let world = SessionOrigin::World {
            name: "Survie".to_string(),
        };
        let server = SessionOrigin::Server {
            name: "Local".to_string(),
            ip: "127.0.0.1".to_string(),
            address: "127.0.0.1:8000".parse().unwrap(),
        };
        let solo = [BackToGame, Settings, SaveNow, SaveAndQuit];
        assert_eq!(pause_buttons(Some(&world)), solo);
        assert_eq!(pause_buttons(None), solo);
        assert_eq!(
            pause_buttons(Some(&server)),
            [BackToGame, Settings, Disconnect]
        );
        let labels =
            [BackToGame, Settings, SaveNow, SaveAndQuit, Disconnect].map(PauseButton::label);
        assert_eq!(
            labels,
            [
                "Back to game",
                "Settings",
                "Save now",
                "Save and quit to title",
                "Disconnect"
            ]
        );
    }

    #[test]
    fn escape_opens_the_pause_menu_during_gameplay() {
        let mut app = app(UIMode::Closed);
        press_escape(&mut app);
        app.update();
        assert_eq!(menus(&mut app), 1);
    }

    #[test]
    fn escape_closes_the_inventory_instead_of_opening_the_pause_menu() {
        let mut app = app(UIMode::Opened);
        press_escape(&mut app);
        app.update();
        assert_eq!(menus(&mut app), 0);
        assert_eq!(inventory(&mut app), Visibility::Hidden);
    }

    #[test]
    fn escape_while_typing_does_not_open_the_pause_menu() {
        let mut app = app(UIMode::Typing);
        press_escape(&mut app);
        app.update();
        assert_eq!(menus(&mut app), 0);
    }

    #[test]
    fn escape_with_the_settings_open_leaves_the_pause_menu_open() {
        let mut app = app(UIMode::Opened);
        app.world_mut().spawn(PauseMenu);
        app.world_mut().spawn(SettingsRoot {
            context: SettingsContext::Game,
            restore_focus: None,
        });
        press_escape(&mut app);
        app.update();
        assert_eq!(menus(&mut app), 1);
    }

    #[test]
    fn escape_closes_the_pause_menu_only_after_the_frame() {
        let mut app = app(UIMode::Closed);
        press_escape(&mut app);
        app.update();
        assert_eq!(menus(&mut app), 1);

        *app.world_mut().resource_mut::<UIMode>() = UIMode::Opened;
        press_escape(&mut app);
        app.world_mut().run_schedule(Update);
        // the systems of `Update` still see it open in the frame of the key
        assert_eq!(menus(&mut app), 1);
        app.world_mut().run_schedule(PostUpdate);
        assert_eq!(menus(&mut app), 0);
    }
}
