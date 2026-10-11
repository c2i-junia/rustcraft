use crate::world::BlockId;

use bevy_ecs::resource::Resource;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

use super::{ItemId, ItemType};

#[derive(Resource, Clone, Copy, Serialize, Deserialize, Default)]
pub struct WorldSeed(pub u32);

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Default, PartialEq)]
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
            biome_type: BiomeType::Ocean,
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

/// Global trait for all numerical enums serving as unique IDs for certain
/// types of elements in the game. Example : ItemId, BlockId...
/// Used in texture atlases and such
pub trait GameElementId: std::hash::Hash + Eq + PartialEq + Copy + Clone + Default + Debug {}
