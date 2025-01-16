use crate::constants::INTERACTION_DISTANCE;
use crate::player::{CurrentPlayerMarker, ViewMode};
use crate::world::{raycast, ClientWorldMap};
use bevy::prelude::*;

#[derive(Component)]
pub struct BlockText;

// Updates UI to display the block the player is looking at (or none if no block is within INTERACTION_DISTANCE)
pub fn block_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    world_map: Res<ClientWorldMap>,
    mut query: Query<(&mut Text, &mut TextColor), With<BlockText>>,
    camera_query: Query<&Transform, With<Camera>>,
    view_mode: Res<ViewMode>,
) {
    let mut col = Color::srgb(1.0, 1.0, 1.0);
    let mut txt = "<none>".to_string();

    let camera_transform = camera_query.single();
    let player_transform = player.single();

    let maybe_block = raycast(&world_map, camera_transform, player_transform, *view_mode);

    if let Some(res) = maybe_block {
        let pos = res.position;
        // Check if block is close enough to the player
        let block_pos = Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32);
        if (block_pos - player.single().translation).length() < INTERACTION_DISTANCE {
            col = Color::WHITE;
            txt = format!("{:?} pos = ({}, {}, {})", res.block, pos.x, pos.y, pos.z);
        }
    }

    for (mut text, mut color) in query.iter_mut() {
        // Update the text content
        **text = txt.clone();
        **color = col;
    }
}
