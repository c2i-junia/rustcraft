use crate::player::CurrentPlayerMarker;
use bevy::prelude::*;
use shared::world::block_to_chunk_coord;

#[derive(Component)]
pub struct CoordsText;

pub fn coords_text_update_system(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    query: Query<Entity, With<CoordsText>>,
    mut writer: TextUiWriter,
) {
    let coords = player.single().unwrap();
    let player_chunk = IVec3::new(
        block_to_chunk_coord(coords.translation.x as i32),
        block_to_chunk_coord(coords.translation.y as i32),
        block_to_chunk_coord(coords.translation.z as i32),
    );

    for entity in query.iter() {
        *writer.text(entity, 0) = format!(
            "X/Y/Z = {:.2}/{:.2}/{:.2}\nChunk pos : {:?}",
            coords.translation.x, coords.translation.y, coords.translation.z, player_chunk
        );
    }
}
