//! The settings screen: one panel with a tab per kind of setting, Controls, Audio and Interface.
//! Every change applies and is saved at once.

pub mod audio;
pub mod controls;
pub mod interface;

use accesskit::{NodeId, Role};
use bevy::{
    a11y::AccessibilityNode,
    input_focus::{tab_navigation::TabGroup, AutoFocus},
    prelude::*,
    ui_widgets::{observe, Activate},
};
use shared::GameFolderPaths;

use super::background::menu_background;
use super::MenuState;
use crate::audio::settings::AudioSettings;
use crate::input::keyboard::save_keybindings;
use crate::state::KeyMap;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::settings::InterfaceSettings;
use crate::ui::theme::{
    BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_XL, PANEL, SPACE_L, SPACE_M, SPACE_XL, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::dialog_open;
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::tabs::{spawn_tabs, TabButton, TabChange, Tabs};
use crate::ui::widgets::text_field::text_field_focused;
use audio::spawn_audio_tab;
use controls::{controls_update_system, spawn_controls_tab, ActionRecorder};
use interface::{show_scale_limit, spawn_interface_tab};

/// Where the settings are open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsContext {
    /// From the main menu, as the screen `MenuState::Settings`.
    Menu,
    /// From the pause menu, over the game.
    // The pause menu (Lot 1, Task 9) is the first to open the settings in game
    #[allow(dead_code)]
    Game,
}

/// The root of the settings.
#[derive(Component)]
pub struct SettingsRoot {
    pub context: SettingsContext,
}

/// The tabs of the settings, in the order of the tab bar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SettingsTab {
    #[default]
    Controls,
    Audio,
    Interface,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 3] = [
        SettingsTab::Controls,
        SettingsTab::Audio,
        SettingsTab::Interface,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::Controls => "Controls",
            SettingsTab::Audio => "Audio",
            SettingsTab::Interface => "Interface",
        }
    }
}

/// The content of the current tab, spawned again when the player chooses another tab.
#[derive(Component)]
pub struct SettingsPanel;

/// A request to close the settings, on their root, applied in `PostUpdate`: the key or the click
/// that asked for it must not also act on the screen beneath.
#[derive(Component)]
struct CloseSettings;

/// The key hints of the settings.
pub const SETTINGS_HINTS: [Hint; 4] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "←→",
        action: "Change",
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

/// The width of the panel of the settings.
const PANEL_WIDTH: f32 = 720.0;
/// The height of the panel: the same for every tab, so that the tabs do not move.
const PANEL_HEIGHT: f32 = 560.0;

pub fn settings_plugin(app: &mut App) {
    app.add_systems(OnEnter(MenuState::Settings), open_menu_settings)
        // the hosted controls list saves the keys it binds when the settings close
        .add_systems(OnExit(MenuState::Settings), save_keybindings)
        .add_systems(
            Update,
            (
                // before the recorder takes the key and hides itself
                settings_escape
                    .run_if(settings_open)
                    .run_if(not(dialog_open))
                    .run_if(not(text_field_focused))
                    .before(controls_update_system),
                controls_update_system.run_if(any_with_component::<ActionRecorder>),
                show_scale_limit.run_if(any_with_component::<interface::ScaleLimitNote>),
            ),
        )
        .add_systems(PostUpdate, close_settings);
}

/// Whether the settings are open.
pub fn settings_open(roots: Query<(), With<SettingsRoot>>) -> bool {
    !roots.is_empty()
}

fn open_menu_settings(mut commands: Commands, assets: Res<AssetServer>) {
    spawn_settings(&mut commands, &assets, SettingsContext::Menu);
}

/// Spawns the settings, open on the Controls tab, and returns their root.
pub fn spawn_settings(
    commands: &mut Commands,
    assets: &AssetServer,
    context: SettingsContext,
) -> Entity {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = match context {
        SettingsContext::Menu => commands
            .spawn((
                menu_background(assets),
                DespawnOnExit(MenuState::Settings),
                TabGroup::new(0),
            ))
            .id(),
        SettingsContext::Game => commands
            .spawn(Node {
                width: percent(100),
                height: percent(100),
                ..default()
            })
            .id(),
    };
    commands.entity(root).insert(SettingsRoot { context });
    // the panel in the middle of the space the key hints leave
    let content = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100),
                height: percent(100),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                padding: UiRect {
                    top: px(SPACE_XL),
                    bottom: px(HINT_BAR_HEIGHT + SPACE_XL),
                    left: px(SPACE_L),
                    right: px(SPACE_L),
                },
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    let panel = commands
        .spawn((
            Node {
                width: px(PANEL_WIDTH),
                max_width: percent(100),
                height: px(PANEL_HEIGHT),
                max_height: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_M),
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
            ChildOf(content),
        ))
        .id();
    commands.spawn((
        Text::new("Settings"),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_XL).into(),
            ..default()
        },
        TextColor(TEXT),
        ChildOf(panel),
    ));
    let labels = SettingsTab::ALL.map(SettingsTab::label);
    let tabs = Tabs {
        label: "Settings",
        tabs: &labels,
        selected: 0,
    };
    let bar = spawn_tabs(commands, &tabs, font.clone());
    commands
        .entity(bar)
        .insert((AutoFocus, ChildOf(panel)))
        .observe(show_tab);
    commands.spawn((
        SettingsPanel,
        AccessibilityNode(accesskit::Node::new(Role::TabPanel)),
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_height: px(0),
            padding: UiRect::top(px(SPACE_L)),
            row_gap: px(SPACE_XL),
            ..default()
        },
        ChildOf(panel),
    ));
    commands.spawn((
        Node::default(),
        ChildOf(panel),
        children![(
            button("Back", ButtonVariant::Default, font.clone()),
            observe(move |_: On<Activate>, mut commands: Commands| {
                commands.entity(root).insert(CloseSettings);
            }),
        )],
    ));
    commands.spawn((
        hint_bar(&SETTINGS_HINTS, font, assets.load(MONO_FONT_PATH)),
        ChildOf(root),
    ));
    // the first tab shows as if the player had chosen it
    commands.trigger(TabChange {
        entity: bar,
        index: 0,
    });
    root
}

/// Replaces the content of the settings panel by the content of the chosen tab, which labels it.
#[allow(clippy::too_many_arguments)]
fn show_tab(
    change: On<TabChange>,
    panel: Single<(Entity, &mut AccessibilityNode), With<SettingsPanel>>,
    tabs: Query<(Entity, &TabButton, &ChildOf)>,
    key_map: Res<KeyMap>,
    audio: Res<AudioSettings>,
    interface: Res<InterfaceSettings>,
    paths: Res<GameFolderPaths>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let tab = SettingsTab::ALL[change.index];
    let (panel, mut node) = panel.into_inner();
    node.set_label(tab.label());
    if let Some((button, ..)) = tabs.iter().find(|(_, button, child_of)| {
        button.0 == change.index && child_of.parent() == change.entity
    }) {
        node.set_labelled_by(vec![NodeId(button.to_bits())]);
    }
    commands.entity(panel).despawn_related::<Children>();
    let font: Handle<Font> = assets.load(FONT_PATH);
    match tab {
        SettingsTab::Controls => {
            spawn_controls_tab(&mut commands, panel, &assets, &key_map, &paths)
        }
        SettingsTab::Audio => spawn_audio_tab(&mut commands, panel, &audio, font),
        SettingsTab::Interface => spawn_interface_tab(&mut commands, panel, &interface, font),
    }
}

/// Escape closes the settings, unless the hosted controls list is waiting for a key to bind.
fn settings_escape(
    keys: Res<ButtonInput<KeyCode>>,
    recorders: Query<&Visibility, With<ActionRecorder>>,
    roots: Query<Entity, With<SettingsRoot>>,
    mut commands: Commands,
) {
    if !keys.just_pressed(KeyCode::Escape) {
        return;
    }
    if recorders
        .iter()
        .any(|visibility| *visibility == Visibility::Visible)
    {
        return;
    }
    for root in &roots {
        commands.entity(root).insert(CloseSettings);
    }
}

/// Closes the settings asked to close: from the main menu, back to it.
fn close_settings(
    roots: Query<(Entity, &SettingsRoot), With<CloseSettings>>,
    mut menu_state: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    for (root, settings) in &roots {
        commands.entity(root).remove::<CloseSettings>();
        match settings.context {
            SettingsContext::Menu => menu_state.set(MenuState::Main),
            SettingsContext::Game => commands.entity(root).despawn(),
        }
    }
}
