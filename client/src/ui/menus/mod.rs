pub mod background;
pub mod home;
pub mod list_screen;
pub mod multi;
pub mod pause;
pub mod server_connect_loading;
pub mod servers;
pub mod settings;
pub mod solo;
pub mod worlds;

use bevy::prelude::*;
use bevy_inspector_egui::bevy_egui::PrimaryEguiContext;
pub use home::*;
pub use server_connect_loading::*;

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};

use multi::multiplayer_action;

use crate::state::{GameState, MenuCamera};
use crate::ui::widgets::dialog::dialog_open;
use crate::ui::widgets::text_field::text_field_focused;

use super::button::*;

pub fn menu_plugin(app: &mut App) {
    app.init_state::<MenuState>()
        .add_systems(OnEnter(GameState::Menu), menu_setup)
        // Systems to handle the main menu screen
        .add_systems(OnEnter(MenuState::Main), home_setup)
        .add_systems(
            Update,
            open_quit_dialog_on_escape
                .run_if(in_state(MenuState::Main))
                .run_if(not(dialog_open))
                .run_if(not(text_field_focused)),
        )
        .add_systems(
            Update,
            continue_on_ctrl_enter
                .run_if(in_state(MenuState::Main))
                .run_if(not(dialog_open)),
        )
        .add_observer(activate_main_menu_button)
        .add_systems(
            OnEnter(MenuState::Multi),
            (multi::multiplayer_menu_setup, multi::load_server_list).chain(),
        )
        .add_systems(
            Update,
            (multiplayer_action).run_if(in_state(MenuState::Multi)),
        )
        .add_systems(OnExit(MenuState::Multi), multi::save_server_list)
        // Common systems to all screens that handles buttons behavior
        .add_systems(
            Update,
            (
                menu_action,
                escape_button
                    .run_if(not(dialog_open))
                    .run_if(not(text_field_focused)),
                button_system,
                mouse_scroll,
            )
                .run_if(in_state(GameState::Menu)),
        )
        // The loading screen is shown while connecting, between the menu and the game
        .add_systems(
            Update,
            update_server_connect_loading_screen.run_if(in_state(GameState::PreGameLoading)),
        );
    settings::settings_plugin(app);
    solo::solo_plugin(app);
}

/// Spawns the camera of the menus and opens the main menu.
fn menu_setup(mut menu_state: ResMut<NextState<MenuState>>, mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MenuCamera,
        PrimaryEguiContext,
        DespawnOnExit(GameState::Menu),
    ));
    menu_state.set(MenuState::Main);
}

fn menu_action(
    interaction_query: Query<
        (&Interaction, &MenuButtonAction),
        (Changed<Interaction>, With<Button>),
    >,
    mut menu_state: ResMut<NextState<MenuState>>,
) {
    for (interaction, menu_button_action) in &interaction_query {
        if *interaction == Interaction::Pressed {
            match menu_button_action {
                MenuButtonAction::BackToMainMenu => menu_state.set(MenuState::Main),
            }
        }
    }
}

fn escape_button(
    keys: Res<ButtonInput<KeyCode>>,
    menu_state: ResMut<State<MenuState>>,
    mut next_menu_state: ResMut<NextState<MenuState>>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        match menu_state.get() {
            MenuState::Solo | MenuState::Multi => next_menu_state.set(MenuState::Main),
            // The settings close themselves
            _ => (),
        }
    }
}

pub fn mouse_scroll(
    mut mouse_wheel_events: MessageReader<MouseWheel>,
    mut query_list: Query<(&mut ScrollingList, &mut Node, &ChildOf, &ComputedNode)>,
    query_node: Query<&ComputedNode>,
) {
    for mouse_wheel_event in mouse_wheel_events.read() {
        debug!("MouseEvent");
        for (mut scrolling_list, mut style, child_of, list_node) in &mut query_list {
            let items_height = list_node.size().y;
            let Ok(container) = query_node.get(child_of.parent()) else {
                continue;
            };
            let container_height = container.size().y;

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
