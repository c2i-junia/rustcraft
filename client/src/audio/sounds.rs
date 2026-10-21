//! The sounds of the game and the files they are played from. The files are made by
//! `scripts/generate_sounds.py`, in `data/sounds/`.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use shared::world::BlockId;

use super::settings::Channel;

/// What a block sounds like when it is walked on, mined, broken or placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundMaterial {
    Grass,
    Dirt,
    Stone,
    Wood,
    Leaves,
    Sand,
    Snow,
    Glass,
}

impl SoundMaterial {
    pub const ALL: [SoundMaterial; 8] = [
        SoundMaterial::Grass,
        SoundMaterial::Dirt,
        SoundMaterial::Stone,
        SoundMaterial::Wood,
        SoundMaterial::Leaves,
        SoundMaterial::Sand,
        SoundMaterial::Snow,
        SoundMaterial::Glass,
    ];

    /// The material of `block`; `None` for water, which makes no block sound.
    pub fn of(block: BlockId) -> Option<Self> {
        Some(match block {
            BlockId::Grass
            | BlockId::TallGrass
            | BlockId::Fern
            | BlockId::Vine
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::Cactus => Self::Grass,
            BlockId::Dirt => Self::Dirt,
            BlockId::Stone
            | BlockId::Cobblestone
            | BlockId::Bedrock
            | BlockId::Debug
            | BlockId::Furnace
            | BlockId::LitFurnace
            | BlockId::CoalOre
            | BlockId::IronOre
            | BlockId::GoldOre
            | BlockId::DiamondOre
            | BlockId::IronBlock
            | BlockId::GoldBlock
            | BlockId::DiamondBlock => Self::Stone,
            BlockId::OakLog
            | BlockId::OakPlanks
            | BlockId::SpruceLog
            | BlockId::SprucePlanks
            | BlockId::JungleLog
            | BlockId::JunglePlanks
            | BlockId::CraftingTable
            | BlockId::Chest
            | BlockId::Torch
            | BlockId::Cocoa => Self::Wood,
            BlockId::OakLeaves | BlockId::SpruceLeaves | BlockId::JungleLeaves => Self::Leaves,
            BlockId::Sand | BlockId::Gravel => Self::Sand,
            BlockId::Snow => Self::Snow,
            BlockId::Glass | BlockId::Ice => Self::Glass,
            BlockId::Water => return None,
        })
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::Grass => "grass",
            Self::Dirt => "dirt",
            Self::Stone => "stone",
            Self::Wood => "wood",
            Self::Leaves => "leaves",
            Self::Sand => "sand",
            Self::Snow => "snow",
            Self::Glass => "glass",
        }
    }

    /// The material whose footsteps are heard on this one: nobody walks on leaves or glass
    /// often enough for them to have their own.
    fn footsteps(self) -> Self {
        match self {
            Self::Leaves => Self::Grass,
            Self::Glass => Self::Stone,
            other => other,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sound {
    Step(SoundMaterial),
    /// One hit of a block being mined.
    Dig(SoundMaterial),
    Break(SoundMaterial),
    Place(SoundMaterial),
    /// The local player lands after a fall.
    Land,
    /// A button of the interface is pressed.
    Click,
    /// The local player picks up an item lying in the world.
    Pickup,
    /// A bucket takes a water source, or pours one.
    Splash,
    /// A stack goes onto the cursor in the inventory.
    ItemPick,
    /// Items leave the cursor into slots of the inventory.
    ItemPlace,
    /// A craft took the result of the crafting grid.
    Craft,
    /// An action the player made was refused.
    Refused,
}

impl Sound {
    pub fn all() -> impl Iterator<Item = Sound> {
        SoundMaterial::ALL
            .into_iter()
            .flat_map(|material| {
                [
                    Sound::Step(material),
                    Sound::Dig(material),
                    Sound::Break(material),
                    Sound::Place(material),
                ]
            })
            .chain([
                Sound::Land,
                Sound::Click,
                Sound::Pickup,
                Sound::Splash,
                Sound::ItemPick,
                Sound::ItemPlace,
                Sound::Craft,
                Sound::Refused,
            ])
    }

    pub fn channel(self) -> Channel {
        match self {
            Sound::Click | Sound::ItemPick | Sound::ItemPlace | Sound::Craft | Sound::Refused => {
                Channel::Interface
            }
            _ => Channel::Effects,
        }
    }

    /// The files of the sound, relative to the assets folder: one per variant, of which one
    /// is picked at random each time it plays.
    pub fn files(self) -> Vec<String> {
        let variants = |kind: &str, material: SoundMaterial, count: u32| -> Vec<String> {
            (1..=count)
                .map(|i| format!("sounds/{kind}/{}{i}.ogg", material.file_name()))
                .collect()
        };
        match self {
            Sound::Step(material) => variants("step", material.footsteps(), 4),
            Sound::Dig(material) => variants("dig", material, 3),
            Sound::Break(material) => variants("break", material, 3),
            Sound::Place(material) => vec![format!("sounds/place/{}.ogg", material.file_name())],
            Sound::Land => vec!["sounds/player/land.ogg".to_string()],
            Sound::Click => vec!["sounds/ui/click.ogg".to_string()],
            // No sound of its own yet: the click is short and soft enough
            Sound::Pickup => vec!["sounds/ui/click.ogg".to_string()],
            Sound::Splash => vec!["sounds/liquid/splash.ogg".to_string()],
            Sound::ItemPick => vec!["sounds/ui/item_pick.ogg".to_string()],
            Sound::ItemPlace => vec!["sounds/ui/item_place.ogg".to_string()],
            Sound::Craft => vec!["sounds/ui/craft.ogg".to_string()],
            Sound::Refused => vec!["sounds/ui/refused.ogg".to_string()],
        }
    }
}

/// The loaded variants of every sound.
#[derive(Resource, Default)]
pub struct SoundLibrary(pub HashMap<Sound, Vec<Handle<AudioSource>>>);

pub fn load_sound_library(mut commands: Commands, asset_server: Res<AssetServer>) {
    let library = Sound::all()
        .map(|sound| {
            let handles = sound
                .files()
                .into_iter()
                .map(|file| asset_server.load(file))
                .collect();
            (sound, handles)
        })
        .collect();
    commands.insert_resource(SoundLibrary(library));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn every_file_of_every_sound_is_in_the_data_folder() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        for sound in Sound::all() {
            for file in sound.files() {
                assert!(data.join(&file).is_file(), "{sound:?}: missing {file}");
            }
        }
    }

    const INVENTORY_SOUNDS: [Sound; 4] = [
        Sound::ItemPick,
        Sound::ItemPlace,
        Sound::Craft,
        Sound::Refused,
    ];

    #[test]
    fn the_inventory_sounds_play_on_the_interface_channel() {
        for sound in INVENTORY_SOUNDS {
            assert!(Sound::all().any(|known| known == sound), "{sound:?}");
            assert_eq!(sound.channel(), Channel::Interface, "{sound:?}");
        }
    }

    /// The length of the file, and its loudest sample from 0 to 1.
    fn length_and_peak(file: &str) -> (std::time::Duration, f32) {
        use bevy::audio::{Decodable, Source};

        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let bytes = std::fs::read(data.join(file)).unwrap();
        let decoder = AudioSource {
            bytes: bytes.into(),
        }
        .decoder();
        let rate = decoder.sample_rate().get() as f32;
        let channels = decoder.channels().get() as f32;
        let samples: Vec<f32> = decoder.collect();
        let length = samples.len() as f32 / channels / rate;
        let peak = samples
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        (std::time::Duration::from_secs_f32(length), peak)
    }

    #[test]
    fn the_inventory_sounds_are_short_and_neither_clipped_nor_silent() {
        for sound in INVENTORY_SOUNDS {
            let longest = match sound {
                Sound::ItemPick | Sound::ItemPlace => 120,
                _ => 300,
            };
            for file in sound.files() {
                let (length, peak) = length_and_peak(&file);
                assert!(
                    length < std::time::Duration::from_millis(longest),
                    "{file}: {length:?}"
                );
                // Under 0 dBFS, over -20 dBFS
                assert!(peak < 1.0 && peak > 0.1, "{file}: peak {peak}");
            }
        }
    }

    #[test]
    fn the_fern_sounds_like_grass() {
        assert_eq!(SoundMaterial::of(BlockId::Fern), Some(SoundMaterial::Grass));
    }

    #[test]
    fn the_vine_sounds_like_grass() {
        assert_eq!(SoundMaterial::of(BlockId::Vine), Some(SoundMaterial::Grass));
    }

    #[test]
    fn the_lit_furnace_sounds_like_stone() {
        assert_eq!(
            SoundMaterial::of(BlockId::LitFurnace),
            Some(SoundMaterial::Stone)
        );
    }

    #[test]
    fn diamond_ore_and_the_storage_blocks_sound_like_stone() {
        // there is no metal sound
        for block in [
            BlockId::DiamondOre,
            BlockId::IronBlock,
            BlockId::GoldBlock,
            BlockId::DiamondBlock,
        ] {
            assert_eq!(
                SoundMaterial::of(block),
                Some(SoundMaterial::Stone),
                "{block:?}"
            );
        }
    }

    #[test]
    fn the_cocoa_sounds_like_wood() {
        assert_eq!(SoundMaterial::of(BlockId::Cocoa), Some(SoundMaterial::Wood));
    }

    #[test]
    fn jungle_wood_sounds_like_wood_and_its_leaves_like_leaves() {
        assert_eq!(
            SoundMaterial::of(BlockId::JungleLog),
            Some(SoundMaterial::Wood)
        );
        assert_eq!(
            SoundMaterial::of(BlockId::JunglePlanks),
            Some(SoundMaterial::Wood)
        );
        assert_eq!(
            SoundMaterial::of(BlockId::JungleLeaves),
            Some(SoundMaterial::Leaves)
        );
    }
}
