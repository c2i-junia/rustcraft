use std::collections::{HashMap, HashSet};

use bevy::math::IVec3;
use bevy_ecs::resource::Resource;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{ChunkMap, MobId, ServerMob},
};

pub const SAVE_PATH: &str = "saves/";

/// Everything the server knows about the world it runs.
#[derive(Resource, Default)]
pub struct ServerWorldMap {
    pub name: String,
    pub chunks: ChunkMap,
    /// For each connected client, the chunks it has received and that have not changed since.
    pub sent_chunks: HashMap<PlayerId, HashSet<IVec3>>,
    pub players: HashMap<PlayerId, Player>,
    pub mobs: HashMap<MobId, ServerMob>,
}
