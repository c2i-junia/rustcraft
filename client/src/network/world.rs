use std::collections::VecDeque;

use bevy::prelude::*;
use bevy::tasks::{block_on, futures_lite::future, ComputeTaskPool, Task};
use bevy_renet::RenetClient;
use shared::messages::{
    mob::MobUpdateEvent, BlockUpdate, DroppedItemsUpdate, PlayerSpawnEvent, PlayerUpdateEvent,
    ServerToClientMessage,
};

use crate::mob::ClientMobs;
use shared::world::SIX_OFFSETS;
use shared::{payload_to_game_message, STC_CHUNK_DATA_CHANNEL, STC_STANDARD_CHANNEL};

use crate::world::time::ClientTime;
use crate::world::ClientWorldMap;

use crate::world::{BlockChanged, ChunkLoadEvent, WorldRenderRequestUpdateEvent};

use super::CachedChatConversation;
use super::SendGameMessageExtension;

/// A message of the chunk channel this large or larger is decoded on the task pool: decoding
/// the chunks of a large one would stall the frame. A smaller one, a block update, is decoded
/// at once when nothing is waiting before it.
const DECODED_APART_BYTES: usize = 4 * 1024;

/// The most chunks applied in a frame, give or take a message: the server sends thousands of
/// chunks of air or stone at once to a client with a large render distance, and adding each to
/// the map and asking for the meshes around it would stall the frame.
const CHUNKS_APPLIED_PER_FRAME: usize = 1024;

/// The messages of the chunk channel being decoded on the task pool, in the order they came:
/// they are applied in that order, each once it and those before it are decoded.
#[derive(Resource, Default)]
pub struct DecodingChunkMessages(VecDeque<Task<Option<ServerToClientMessage>>>);

impl DecodingChunkMessages {
    /// Forgets the messages being decoded, those of a session that ended.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Takes the messages that came on the chunk channel, and returns those ready to apply, in
    /// order, up to about `CHUNKS_APPLIED_PER_FRAME` chunks: the others wait for the next frame.
    fn receive(&mut self, client: &mut RenetClient) -> Vec<ServerToClientMessage> {
        let mut ready = Vec::new();
        while let Some(payload) = client.receive_message(STC_CHUNK_DATA_CHANNEL) {
            if self.0.is_empty() && payload.len() < DECODED_APART_BYTES {
                ready.extend(decode(&payload));
                continue;
            }
            self.0
                .push_back(ComputeTaskPool::get().spawn(async move { decode(&payload) }));
        }
        let mut chunks = 0;
        while let Some(task) = self.0.front_mut() {
            if chunks >= CHUNKS_APPLIED_PER_FRAME {
                break;
            }
            let Some(message) = block_on(future::poll_once(task)) else {
                break;
            };
            self.0.pop_front();
            if let Some(ServerToClientMessage::WorldUpdate(update)) = &message {
                chunks += update.new_map.len();
            }
            ready.extend(message);
        }
        ready
    }
}

fn decode(payload: &[u8]) -> Option<ServerToClientMessage> {
    payload_to_game_message(payload)
        .inspect_err(|err| warn!("Error deserializing message: {err:?}"))
        .ok()
}

pub fn update_world_from_network(
    commands: &mut Commands,
    client: &mut ResMut<RenetClient>,
    world: &mut ResMut<ClientWorldMap>,
    ev_render: &mut MessageWriter<WorldRenderRequestUpdateEvent>,
    ev_chunk_load: &mut MessageWriter<ChunkLoadEvent>,
    ev_player_spawn: &mut MessageWriter<PlayerSpawnEvent>,
    mobs: &mut ClientMobs,
    ev_player_update: &mut MessageWriter<PlayerUpdateEvent>,
    ev_block_changed: &mut MessageWriter<BlockChanged>,
    ev_dropped_items: &mut MessageWriter<DroppedItemsUpdate>,
    chat: &mut ResMut<CachedChatConversation>,
    client_time: &mut ResMut<ClientTime>,
    decoding: &mut DecodingChunkMessages,
) {
    let mut messages = Vec::new();
    while let Some(Ok(msg)) = client.receive_game_message_by_channel(STC_STANDARD_CHANNEL) {
        messages.push(msg);
    }
    messages.extend(decoding.receive(client));
    for msg in messages {
        match msg {
            ServerToClientMessage::WorldUpdate(world_update) => {
                debug!(
                    "Received world update, {} chunks received",
                    world_update.new_map.len()
                );

                for (pos, chunk) in world_update.new_map {
                    world.blocks.insert_chunk(pos, chunk);
                    ev_chunk_load.write(ChunkLoadEvent::Loaded(pos));
                    world.chunk_arrived(pos);
                    // The new chunk, and the neighbours whose faces toward it may now be hidden
                    ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(pos));
                    for offset in SIX_OFFSETS {
                        ev_render.write(WorldRenderRequestUpdateEvent::ChunkToReload(pos + offset));
                    }
                }

                for (id, mob) in world_update.mobs {
                    mobs.update(id, mob);
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
            ServerToClientMessage::MobUpdate(MobUpdateEvent { id, mob }) => {
                mobs.update(id, mob);
            }
            ServerToClientMessage::MobRemoved(id) => {
                mobs.remove(id);
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
            ServerToClientMessage::WeatherSet(_)
            | ServerToClientMessage::LightningStrike { .. } => {}
        }
    }
}
