pub mod home;
pub mod multi;
pub mod pause;
pub mod server_connect_loading;
pub mod settings;
pub mod solo;
pub mod splash;

use bevy::prelude::*;
pub use home::*;
pub use server_connect_loading::*;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

use bevy::app::AppExit;
use multi::multiplayer_action;
use settings::controls::{controls_menu_setup, controls_update_system};

use crate::input::keyboard::save_keybindings;
use crate::{GameState, MenuCamera};

use super::button::*;

pub fn menu_plugin(app: &mut App) {
    app.init_state::<MenuState>()
        .enable_state_scoped_entities::<MenuState>()
        .add_systems(OnEnter(GameState::Menu), menu_setup)
        // Systems to handle the main menu screen
        .add_systems(OnEnter(MenuState::Main), home_setup)
        // Systems to handle the play menu screen
        .add_systems(
            OnEnter(MenuState::Solo),
            (solo::solo_menu_setup, solo::list_worlds).chain(),
        )
        .add_systems(Update, solo::solo_action.run_if(in_state(MenuState::Solo)))
        // Systems to handle the settings menu screen
        .add_systems(OnEnter(MenuState::Settings), settings::settings_menu_setup)
        // Systems to handle the display settings screen
        // Systems to handle the sound settings screen
        // save the keybings when lauching the game, and when exiting settings
        .add_systems(OnEnter(GameState::Menu), save_keybindings)
        .add_systems(OnExit(MenuState::SettingsControls), save_keybindings)
        .add_systems(
            OnEnter(MenuState::Multi),
            (multi::multiplayer_menu_setup, multi::load_server_list).chain(),
        )
        .add_systems(
            Update,
            (multiplayer_action).run_if(in_state(MenuState::Multi)),
        )
        .add_systems(OnExit(MenuState::Multi), multi::save_server_list)
        .add_systems(
            Update,
            controls_update_system.run_if(in_state(MenuState::SettingsControls)),
        )
        // Common systems to all screens that handles buttons behavior
        .add_systems(
            Update,
            (menu_action, escape_button, button_system, mouse_scroll)
                .run_if(in_state(GameState::Menu)),
        )
        .add_systems(OnEnter(MenuState::SettingsControls), controls_menu_setup);
}

/// Tag component for scrolling UI lists
fn menu_setup(mut menu_state: ResMut<NextState<MenuState>>, mut commands: Commands) {
    commands.spawn((Camera2d, MenuCamera, StateScoped(GameState::Menu)));
    menu_state.set(MenuState::Main);
}

fn menu_action(
    interaction_query: Query<
        (&Interaction, &MenuButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut app_exit_events: EventWriter<AppExit>,
    mut menu_state: ResMut<NextState<MenuState>>,
) {
    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match menu_button_action {
                MenuButtonAction::Quit => {
                    app_exit_events.write(AppExit::Success);
                }
                MenuButtonAction::Solo => menu_state.set(MenuState::Solo),
                MenuButtonAction::Settings => menu_state.set(MenuState::Settings),
                MenuButtonAction::BackToMainMenu => menu_state.set(MenuState::Main),
                MenuButtonAction::BackToSettings => {
                    menu_state.set(MenuState::Settings);
                }
                MenuButtonAction::Multi => menu_state.set(MenuState::Multi),
                MenuButtonAction::SettingsControls => menu_state.set(MenuState::SettingsControls),
            }
        }
    }
}

fn escape_button(
    keys: Res<ButtonInput<KeyCode>>,
    mut app_exit_events: EventWriter<AppExit>,
    menu_state: ResMut<State<MenuState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        match menu_state.get() {
            MenuState::Main => {
                app_exit_events.write(AppExit::Success);
            }
            MenuState::Solo | MenuState::Multi | MenuState::Settings => {
                next_menu_state.set(MenuState::Main)
            }
            // todo: decide how we want to bypass keyboard set dialog
            // MenuState::SettingsControls => next_menu_state.set(MenuState::Settings),
            _ => (),
        }
    }
}

pub fn mouse_scroll(
    mut mouse_wheel_events: EventReader<MouseWheel>,
    mut query_list: Query<(&mut ScrollingList, &mut Node, &ChildOf, &ComputedNode)>,
    query_node: Query<&ComputedNode>,
) {
    for mouse_wheel_event in mouse_wheel_events.read() {
        debug!("MouseEvent");
        for (mut scrolling_list, mut style, child_of, list_node) in &mut query_list {
            let items_height = list_node.size().y;
            let container_height = query_node.get(child_of.parent()).unwrap().size().y;

            let max_scroll = (items_height - container_height).max(0.) + 30.;

            let dy = match mouse_wheel_event.unit {
                MouseScrollUnit::Line => mouse_wheel_event.y * 20.,
                MouseScrollUnit::Pixel => mouse_wheel_event.y,
            };

            scrolling_list.position += dy;
            scrolling_list.position = scrolling_list.position.clamp(-max_scroll, 0.);
            style.top = Val::Px(scrolling_list.position);
            debug!(
                "Mouse event : {:?}, {:?}, {:?}, {:?}",
                container_height, max_scroll, items_height, scrolling_list.position
            );
        }
    }
}
