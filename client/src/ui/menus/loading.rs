use crate::GameState;
use bevy::prelude::*;

// Spawns the necessary components for the loading screen.
pub fn load_loading_screen(mut commands: Commands) {
    let text_style = TextStyle {
        font_size: 80.0,
        ..default()
    };

    // Spawn the UI and Loading screen camera.
    commands.spawn((
        StateScoped(GameState::PreGameLoading),
        Camera {
            order: 1,
            ..default()
        },
    ));

    // Spawn the UI that will make up the loading screen.
    commands
        .spawn((
            StateScoped(GameState::PreGameLoading),
            (
                Node {
                    height: Val::Percent(100.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::BLACK),
            ),
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_sections([TextSection::new(
                "Loading...",
                text_style.clone(),
            )]));
        });
}
