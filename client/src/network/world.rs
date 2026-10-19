use std::time::Instant;

use crate::world::ChunkRender;
use bevy::prelude::*;
use bevy_renet::RenetClient;
use shared::messages::{
    mob::MobUpdateEvent, BlockUpdate, DroppedItemsUpdate, PlayerSpawnEvent, PlayerUpdateEvent,
    ServerToClientMessage,
};
use shared::world::SIX_OFFSETS;
use shared::STC_AUTH_CHANNEL;

use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

use crate::world::{BlockChanged, ChunkLoadEvent, WorldRenderRequestUpdateEvent};

use super::CachedChatConversation;
use super::SendGameMessageExtension;

pub fn update_world_from_network(
    commands: &mut Commands,
    client: &mut ResMut<RenetClient>,
    world: &mut ResMut<ClientWorldMap>,
    ev_render: &mut MessageWriter<WorldRenderRequestUpdateEvent>,
    ev_chunk_load: &mut MessageWriter<ChunkLoadEvent>,
    ev_player_spawn: &mut MessageWriter<PlayerSpawnEvent>,
    ev_mob_update: &mut MessageWriter<MobUpdateEvent>,
    ev_player_update: &mut MessageWriter<PlayerUpdateEvent>,
    ev_block_changed: &mut MessageWriter<BlockChanged>,
    ev_dropped_items: &mut MessageWriter<DroppedItemsUpdate>,
    chat: &mut ResMut<CachedChatConversation>,
    client_time: &mut ResMut<ClientTime>,
) {
    while let Some(Ok(msg)) = client.receive_game_message_except_channel(STC_AUTH_CHANNEL) {
        match msg {
            ServerToClientMessage::WorldUpdate(world_update) => {
                debug!(
                    "Received world update, {} chunks received",
                    world_update.new_map.len()
                );

                for (pos, chunk) in world_update.new_map {
                    world.blocks.insert_chunk(pos, chunk);
                    ev_chunk_load.write(ChunkLoadEvent::Loaded(pos));
                    // Meshes requested before this chunk arrived are outdated
                    world
                        .render
                        .entry(pos)
                        .and_modify(|render| render.last_mesh_ts = Instant::now())
                        .or_insert_with(ChunkRender::new);
                    // The new chunk, and the neighbours whose faces toward it may now be hidden
                    ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(pos));
                    for offset in SIX_OFFSETS {
                        ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(pos + offset));
                    }
                }

                for (id, mob) in world_update.mobs {
                    debug!("ServerMob received: {:?}", mob);
                    ev_mob_update.write(MobUpdateEvent { id, mob });
                }
            }
            ServerToClientMessage::ChunkUnloads(chunks) => {
                debug!("Unloading {} chunks", chunks.len());
                for chunk_pos in chunks {
                    world.unload_chunk(chunk_pos, commands);
                    ev_chunk_load.write(ChunkLoadEvent::Unloaded(chunk_pos));
                }
            }
            ServerToClientMessage::BlockUpdates(updates) => {
                for BlockUpdate { position, block } in updates {
                    let before = world.blocks.get_block(position).copied();
                    // The map ignores an update for a chunk it does not have.
                    match block {
                        Some(block) => world.blocks.set_block(position, block),
                        None => {
                            world.blocks.remove_block(position);
                        }
                    }
                    let after = world.blocks.get_block(position).copied();
                    if after != before {
                        ev_block_changed.write(BlockChanged {
                            position,
                            before,
                            after,
                            by_local_player: false,
                        });
                    }
                }
            }
            ServerToClientMessage::PlayerSpawn(spawn_event) => {
                info!("Received SINGLE spawn event {:?}", spawn_event);
                ev_player_spawn.write(spawn_event);
            }
            ServerToClientMessage::MobUpdate(update_event) => {
                ev_mob_update.write(update_event);
            }
            ServerToClientMessage::PlayerUpdate(update) => {
                ev_player_update.write(update);
            }
            ServerToClientMessage::AuthRegisterResponse(_) => {}
            ServerToClientMessage::DroppedItems(update) => {
                ev_dropped_items.write(update);
            }
            ServerToClientMessage::ChatMessage(message) => {
                chat.unread.push(message);
            }
            ServerToClientMessage::TimeSet(tick) => {
                client_time.0 = tick;
            }
        }
    }
}
