use crate::messages::PlayerId;
use crate::players::Player;
use crate::world::block_to_chunk_coord;
use crate::world::global_block_to_chunk_pos;
use crate::world::to_local_pos;
use crate::world::BlockId;
use crate::CHUNK_SIZE;
use bevy::math::bounding::Aabb3d;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

use super::BlockData;
use super::ItemId;
use super::ItemType;
use super::MobId;
use super::ServerMob;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct ServerItemStack {
    pub id: u128,
    pub despawned: bool,
    pub stack: ItemStack,
    pub pos: Vec3,
    pub timestamp: u64,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct ServerChunk {
    pub map: HashMap<IVec3, BlockData>,
    /// Timestamp marking the last update this chunk has received
    pub ts: u64,
    pub sent_to_clients: Vec<PlayerId>,
}

// #[derive(Resource)]
// pub struct PlayerInventories(HashMap<PlayerId, Inventory>);

#[derive(Resource, Default, Clone, Serialize, Deserialize, Debug)]
pub struct ServerWorldMap {
    pub name: String,
    pub chunks: ServerChunkWorldMap,
    pub players: HashMap<PlayerId, Player>,
    pub mobs: HashMap<MobId, ServerMob>,
    pub item_stacks: Vec<ServerItemStack>,
    pub time: u64,
}

#[derive(Default, Clone, Serialize, Deserialize, Debug)]
pub struct ServerChunkWorldMap {
    pub map: HashMap<IVec3, ServerChunk>,
    pub chunks_to_update: Vec<IVec3>,
}

#[derive(Resource, Clone, Copy, Serialize, Deserialize)]
pub struct WorldSeed(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default)]
pub struct ItemStack {
    pub item_id: ItemId,
    pub item_type: ItemType,
    pub nb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeType {
    Plains,
    Forest,
    MediumMountain,
    HighMountainGrass,
    Desert,
    IcePlain,
    FlowerPlains,
    ShallowOcean,
    Ocean,
    DeepOcean,
}

#[derive(Debug, Clone, Copy)]
pub struct Biome {
    pub biome_type: BiomeType,
    pub base_height: i32,
    pub height_variation: i32,
    pub surface_block: BlockId,
    pub sub_surface_block: BlockId,
}

pub fn get_biome_data(biome_type: BiomeType) -> Biome {
    match biome_type {
        BiomeType::Plains => Biome {
            biome_type: BiomeType::Plains,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Forest => Biome {
            biome_type: BiomeType::Forest,
            base_height: 64,
            height_variation: 2,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::MediumMountain => Biome {
            biome_type: BiomeType::MediumMountain,
            base_height: 70,
            height_variation: 4,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::HighMountainGrass => Biome {
            biome_type: BiomeType::HighMountainGrass,
            base_height: 75,
            height_variation: 7,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Desert => Biome {
            biome_type: BiomeType::Desert,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::IcePlain => Biome {
            biome_type: BiomeType::IcePlain,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Snow,
            sub_surface_block: BlockId::Ice,
        },
        BiomeType::FlowerPlains => Biome {
            biome_type: BiomeType::FlowerPlains,
            base_height: 64,
            height_variation: 1,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::ShallowOcean => Biome {
            biome_type: BiomeType::ShallowOcean,
            base_height: 60,
            height_variation: 1,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::Ocean => Biome {
            biome_type: BiomeType::DeepOcean,
            base_height: 55,
            height_variation: 2,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::DeepOcean => Biome {
            biome_type: BiomeType::DeepOcean,
            base_height: 50,
            height_variation: 3,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
    }
}

pub trait WorldMap {
    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData>;
    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData>;
    fn set_block(&mut self, position: &IVec3, block: BlockData);

    fn get_heigh_ground(&self, position: Vec3) -> i32 {
        for y in (0..256).rev() {
            if self
                .get_block_by_coordinates(&IVec3::new(position.x as i32, y, position.z as i32))
                .is_some()
            {
                return y;
            }
        }
        0
    }

    fn check_collision_box(&self, hitbox: &Aabb3d) -> bool {
        // Check all blocks inside the hitbox
        for x in (hitbox.min.x as i32)..=(hitbox.max.x as i32) {
            for y in (hitbox.min.y as i32)..=(hitbox.max.y as i32) {
                for z in (hitbox.min.z as i32)..=(hitbox.max.z as i32) {
                    if let Some(block) = self.get_block_by_coordinates(&IVec3::new(x, y, z)) {
                        if block.id.has_hitbox() {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    fn check_collision_point(&self, point: &Vec3) -> bool {
        if let Some(block) = self.get_block_by_coordinates(&IVec3::new(
            point.x as i32,
            point.y as i32,
            point.z as i32,
        )) {
            block.id.has_hitbox()
        } else {
            false
        }
    }

    fn get_surrounding_chunks(&self, position: Vec3, radius: i32) -> Vec<IVec3> {
        let mut chunks = Vec::new();
        let x = position.x as i32;
        let y = position.y as i32;
        let z = position.z as i32;
        let cx = block_to_chunk_coord(x);
        let cy = block_to_chunk_coord(y);
        let cz = block_to_chunk_coord(z);
        for i in -radius..=radius {
            for j in -radius..=radius {
                for k in -radius..=radius {
                    chunks.push(IVec3::new(cx + i, cy + j, cz + k));
                }
            }
        }
        chunks
    }
}

impl WorldMap for ServerChunkWorldMap {
    fn get_block_by_coordinates(&self, position: &IVec3) -> Option<&BlockData> {
        let x: i32 = position.x;
        let y: i32 = position.y;
        let z: i32 = position.z;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        let chunk: Option<&ServerChunk> = self.map.get(&IVec3::new(cx, cy, cz));
        match chunk {
            Some(chunk) => {
                let sub_x: i32 = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                let sub_y: i32 = ((y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                let sub_z: i32 = ((z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
                chunk.map.get(&IVec3::new(sub_x, sub_y, sub_z))
            }
            None => {
                warn!("Chunk not found for block at {:?}", position);
                None
            }
        }
    }

    fn remove_block_by_coordinates(&mut self, global_block_pos: &IVec3) -> Option<BlockData> {
        let block: &BlockData = self.get_block_by_coordinates(global_block_pos)?;
        let kind: BlockData = *block;

        let chunk_pos: IVec3 = global_block_to_chunk_pos(global_block_pos);
        let cx = chunk_pos.x;
        let cy = chunk_pos.y;
        let cz = chunk_pos.z;

        let chunk_map: &mut ServerChunk =
            self.map
                .get_mut(&IVec3::new(chunk_pos.x, chunk_pos.y, chunk_pos.z))?;

        let local_block_pos: IVec3 = to_local_pos(global_block_pos);

        chunk_map.map.remove(&local_block_pos);
        self.chunks_to_update.push(IVec3::new(cx, cy, cz));

        Some(kind)
    }

    fn set_block(&mut self, position: &IVec3, block: BlockData) {
        let x: i32 = position.x;
        let y: i32 = position.y;
        let z: i32 = position.z;
        let cx: i32 = block_to_chunk_coord(x);
        let cy: i32 = block_to_chunk_coord(y);
        let cz: i32 = block_to_chunk_coord(z);
        let chunk: &mut ServerChunk = self.map.entry(IVec3::new(cx, cy, cz)).or_default();
        let sub_x: i32 = ((x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let sub_y: i32 = ((y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;
        let sub_z: i32 = ((z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE;

        chunk.map.insert(IVec3::new(sub_x, sub_y, sub_z), block);
        self.chunks_to_update.push(IVec3::new(cx, cy, cz));
    }
}

/// Global trait for all numerical enums serving as unique IDs for certain
/// types of elements in the game. Example : ItemId, BlockId...
/// Used in texture atlases and such
pub trait GameElementId: std::hash::Hash + Eq + PartialEq + Copy + Clone + Default + Debug {}
