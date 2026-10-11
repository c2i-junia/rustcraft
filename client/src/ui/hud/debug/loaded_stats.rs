use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;
use bevy::prelude::*;

#[derive(Component)]
pub struct TimeText;

#[derive(Component)]
pub struct ChunksNumberText;

pub fn total_blocks_text_update_system(
    query_chunks: Query<Entity, With<ChunksNumberText>>,
    mut writer: TextUiWriter,
    world_map: Res<ClientWorldMap>,
) {
    for entity in query_chunks.iter() {
        *writer.text(entity, 0) = format!("Loaded chunks: {}", world_map.map.len());
    }
}

pub fn time_text_update_system(
    query: Query<Entity, With<TimeText>>,
    mut writer: TextUiWriter,
    time_resource: Res<ClientTime>,
) {
    for entity in query.iter() {
        *writer.text(entity, 0) = format!("Time: {}", time_resource.0);
    }
}
