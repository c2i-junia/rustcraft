//! The pause menu, over the game, which goes on beneath it: back to the game, the settings,
//! saving, and leaving. Escape opens it during gameplay and closes it.

use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    camera::visibility::VisibilitySystems,
    input_focus::{tab_navigation::TabGroup, AutoFocus, InputFocus, InputFocusVisible},
    prelude::*,
    ui::InteractionDisabled,
    ui_widgets::{observe, Activate},
};
use bevy_renet::RenetClient;

use crate::input::data::GameAction;
use crate::input::keyboard::is_action_just_pressed;
use crate::network::leave::{leaving, LeaveGame};
use crate::network::save::send_save_request_to_server;
use crate::network::session::{end_session, SessionOrigin};
use crate::network::LocalSaves;
use crate::state::{GameSet, GameState, KeyMap};
use crate::ui::assets::{load_font, FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::reticle::show_reticle;
use crate::ui::hud::{HudBottom, InventoryRoot, UIMode, UiDialog};
use crate::ui::menus::settings::controls::KeyCapture;
use crate::ui::menus::settings::{
    close_settings, settings_open, spawn_settings, SettingsContext, SettingsRoot,
};
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_S, FONT_XL, MUTED, PANEL, SPACE_L, SPACE_S, SPACE_XL,
    SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, set_label, ButtonVariant};
use crate::ui::widgets::dialog::dialog_open;
use crate::ui::widgets::focus::{keep_focus_inside, ArrowNav};
use crate::ui::widgets::hints::{hint_bar, Hint, HintBar, HINT_BAR_HEIGHT};
use crate::ui::widgets::text_field::text_field_focused;
use crate::ui::widgets::toast::{show_toast, ToastKind};

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

/// The label of "Save now", and of "Save and quit to title", while the world is being saved.
pub const SAVING: &str = "Saving...";

/// The toast once the world is saved.
pub const WORLD_SAVED: &str = "World saved";

/// The toast when the world was not saved in time.
pub const SAVE_FAILED: &str = "Could not save the world";

/// How long "Save now" waits for the world to be written before saying it could not be saved.
pub const SAVE_TIMEOUT: Duration = Duration::from_secs(10);

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

/// A save asked by "Save now": the counts of saves written and failed when it was asked, and
/// when.
#[derive(Resource)]
pub struct SaveNow {
    pub from: u64,
    pub failures: u64,
    pub since: Instant,
}

/// `Some(true)` once the counter passed `from`, `Some(false)` once a save `failed` since, or
/// after SAVE_TIMEOUT, else `None`.
pub fn save_outcome(from: u64, now: u64, failed: bool, elapsed: Duration) -> Option<bool> {
    if now > from {
        Some(true)
    } else if failed || elapsed >= SAVE_TIMEOUT {
        Some(false)
    } else {
        None
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
        toggle_pause_menu
            .after(GameSet::UiMode)
            .run_if(in_state(GameState::Game))
            .run_if(not(dialog_open))
            .run_if(not(text_field_focused))
            .run_if(not(leaving)),
    )
    // after the menu opens: a menu opened while saving never shows "Save now" enabled; never
    // while leaving, whose buttons stay disabled
    .add_systems(
        Update,
        watch_save_now
            .after(toggle_pause_menu)
            .run_if(in_state(GameState::Game))
            .run_if(resource_exists::<SaveNow>)
            .run_if(not(leaving)),
    )
    .add_systems(Update, show_leaving.run_if(leaving))
    .add_systems(OnExit(GameState::Game), |mut commands: Commands| {
        commands.remove_resource::<SaveNow>();
    })
    // the HUD hides in the frame the menus first show, and shows again in the frame they are
    // gone
    .add_systems(
        PostUpdate,
        (
            close_pause_menu,
            (hide_hud_under_menus, show_reticle)
                .after(close_pause_menu)
                .after(close_settings)
                .before(VisibilitySystems::VisibilityPropagate)
                .run_if(in_state(GameState::Game)),
        ),
    );
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

/// Spawns the pause menu of the session started from `origin`, the focus on Back to game.
fn spawn_pause_menu(commands: &mut Commands, assets: &AssetServer, origin: Option<&SessionOrigin>) {
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
    keep_focus_inside(commands, root, first);
}

/// Carries out the button of the pause menu that was activated.
#[allow(clippy::too_many_arguments)]
fn act_on_pause_button(
    event: On<Activate>,
    buttons: Query<&PauseButton>,
    menus: Query<Entity, With<PauseMenu>>,
    settings: Query<(), With<SettingsRoot>>,
    origin: Option<Res<SessionOrigin>>,
    saves: Option<Res<LocalSaves>>,
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
        PauseButton::SaveNow => {
            send_save_request_to_server(&mut client);
            // the embedded server counts its saves: the button waits for this one
            if let Some(saves) = saves {
                commands.insert_resource(SaveNow {
                    from: saves.0.get(),
                    failures: saves.0.failures(),
                    since: Instant::now(),
                });
                commands
                    .entity(event.entity)
                    .queue(set_label(SAVING))
                    .insert(InteractionDisabled);
            }
        }
        PauseButton::SaveAndQuit => commands.trigger(LeaveGame),
        PauseButton::Disconnect => {
            if let Some(origin) = origin {
                end_session(&mut commands, &mut game, &origin, None);
            }
        }
    }
}

/// Says in a toast whether the save asked by "Save now" was written, and gives the button back.
/// A menu opened again while the world is being saved shows the button waiting.
fn watch_save_now(
    save: Res<SaveNow>,
    saves: Res<LocalSaves>,
    buttons: Query<(Entity, &PauseButton, Has<InteractionDisabled>)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let failed = saves.0.failures() > save.failures;
    let outcome = save_outcome(save.from, saves.0.get(), failed, save.since.elapsed());
    for (entity, &action, disabled) in &buttons {
        if action != PauseButton::SaveNow {
            continue;
        }
        let mut button = commands.entity(entity);
        match outcome {
            None if !disabled => {
                button.queue(set_label(SAVING)).insert(InteractionDisabled);
            }
            None => {}
            Some(_) => {
                button
                    .queue(set_label(action.label()))
                    .remove::<InteractionDisabled>();
            }
        }
    }
    let Some(saved) = outcome else {
        return;
    };
    let (text, kind) = if saved {
        (WORLD_SAVED, ToastKind::Info)
    } else {
        (SAVE_FAILED, ToastKind::Error)
    };
    show_toast(&mut commands, text, kind, load_font(&assets));
    commands.remove_resource::<SaveNow>();
}

/// While the game is left for the title, the pause menu waits: every button is disabled, the
/// leaving one reads "Saving...", and its hints, which no key follows any more, are hidden.
fn show_leaving(
    buttons: Query<(Entity, &PauseButton), Without<InteractionDisabled>>,
    menus: Query<&Children, With<PauseMenu>>,
    mut hints: Query<&mut Visibility, With<HintBar>>,
    mut commands: Commands,
) {
    for (entity, &action) in &buttons {
        let mut button = commands.entity(entity);
        button.insert(InteractionDisabled);
        if action == PauseButton::SaveAndQuit {
            button.queue(set_label(SAVING));
        }
    }
    for children in &menus {
        let mut bars = hints.iter_many_mut(children);
        while let Some(mut visibility) = bars.fetch_next() {
            visibility.set_if_neq(Visibility::Hidden);
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
    use crate::network::leave::Leaving;
    use bevy::state::app::StatesPlugin;
    use shared::SaveCounter;
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
    fn a_save_is_told_once_written_or_after_ten_seconds() {
        let s = Duration::from_secs;
        assert_eq!(save_outcome(5, 5, false, s(1)), None);
        assert_eq!(save_outcome(5, 6, false, s(1)), Some(true));
        assert_eq!(save_outcome(5, 5, false, s(10)), Some(false));
        assert_eq!(save_outcome(5, 6, false, s(11)), Some(true));
    }

    #[test]
    fn a_failed_save_is_told_at_once_unless_another_save_was_written() {
        let s = Duration::from_secs;
        assert_eq!(save_outcome(5, 5, true, s(1)), Some(false));
        assert_eq!(save_outcome(5, 6, true, s(1)), Some(true));
    }

    /// An app in the game with the systems of the pause menu, and a save asked by "Save now" that
    /// is not written yet.
    fn saving_app(saves: &SaveCounter) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default(), StatesPlugin))
            .init_asset::<Font>()
            .insert_state(GameState::Game)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<InputFocus>()
            .init_resource::<InputFocusVisible>()
            .init_resource::<KeyCapture>()
            .insert_resource(KeyMap {
                map: BTreeMap::from([(GameAction::Escape, vec![KeyCode::Escape])]),
            })
            .insert_resource(UIMode::Closed)
            .insert_resource(LocalSaves(saves.clone()))
            .insert_resource(SaveNow {
                from: saves.get(),
                failures: saves.failures(),
                since: Instant::now(),
            })
            .add_plugins(pause_plugin);
        app
    }

    /// Opens the pause menu with Escape, then lets go of the key.
    fn open_pause_menu(app: &mut App) {
        press_escape(app);
        app.update();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
    }

    /// The label of "Save now", and whether it is disabled.
    fn save_now(app: &mut App) -> (String, bool) {
        let world = app.world_mut();
        let (label, disabled, children) = world
            .query::<(
                &PauseButton,
                &AccessibleLabel,
                Has<InteractionDisabled>,
                &Children,
            )>()
            .iter(world)
            .find(|(action, ..)| **action == PauseButton::SaveNow)
            .map(|(_, label, disabled, children)| (label.0.clone(), disabled, children[0]))
            .unwrap();
        assert_eq!(world.get::<Text>(children).unwrap().0, label);
        (label, disabled)
    }

    #[test]
    fn a_menu_opened_while_saving_shows_save_now_waiting_from_its_first_frame() {
        let mut app = saving_app(&SaveCounter::default());
        open_pause_menu(&mut app);
        assert_eq!(save_now(&mut app), (SAVING.to_string(), true));
    }

    #[test]
    fn a_failed_save_says_so_at_once_and_gives_save_now_back() {
        let saves = SaveCounter::default();
        let mut app = saving_app(&saves);
        open_pause_menu(&mut app);

        saves.bump_failure();
        app.update();

        assert_eq!(save_now(&mut app), ("Save now".to_string(), false));
        assert!(!app.world().contains_resource::<SaveNow>());
        let world = app.world_mut();
        let alerts: Vec<Option<String>> = world
            .query::<&AccessibilityNode>()
            .iter(world)
            .filter(|node| node.role() == Role::Alert)
            .map(|node| node.label().map(str::to_string))
            .collect();
        assert_eq!(alerts, [Some(SAVE_FAILED.to_string())]);
    }

    /// The labels of the toasts of `role` shown.
    fn toasts(app: &mut App, role: Role) -> Vec<Option<String>> {
        let world = app.world_mut();
        world
            .query::<&AccessibilityNode>()
            .iter(world)
            .filter(|node| node.role() == role)
            .map(|node| node.label().map(str::to_string))
            .collect()
    }

    #[test]
    fn while_leaving_the_pause_menu_waits_and_escape_does_nothing() {
        let mut app = saving_app(&SaveCounter::default());
        app.world_mut().remove_resource::<SaveNow>();
        open_pause_menu(&mut app);
        app.world_mut().insert_resource(Leaving {
            since: Instant::now(),
        });
        app.update();

        let world = app.world_mut();
        let buttons: Vec<(PauseButton, bool)> = world
            .query::<(&PauseButton, Has<InteractionDisabled>)>()
            .iter(world)
            .map(|(&action, disabled)| (action, disabled))
            .collect();
        assert_eq!(buttons.len(), 4);
        assert!(buttons.iter().all(|&(_, disabled)| disabled), "{buttons:?}");
        let leave = world
            .query::<(&PauseButton, &AccessibleLabel)>()
            .iter(world)
            .find(|(action, _)| **action == PauseButton::SaveAndQuit)
            .map(|(_, label)| label.0.clone());
        assert_eq!(leave.as_deref(), Some(SAVING));
        let hints = world
            .query_filtered::<&Visibility, With<HintBar>>()
            .single(world)
            .unwrap();
        assert_eq!(*hints, Visibility::Hidden);

        press_escape(&mut app);
        app.update();
        app.update();
        assert_eq!(menus(&mut app), 1);
    }

    #[test]
    fn leaving_never_gives_save_now_back() {
        let saves = SaveCounter::default();
        let mut app = saving_app(&saves);
        open_pause_menu(&mut app);
        app.world_mut().insert_resource(Leaving {
            since: Instant::now(),
        });

        saves.bump();
        app.update();

        assert_eq!(save_now(&mut app), (SAVING.to_string(), true));
        assert_eq!(toasts(&mut app, Role::Status), []);
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
