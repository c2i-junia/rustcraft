use crate::network::save::send_save_request_to_server;
use bevy::{
    asset::AssetServer,
    color::{Alpha, Color},
    input::ButtonInput,
    prelude::*,
    ui::{
        AlignItems, BackgroundColor, BorderColor, Display, FlexDirection, FocusPolicy, Interaction,
        JustifyContent, Node, UiRect, Val,
    },
};
use bevy_renet::RenetClient;

use crate::input::keyboard::is_action_just_pressed;
use crate::state::{GameState, KeyMap};

use crate::ui::hud::{InventoryRoot, UIMode, UiDialog};

#[derive(Component)]
pub struct PauseMenu;

#[derive(Component)]
pub enum PauseButtonAction {
    Resume,
    Save,
    Menu,
}

pub fn setup_pause_menu(mut commands: Commands, assets: Res<AssetServer>) {
    commands
        .spawn((
            PauseMenu,
            UiDialog,
            Name::new("PauseMenu"),
            DespawnOnExit(GameState::Game),
            BackgroundColor(Color::BLACK.with_alpha(0.6)),
            (
                Node {
                    width: Val::Vw(100.),
                    height: Val::Vh(100.),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..Default::default()
                },
                FocusPolicy::Block,
                Visibility::Hidden,
            ),
            GlobalZIndex(5),
        ))
        .with_children(|root| {
            root.spawn(Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceAround,
                height: Val::Vh(40.),
                min_width: Val::Vw(40.),
                ..Default::default()
            })
            .with_children(|wrapper| {
                for (msg, action) in [
                    ("Resume", PauseButtonAction::Resume),
                    ("Save", PauseButtonAction::Save),
                    ("Back to menu", PauseButtonAction::Menu),
                ] {
                    wrapper
                        .spawn((
                            action,
                            (
                                Button,
                                Node {
                                    width: Val::Percent(100.),
                                    border: UiRect::all(Val::Px(3.)),
                                    display: Display::Flex,
                                    flex_direction: FlexDirection::Row,
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    padding: UiRect::all(Val::Px(7.)),
                                    ..Default::default()
                                },
                                BackgroundColor(Color::srgb(0.3, 0.3, 0.3)),
                                BorderColor::all(Color::BLACK),
                            ),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(msg),
                                TextFont {
                                    font: assets.load("./fonts/RustCraftRegular-Bmg3.otf").into(),
                                    font_size: 20.0.into(),
                                    font_smoothing: default(),
                                    ..default()
                                },
                                TextColor(Color::WHITE),
                            ));
                        });
                }
            });
        });
}

pub fn render_pause_menu(
    queries: (
        Query<(&PauseButtonAction, &mut BorderColor, Ref<Interaction>)>,
        Query<&mut Visibility, With<PauseMenu>>,
        Query<&mut Visibility, (With<InventoryRoot>, Without<PauseMenu>)>,
    ),
    input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    mut game_state: ResMut<NextState<GameState>>,
    key_map: Res<KeyMap>,
    mut client: ResMut<RenetClient>,
) {
    let (mut button, mut visibility, mut inventory) = queries;
    let mut vis = visibility.single_mut().unwrap();

    if is_action_just_pressed(crate::input::data::GameAction::Escape, &input, &key_map) {
        if *vis == Visibility::Hidden {
            if *ui_mode == UIMode::Closed {
                *vis = Visibility::Visible;
            } else if let Ok(mut inventory) = inventory.single_mut() {
                // Escape closes the inventory; the chat closes itself
                *inventory = Visibility::Hidden;
            }
        } else {
            *vis = Visibility::Hidden;
        }
    }

    if *vis != Visibility::Visible {
        return;
    }

    for (action, mut bcolor, interaction) in button.iter_mut() {
        match *interaction {
            // Act once per click, not on every frame the button stays pressed
            Interaction::Pressed if !interaction.is_changed() => {}
            Interaction::Pressed => match *action {
                PauseButtonAction::Menu => {
                    game_state.set(GameState::Menu);
                }
                PauseButtonAction::Resume => {
                    *vis = Visibility::Hidden;
                }
                PauseButtonAction::Save => {
                    send_save_request_to_server(&mut client);
                }
            },
            Interaction::Hovered => {
                *bcolor = BorderColor::all(Color::WHITE);
            }
            Interaction::None => {
                *bcolor = BorderColor::all(Color::BLACK);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::data::GameAction;
    use bevy::state::app::StatesPlugin;
    use shared::get_shared_renet_config;
    use std::collections::BTreeMap;

    fn app(ui_mode: UIMode) -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_state::<GameState>()
            .init_resource::<ButtonInput<KeyCode>>()
            .insert_resource(RenetClient::new(get_shared_renet_config()))
            .insert_resource(KeyMap {
                map: BTreeMap::from([(GameAction::Escape, vec![KeyCode::Escape])]),
            })
            .insert_resource(ui_mode)
            .add_systems(Update, render_pause_menu);
        app.world_mut().spawn((PauseMenu, Visibility::Hidden));
        app.world_mut().spawn((InventoryRoot, Visibility::Visible));
        app
    }

    fn press_escape(app: &mut App) {
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();
    }

    fn visibility_of<C: Component>(app: &mut App) -> Visibility {
        *app.world_mut()
            .query_filtered::<&Visibility, With<C>>()
            .single(app.world())
            .unwrap()
    }

    #[test]
    fn escape_opens_the_pause_menu_during_gameplay() {
        let mut app = app(UIMode::Closed);
        press_escape(&mut app);
        assert_eq!(visibility_of::<PauseMenu>(&mut app), Visibility::Visible);
    }

    #[test]
    fn escape_closes_the_inventory_instead_of_opening_the_pause_menu() {
        let mut app = app(UIMode::Opened);
        press_escape(&mut app);
        assert_eq!(visibility_of::<PauseMenu>(&mut app), Visibility::Hidden);
        assert_eq!(visibility_of::<InventoryRoot>(&mut app), Visibility::Hidden);
    }

    #[test]
    fn escape_while_typing_does_not_open_the_pause_menu() {
        let mut app = app(UIMode::Typing);
        press_escape(&mut app);
        assert_eq!(visibility_of::<PauseMenu>(&mut app), Visibility::Hidden);
    }
}
