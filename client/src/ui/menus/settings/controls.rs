//! The Controls tab of the settings: one line per action with the keys bound to it, a button to
//! clear them, and a dialog that binds the next key pressed.

use bevy::{color::palettes::css, prelude::*, ui::FocusPolicy};
use shared::GameFolderPaths;

use crate::input::data::GameAction;
use crate::menus::ScrollingList;
use crate::state::KeyMap;

use crate::ui::assets::*;
use crate::ui::style::{text_font, NORMAL_BUTTON};

/// Width of the last column of a line, which holds its "clear" button: wide enough to show
/// the button while the line is hovered, and too narrow for it otherwise.
const CLEAR_COLUMN_SHOWN: f32 = 40.;
const CLEAR_COLUMN_HIDDEN: f32 = 10.;

/// Clears the keys of `action`, which are shown in the entity `binds`.
#[derive(Debug, Component, PartialEq, Eq)]
pub struct ClearButton {
    action: GameAction,
    binds: Entity,
}

/// The line of `action`: clicking it asks for a key to add to those shown in `binds`.
#[derive(Component, Debug, PartialEq, Eq)]
pub struct EditControlButton {
    action: GameAction,
    binds: Entity,
}

/// The "Press any key" dialog. While it is visible, the next key pressed is bound to `action`
/// and added to the keys shown in `binds`.
#[derive(Component)]
pub struct ActionRecorder {
    pub action: GameAction,
    pub binds: Entity,
}

/// Spawns the list of the actions and the "Press any key..." dialog under `panel`.
pub fn spawn_controls_tab(
    commands: &mut Commands,
    panel: Entity,
    assets: &AssetServer,
    key_map: &KeyMap,
    paths: &GameFolderPaths,
) {
    let font = assets.load(FONT_PATH);
    let base_path = paths.assets_folder_path.display();
    let trash_icon: Handle<Image> = assets.load(format!("{base_path}/graphics/trash.png"));

    let list = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                width: Val::Percent(100.),
                ..default()
            },
            ScrollingList { position: 0. },
        ))
        .id();
    for (action, keys) in &key_map.map {
        let line = spawn_control_line(commands, assets, *action, keys, &trash_icon);
        commands.entity(list).add_child(line);
    }

    commands
        .spawn((
            Node {
                overflow: Overflow::clip_y(),
                flex_grow: 1.,
                min_height: Val::Px(0.),
                width: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(panel),
        ))
        .add_child(list);

    let recorder = spawn_action_recorder(commands, font);
    commands.entity(recorder).insert(ChildOf(panel));
}

/// One line of the list: the name of `action`, its keys, and the button that clears them.
fn spawn_control_line(
    commands: &mut Commands,
    assets: &AssetServer,
    action: GameAction,
    keys: &[KeyCode],
    trash_icon: &Handle<Image>,
) -> Entity {
    let name = commands
        .spawn((
            Text::new(format!("{action:?}")),
            text_font(assets.load(FONT_PATH), 24.0),
            TextColor(Color::WHITE),
            Node {
                margin: UiRect::all(Val::Px(10.)),
                ..default()
            },
        ))
        .id();

    let binds = commands
        .spawn(Node {
            flex_direction: FlexDirection::RowReverse,
            column_gap: Val::Px(15.),
            margin: UiRect::horizontal(Val::Px(10.)),
            ..default()
        })
        .id();
    update_input_component(commands, binds, keys, assets);

    let clear_button = commands
        .spawn((
            Button,
            // The click also reaches the line below the button
            FocusPolicy::Pass,
            Node {
                border_radius: BorderRadius::all(Val::Percent(25.)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                width: Val::Percent(80.),
                padding: UiRect::all(Val::Px(5.)),
                ..default()
            },
            ClearButton { action, binds },
            Name::new(format!("Clear {action:?}")),
        ))
        .with_child((
            ImageNode::new(trash_icon.clone()),
            Node {
                width: Val::Percent(100.),
                ..default()
            },
        ))
        .id();

    commands
        .spawn((
            Button,
            BorderColor::all(Color::srgb(0.3, 0.3, 0.3)),
            Node {
                display: Display::Grid,
                width: Val::Percent(100.),
                height: Val::Auto,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                grid_template_columns: vec![
                    RepeatedGridTrack::flex(2, 1.),
                    RepeatedGridTrack::px(1, CLEAR_COLUMN_SHOWN),
                ],
                border: UiRect::bottom(Val::Px(2.5)),
                ..default()
            },
            EditControlButton { action, binds },
        ))
        .add_children(&[name, binds, clear_button])
        .id()
}

/// The hidden "Press any key..." dialog, which covers the panel while it records a key.
fn spawn_action_recorder(commands: &mut Commands, font: Handle<Font>) -> Entity {
    let dialog = commands
        .spawn((
            BackgroundColor(Color::srgb(0.2, 0.2, 0.2)),
            BorderColor::all(Color::Srgba(css::BLUE_VIOLET)),
            Node {
                border: UiRect::all(Val::Px(2.5)),
                min_width: Val::Percent(50.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(Val::Px(10.)),
                ..default()
            },
        ))
        .with_child((
            Text::new("Press any key..."),
            text_font(font, 21.0),
            TextColor(Color::WHITE),
            Node {
                margin: UiRect::all(Val::Px(25.)),
                width: Val::Auto,
                ..default()
            },
        ))
        .id();

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            FocusPolicy::Block,
            GlobalZIndex(2),
            // Both fields are set by the line that opens the dialog
            ActionRecorder {
                action: GameAction::Escape,
                binds: Entity::PLACEHOLDER,
            },
        ))
        .add_child(dialog)
        .id()
}

/// How a key is written on the screen: `KeyW` is `W`, the arrow keys are arrows.
fn key_label(key: KeyCode) -> String {
    match key {
        KeyCode::ArrowLeft => "←".into(),
        KeyCode::ArrowRight => "→".into(),
        KeyCode::ArrowUp => "↑".into(),
        KeyCode::ArrowDown => "↓".into(),
        _ => format!("{key:?}").replace("Key", ""),
    }
}

/// Replaces what `entity` shows by one label per key of `binds`.
pub fn update_input_component(
    commands: &mut Commands,
    entity: Entity,
    binds: &[KeyCode],
    assets: &AssetServer,
) {
    commands.entity(entity).despawn_related::<Children>();
    let font: Handle<Font> = assets.load(FONT_PATH);

    for key in binds {
        let label = commands
            .spawn((
                BackgroundColor(Color::Srgba(css::BLUE_VIOLET)),
                Node {
                    border_radius: BorderRadius::all(Val::Px(10.)),
                    padding: UiRect::horizontal(Val::Px(10.)),
                    ..default()
                },
            ))
            .with_child((
                Text::new(key_label(*key)),
                text_font(font.clone(), 21.0),
                TextColor(Color::WHITE),
            ))
            .id();

        commands.entity(entity).add_child(label);
    }
}

pub fn controls_update_system(
    mut edit_query: Query<
        (&Interaction, &EditControlButton, &Children, &mut Node),
        Changed<Interaction>,
    >,
    mut clear_query: Query<
        (&Interaction, &ClearButton, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut recorder_query: Query<(&mut ActionRecorder, &mut Visibility)>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    input: Res<ButtonInput<KeyCode>>,
    mut key_map: ResMut<KeyMap>,
) {
    let Ok((mut recorder, mut recorder_visibility)) = recorder_query.single_mut() else {
        return;
    };

    // While the dialog is open, the first key pressed is the one to bind
    if *recorder_visibility == Visibility::Visible {
        if let Some(key) = input.get_just_pressed().next() {
            *recorder_visibility = Visibility::Hidden;
            let keys = key_map.map.entry(recorder.action).or_default();
            keys.push(*key);
            update_input_component(&mut commands, recorder.binds, keys, &assets);
            return;
        }
    }

    for (interaction, line, children, mut node) in edit_query.iter_mut() {
        let clear_column_width = match *interaction {
            Interaction::Pressed => {
                // A click on the "clear" button of the line reaches the line too: it is not
                // a request for a new key
                if children.iter().any(|child| clear_query.contains(child)) {
                    continue;
                }
                *recorder_visibility = Visibility::Visible;
                recorder.action = line.action;
                recorder.binds = line.binds;
                continue;
            }
            Interaction::Hovered => CLEAR_COLUMN_SHOWN,
            Interaction::None => CLEAR_COLUMN_HIDDEN,
        };
        node.grid_template_columns.pop();
        node.grid_template_columns
            .push(RepeatedGridTrack::px(1, clear_column_width));
    }

    for (interaction, clear, mut background) in clear_query.iter_mut() {
        match *interaction {
            Interaction::Pressed => {
                key_map.map.insert(clear.action, Vec::new());
                update_input_component(&mut commands, clear.binds, &[], &assets);
            }
            Interaction::Hovered => background.0 = Color::Srgba(css::RED),
            Interaction::None => background.0 = NORMAL_BUTTON,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_labelled_without_their_prefix_and_arrows_as_arrows() {
        assert_eq!(key_label(KeyCode::KeyW), "W");
        assert_eq!(key_label(KeyCode::Space), "Space");
        assert_eq!(key_label(KeyCode::ArrowLeft), "←");
        assert_eq!(key_label(KeyCode::ArrowDown), "↓");
    }

    /// The controls screen for a key map of one action, driven by the real systems.
    fn screen() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_asset::<Image>()
            .insert_resource(KeyMap {
                map: [(GameAction::Jump, vec![KeyCode::Space])].into(),
            })
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(GameFolderPaths {
                game_folder_path: "game".into(),
                assets_folder_path: "assets".into(),
            })
            .add_systems(Startup, spawn_tab)
            .add_systems(Update, controls_update_system);
        app.update();
        app
    }

    fn spawn_tab(
        mut commands: Commands,
        assets: Res<AssetServer>,
        key_map: Res<KeyMap>,
        paths: Res<GameFolderPaths>,
    ) {
        let panel = commands.spawn(Node::default()).id();
        spawn_controls_tab(&mut commands, panel, &assets, &key_map, &paths);
    }

    fn only<C: Component>(app: &mut App) -> Entity {
        let mut query = app.world_mut().query_filtered::<Entity, With<C>>();
        query.single(app.world()).unwrap()
    }

    fn recorder_is_open(app: &mut App) -> bool {
        let recorder = only::<ActionRecorder>(app);
        *app.world().get::<Visibility>(recorder).unwrap() == Visibility::Visible
    }

    fn bound_keys(app: &App) -> Vec<KeyCode> {
        app.world().resource::<KeyMap>().map[&GameAction::Jump].clone()
    }

    #[test]
    fn clicking_a_line_then_pressing_a_key_adds_the_key_to_the_action() {
        let mut app = screen();
        let line = only::<EditControlButton>(&mut app);

        app.world_mut()
            .entity_mut(line)
            .insert(Interaction::Pressed);
        app.update();
        assert!(recorder_is_open(&mut app));

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyJ);
        app.update();
        assert!(!recorder_is_open(&mut app));
        assert_eq!(bound_keys(&app), [KeyCode::Space, KeyCode::KeyJ]);

        // The line shows one label per key
        let binds = app.world().get::<EditControlButton>(line).unwrap().binds;
        assert_eq!(app.world().get::<Children>(binds).unwrap().len(), 2);
    }

    #[test]
    fn the_clear_button_removes_the_keys_without_asking_for_a_new_one() {
        let mut app = screen();
        let line = only::<EditControlButton>(&mut app);
        let clear = only::<ClearButton>(&mut app);

        // A click on the button reaches the line under it too
        app.world_mut()
            .entity_mut(line)
            .insert(Interaction::Pressed);
        app.world_mut()
            .entity_mut(clear)
            .insert(Interaction::Pressed);
        app.update();

        assert!(bound_keys(&app).is_empty());
        assert!(!recorder_is_open(&mut app));
    }
}
