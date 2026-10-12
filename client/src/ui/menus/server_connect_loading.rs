use crate::state::GameState;
use crate::ui::assets::{MONO_FONT_PATH, SANS_FONT_PATH};
use bevy::{color::palettes::tailwind::YELLOW_500, prelude::*};

#[derive(Component)]
pub struct CancelButtonMarker;

#[derive(Component)]
pub struct LoadingTextMarker;

pub fn setup_server_connect_loading_screen(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root_bundle = (
        Node {
            width: Val::Percent(100.),
            height: Val::Percent(100.),
            display: Display::Flex,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: Val::Px(60.),
            ..default()
        },
        DespawnOnExit(GameState::PreGameLoading),
    );

    let loading_text_bundle = (
        Text::new("Connecting to server"),
        TextFont {
            font: asset_server.load(SANS_FONT_PATH).into(),
            font_size: 67.0.into(),
            ..default()
        },
        LoadingTextMarker,
    );

    let cancel_button_bundle = (
        Text::new("[Cancel]"),
        TextFont {
            font: asset_server.load(MONO_FONT_PATH).into(),
            font_size: 32.0.into(),
            ..default()
        },
        TextColor(Color::from(YELLOW_500)),
        CancelButtonMarker,
        Button,
    );

    commands.spawn(root_bundle).with_children(|p| {
        p.spawn(loading_text_bundle);
        p.spawn(cancel_button_bundle);
    });
}

pub fn update_server_connect_loading_screen(
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<CancelButtonMarker>)>,
    mut game_state: ResMut<NextState<GameState>>,
    mut loading_text_query: Query<&mut Text, With<LoadingTextMarker>>,
    mut main_counter: Local<u64>,
    mut dot_counter: Local<u64>,
) {
    *main_counter += 1;

    if (*main_counter).is_multiple_of(20) {
        for mut text in loading_text_query.iter_mut() {
            text.0 = format!(
                "Connecting to server{}",
                ".".repeat((*dot_counter % 4) as usize)
            );
        }
        *dot_counter += 1;
    }

    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            info!("Cancel button clicked");
            game_state.set(GameState::Menu);
        }
    }
}
