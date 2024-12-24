use crate::player::CurrentPlayerMarker;
use crate::world::ClientWorldMap;
use crate::{
    camera::BlockRaycastSet,
    constants::{CUBE_SIZE, INTERACTION_DISTANCE},
};
use bevy::prelude::*;
use bevy_mod_raycast::prelude::RaycastSource;

#[derive(Component)]
pub struct BlockText;

// Updates UI to display the block the player is looking at (or none if no block is within INTERACTION_DISTANCE)
pub fn block_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    world_map: Res<ClientWorldMap>,
    query: Query<Entity, With<BlockText>>,
    mut writer: TextUiWriter,
    mut text_colors: Query<&mut TextColor>,
    raycast_source: Query<&RaycastSource<BlockRaycastSet>>, // Raycast to get current "selected" block
) {
    let raycast_source = raycast_source.single();
    let mut col = Color::srgb(1.0, 1.0, 1.0);
    let mut txt = "<none>".to_string();

    if let Some((_entity, intersection)) = raycast_source.intersections().first() {
        // Check if block is close enough to the player
        if (intersection.position() - player.single().translation).length() < INTERACTION_DISTANCE {
            let block_pos = intersection.position() - intersection.normal() * (CUBE_SIZE / 2.0);
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

    for entity in query.iter() {
        // Update the text content
        *writer.text(entity, 1) = txt.clone();

        // Update the text color
        if let Ok(mut color) = text_colors.get_mut(entity) {
            color.0 = col;
        }
    }
}
