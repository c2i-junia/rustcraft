use crate::player::CurrentPlayerMarker;
use crate::world::ClientWorldMap;
use crate::{camera::BlockRaycastSet, constants::INTERACTION_DISTANCE};
use bevy::prelude::*;
use bevy_mod_raycast::prelude::RaycastSource;

#[derive(Component)]
pub struct BlockText;

// Updates UI to display the block the player is looking at (or none if no block is within INTERACTION_DISTANCE)
pub fn block_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    world_map: Res<ClientWorldMap>,
    mut query: Query<(&mut Text, &mut TextColor), With<BlockText>>,
    raycast_source: Query<&RaycastSource<BlockRaycastSet>>, // Raycast to get current "selected" block
) {
    let raycast_source = raycast_source.single();
    let mut col = Color::srgb(1.0, 1.0, 1.0);
    let mut txt = "<none>".to_string();

    if let Some((_entity, intersection)) = raycast_source.intersections().first() {
        // Check if block is close enough to the player
        if (intersection.position() - player.single().translation).length() < INTERACTION_DISTANCE {
            let block_pos = intersection.position() - intersection.normal() / 10.0;
            let vec = IVec3::new(
                block_pos.x.round() as i32,
                block_pos.y.round() as i32,
                block_pos.z.round() as i32,
            );
            if let Some(block) = world_map.get_block_by_coordinates(&vec) {
                col = Color::WHITE;
                txt = format!(
                    "{:?} | pos = {}",
                    block,
                    intersection.position().xyz().round()
                );
            }
        }
    }

    for (mut text, mut color) in query.iter_mut() {
        // Update the text content
        **text = txt.clone();
        **color = col;
    }
}
