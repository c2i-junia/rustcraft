pub mod background;
pub mod home;
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

use crate::state::{GameState, MenuCamera};
use crate::ui::widgets::dialog::dialog_open;
use crate::ui::widgets::text_field::text_field_focused;

/// The screen of the menus shown, `Disabled` outside the menus.
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum MenuState {
    Main,
    Solo,
    Multi,
    Settings,
    #[default]
    Disabled,
}

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
            Update,
            escape_button
                .run_if(in_state(GameState::Menu))
                .run_if(not(dialog_open))
                .run_if(not(text_field_focused)),
        )
        // The loading screen is shown while connecting, between the menu and the game
        .add_systems(
            Update,
            update_server_connect_loading_screen.run_if(in_state(GameState::PreGameLoading)),
        );
    settings::settings_plugin(app);
    solo::solo_plugin(app);
    multi::multi_plugin(app);
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

/// A text of a menu screen, which breaks a long word rather than leave its box.
fn text(content: &str, size: f32, color: Color, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        TextLayout::linebreak(LineBreak::WordOrCharacter),
    )
}

/// The card that takes the focus once the card `name` of a list of `names` is gone: the next
/// one, else the previous.
fn neighbour(names: &[String], name: &str) -> Option<String> {
    let index = names.iter().position(|other| other == name)?;
    names
        .get(index + 1)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|previous| names.get(previous))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deleted_card_hands_the_focus_to_the_next_else_the_previous() {
        let names: Vec<String> = ["A", "B", "C"].map(String::from).to_vec();
        assert_eq!(neighbour(&names, "A").as_deref(), Some("B"));
        assert_eq!(neighbour(&names, "B").as_deref(), Some("C"));
        assert_eq!(neighbour(&names, "C").as_deref(), Some("B"));
        assert_eq!(neighbour(&names[..1], "A"), None);
        assert_eq!(neighbour(&names, "Z"), None);
    }
}
