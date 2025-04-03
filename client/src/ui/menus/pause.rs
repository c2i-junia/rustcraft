use crate::network::save::send_save_request_to_server;
use bevy::{
    asset::AssetServer,
    color::{Alpha, Color},
    core::Name,
    input::ButtonInput,
    prelude::*,
    ui::{
        AlignItems, BackgroundColor, BorderColor, Display, FlexDirection, FocusPolicy, Interaction,
        JustifyContent, Node, UiRect, Val,
    },
};
use bevy_renet::renet::RenetClient;
use shared::GameFolderPaths;

use crate::{input::keyboard::is_action_just_pressed, GameState, KeyMap};

use crate::ui::hud::UiDialog;

#[derive(Component)]
pub struct PauseMenu;

#[derive(Component)]
pub enum PauseButtonAction {
    Resume,
    Save,
    Menu,
}

pub fn setup_pause_menu(
    mut commands: Commands,
    assets: Res<AssetServer>,
    _paths: Res<GameFolderPaths>,
) {
    commands
        .spawn((
            PauseMenu,
            UiDialog,
            Name::new("PauseMenu"),
            StateScoped(GameState::Game),
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
                                BorderColor(Color::BLACK),
                            ),
                        ))
                        .with_children(|btn| {
                            btn.spawn((
                                Text::new(msg),
                                TextFont {
                                    font: assets.load("./fonts/RustCraftRegular-Bmg3.otf"),
                                    font_size: 20.,
                                    font_smoothing: default(),
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
        Query<(&PauseButtonAction, &mut BorderColor, &Interaction)>,
        Query<&mut Visibility, With<PauseMenu>>,
    ),
    input: Res<ButtonInput<KeyCode>>,
    mut game_state: ResMut<NextState<GameState>>,
    key_map: Res<KeyMap>,
    mut client: ResMut<RenetClient>,
) {
    let (mut button, mut visibility) = queries;
    let mut vis = visibility.single_mut();

    if is_action_just_pressed(crate::input::data::GameAction::Escape, &input, &key_map) {
        *vis = match *vis {
            Visibility::Visible | Visibility::Inherited => Visibility::Hidden,
            Visibility::Hidden => Visibility::Visible,
        }
    }

    if *vis != Visibility::Visible {
        return;
    }

    for (action, mut bcolor, interaction) in button.iter_mut() {
        match *interaction {
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
                bcolor.0 = Color::WHITE;
            }
            Interaction::None => {
                bcolor.0 = Color::BLACK;
            }
        }
    }
}
