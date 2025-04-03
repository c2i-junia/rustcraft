pub mod background_generation;
pub mod broadcast_world;
mod data;
pub mod generation;
pub mod load_from_file;
pub mod save;
pub mod simulation;
pub mod stacks;

use bevy::prelude::Event;
use bevy::prelude::EventReader;
use bevy::prelude::IVec3;
use bevy::prelude::ResMut;
use bevy::prelude::*;
use shared::world::{BlockData, ItemStack, ServerItemStack, ServerWorldMap, WorldMap};
use ulid::Ulid;

#[derive(Event, Debug)]
pub struct BlockInteractionEvent {
    pub position: IVec3,
    pub block_type: Option<BlockData>, // None = delete, Some = add
}

pub fn handle_block_interactions(
    mut world_map: ResMut<ServerWorldMap>,
    mut events: EventReader<BlockInteractionEvent>,
) {
    for event in events.read() {
        match &event.block_type {
            Some(block) => {
                world_map.chunks.set_block(&event.position, *block);
                debug!("Block added at {:?}: {:?}", event.position, block);
            }
            None => {
                // Handle block removal with proper option handling
                if let Some(block) = world_map.chunks.get_block_by_coordinates(&event.position) {
                    for (id, nb) in block.id.get_drops(1) {
                        world_map.item_stacks.push(ServerItemStack {
                            id: Ulid::new().0,
                            despawned: false,
                            stack: ItemStack {
                                item_id: id,
                                item_type: id.get_default_type(),
                                nb,
                            },
                            pos: Vec3::new(
                                event.position.x as f32,
                                event.position.y as f32,
                                event.position.z as f32,
                            ),
                            timestamp: 0,
                        });
                    }

                    world_map
                        .chunks
                        .remove_block_by_coordinates(&event.position);
                    info!("Block removed at {:?}", event.position);
                } else {
                    warn!("No block found at position {:?}", event.position);
                }
            }
        }
    }
}
