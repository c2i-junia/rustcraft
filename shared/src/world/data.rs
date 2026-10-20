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
    /// Hot, drier than a forest and wetter than a desert: grass with a few trees.
    Savanna,
    /// Hot and wet, along the warm coasts: the wet half of the hot forests.
    Jungle,
}

#[derive(Debug, Clone, Copy)]
pub struct Biome {
    pub biome_type: BiomeType,
    pub surface_block: BlockId,
    pub sub_surface_block: BlockId,
}

pub fn get_biome_data(biome_type: BiomeType) -> Biome {
    match biome_type {
        BiomeType::Plains => Biome {
            biome_type: BiomeType::Plains,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Forest => Biome {
            biome_type: BiomeType::Forest,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::MediumMountain => Biome {
            biome_type: BiomeType::MediumMountain,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::HighMountainGrass => Biome {
            biome_type: BiomeType::HighMountainGrass,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Desert => Biome {
            biome_type: BiomeType::Desert,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::IcePlain => Biome {
            biome_type: BiomeType::IcePlain,
            surface_block: BlockId::Snow,
            sub_surface_block: BlockId::Ice,
        },
        BiomeType::FlowerPlains => Biome {
            biome_type: BiomeType::FlowerPlains,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::ShallowOcean => Biome {
            biome_type: BiomeType::ShallowOcean,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::Ocean => Biome {
            biome_type: BiomeType::Ocean,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::DeepOcean => Biome {
            biome_type: BiomeType::DeepOcean,
            surface_block: BlockId::Sand,
            sub_surface_block: BlockId::Sand,
        },
        BiomeType::Savanna => Biome {
            biome_type: BiomeType::Savanna,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
        BiomeType::Jungle => Biome {
            biome_type: BiomeType::Jungle,
            surface_block: BlockId::Grass,
            sub_surface_block: BlockId::Dirt,
        },
    }
}

/// Global trait for all numerical enums serving as unique IDs for certain
/// types of elements in the game. Example : ItemId, BlockId...
/// Used in texture atlases and such
pub trait GameElementId: std::hash::Hash + Eq + PartialEq + Copy + Clone + Default + Debug {}
