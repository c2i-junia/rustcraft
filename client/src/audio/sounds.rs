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
            | BlockId::Dandelion
            | BlockId::Poppy
            | BlockId::Cactus => Self::Grass,
            BlockId::Dirt => Self::Dirt,
            BlockId::Stone
            | BlockId::Cobblestone
            | BlockId::Bedrock
            | BlockId::Debug
            | BlockId::Furnace => Self::Stone,
            BlockId::OakLog
            | BlockId::OakPlanks
            | BlockId::SpruceLog
            | BlockId::SprucePlanks
            | BlockId::CraftingTable => Self::Wood,
            BlockId::OakLeaves | BlockId::SpruceLeaves => Self::Leaves,
            BlockId::Sand => Self::Sand,
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
            .chain([Sound::Land, Sound::Click])
    }

    pub fn channel(self) -> Channel {
        match self {
            Sound::Click => Channel::Interface,
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
}
