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

use bevy::ecs::system::SystemParam;
use bevy::input_focus::{FocusCause, InputFocus, InputFocusVisible};

use crate::state::{GameState, MenuCamera};
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::widgets::dialog::{dialog_open, CloseRequest, DialogRoot};
use crate::ui::widgets::hints::{hint_bar, Hint};
use crate::ui::widgets::list::ListCard;
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

/// The key hints of a list screen, named by the marker of its hint bar: those shown while a card
/// of the list has the focus, those shown otherwise, those shown while the list is empty, and
/// those shown while it cannot be read.
trait ListHints: Component + Default {
    const CARDS: &'static [Hint];
    const SCREEN: &'static [Hint];
    const EMPTY: &'static [Hint] = Self::SCREEN;
    const UNREADABLE: &'static [Hint] = Self::SCREEN;
}

/// On the list of a list screen without a card: its hint bar shows `ListHints::EMPTY`, without
/// the keys that do nothing then.
#[derive(Component)]
struct ListEmpty;

/// On the list of a list screen whose file cannot be read: its hint bar shows
/// `ListHints::UNREADABLE`, without the keys that do nothing then.
#[derive(Component)]
struct ListUnreadable;

/// Which key hints the hint bar of a list screen shows.
#[derive(Component, PartialEq, Eq, Clone, Copy)]
enum HintsShown {
    /// A card has the focus.
    Cards,
    /// Anything else has it.
    Elsewhere,
    /// The list is empty.
    Empty,
    /// The list cannot be read.
    Unreadable,
}

/// The hint bar of the list screen `S`, with the hints for anything but a card.
fn list_hint_bar<S: ListHints>(assets: &AssetServer) -> impl Bundle {
    (
        hint_bar(
            S::SCREEN,
            assets.load(FONT_PATH),
            assets.load(MONO_FONT_PATH),
        ),
        HintsShown::Elsewhere,
        S::default(),
    )
}

/// Keeps the hint bar of the list screen `S` on the hints of the cards while a card has the
/// focus, on those of an unreadable or an empty list while it is one, and on those of the screen
/// otherwise.
fn follow_focus_with_hints<S: ListHints>(
    focus: Res<InputFocus>,
    cards: Query<(), With<ListCard>>,
    unreadable: Query<(), With<ListUnreadable>>,
    empty: Query<(), With<ListEmpty>>,
    bars: Query<(Entity, &HintsShown, &ChildOf), With<S>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let wanted = if !unreadable.is_empty() {
        HintsShown::Unreadable
    } else if !empty.is_empty() {
        HintsShown::Empty
    } else if focus.get().is_some_and(|entity| cards.contains(entity)) {
        HintsShown::Cards
    } else {
        HintsShown::Elsewhere
    };
    let Ok((bar, &shown, child_of)) = bars.single() else {
        return;
    };
    if shown == wanted {
        return;
    }
    let hints = match wanted {
        HintsShown::Cards => S::CARDS,
        HintsShown::Elsewhere => S::SCREEN,
        HintsShown::Empty => S::EMPTY,
        HintsShown::Unreadable => S::UNREADABLE,
    };
    commands.entity(bar).despawn();
    commands.spawn((
        hint_bar(hints, assets.load(FONT_PATH), assets.load(MONO_FONT_PATH)),
        wanted,
        S::default(),
        ChildOf(child_of.parent()),
    ));
}

/// Where the focus goes once the cards of a list screen were built again. A dialog still open
/// (not closing) keeps it.
#[derive(SystemParam)]
struct ListFocus<'w, 's> {
    focus: ResMut<'w, InputFocus>,
    visible: ResMut<'w, InputFocusVisible>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    open_dialogs: Query<'w, 's, (), (With<DialogRoot>, Without<CloseRequest>)>,
}

impl ListFocus<'_, '_> {
    /// Puts the focus on `entity`; on a card reached by the keyboard, with its ring.
    fn set(&mut self, entity: Entity, card: bool) {
        if !self.open_dialogs.is_empty() {
            return;
        }
        self.focus.set(entity, FocusCause::Navigated);
        // reached by the keyboard, the card shows its ring; a click leaves it hidden
        let typed = self.keys.get_just_pressed().next().is_some()
            || self.keys.get_just_released().next().is_some();
        if card && typed {
            self.visible.0 = true;
        }
    }
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
