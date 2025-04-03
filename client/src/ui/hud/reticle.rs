use bevy::prelude::*;

use crate::GameState;

pub fn spawn_reticle(mut commands: Commands) {
    // Main container for the reticle
    commands
        .spawn((
            StateScoped(GameState::Game), // Link the reticle to the Game state
            Node {
                position_type: PositionType::Absolute,
                margin: UiRect {
                    left: Val::Auto,
                    right: Val::Auto,
                    top: Val::Auto,
                    bottom: Val::Auto,
                },
                ..default()
            },
        ))
        .with_children(|parent| {
            // Horizontal line (horizontal bar of the cross)
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(20.0),
                    height: Val::Px(2.0),
                    left: Val::Px(-10.0),
                    top: Val::Px(-1.0),
                    ..Default::default()
                },
                BackgroundColor(Color::WHITE),
            ));

            // Vertical line (vertical bar of the cross)
            parent.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(2.0),
                    height: Val::Px(20.0),
                    left: Val::Px(-1.0),
                    top: Val::Px(-10.0),
                    ..Default::default()
                },
                BackgroundColor(Color::WHITE),
            ));
        });
}
