//! The skin of a player: the 64x64 image its model is cut from. A player wears one of the skins
//! shipped with the game, named, or one it imported, whose pixels travel with it: the server
//! hands every player's skin to the others when they spawn.

use serde::{Deserialize, Serialize};

/// The side of a skin, in pixels.
pub const SKIN_SIDE: u32 = 64;

/// The bytes of the pixels of a skin, four (RGBA) a pixel, row by row from the top.
pub const SKIN_BYTES: usize = (SKIN_SIDE * SKIN_SIDE * 4) as usize;

/// The skins shipped with the game, by name: `data/graphics/skins/<name>.png`, drawn by
/// `scripts/generate_skins.py`. The first is the skin of a player who has not chosen one.
pub const BUILT_IN_SKINS: [&str; 12] = [
    "Wanderer", "Miner", "Gardener", "Sailor", "Scholar", "Ranger", "Smith", "Baker", "Guard",
    "Courier", "Healer", "Fisher",
];

/// The skin of a player, as it travels between the client and the server.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum PlayerSkin {
    /// A skin shipped with the game, by its name in `BUILT_IN_SKINS`.
    BuiltIn(String),
    /// The pixels of a skin the player imported: `SKIN_BYTES` bytes of RGBA, row by row.
    Custom(Vec<u8>),
}

impl Default for PlayerSkin {
    fn default() -> Self {
        PlayerSkin::BuiltIn(BUILT_IN_SKINS[0].to_string())
    }
}

/// The pixels are not printed: they are 16 KiB.
impl std::fmt::Debug for PlayerSkin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlayerSkin::BuiltIn(name) => f.debug_tuple("BuiltIn").field(name).finish(),
            PlayerSkin::Custom(pixels) => write!(f, "Custom({} bytes)", pixels.len()),
        }
    }
}

impl PlayerSkin {
    /// The skin a server keeps for a player who asked for this one: the same, unless it names
    /// no skin of the game or its pixels are not those of a 64x64 image, which give the
    /// default skin.
    pub fn validated(self) -> PlayerSkin {
        let valid = match &self {
            PlayerSkin::BuiltIn(name) => BUILT_IN_SKINS.contains(&name.as_str()),
            PlayerSkin::Custom(pixels) => pixels.len() == SKIN_BYTES,
        };
        if valid {
            self
        } else {
            PlayerSkin::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_skin_of_the_game_or_of_the_right_size_is_kept() {
        let miner = PlayerSkin::BuiltIn("Miner".into());
        assert_eq!(miner.clone().validated(), miner);
        let custom = PlayerSkin::Custom(vec![7; SKIN_BYTES]);
        assert_eq!(custom.clone().validated(), custom);
    }

    #[test]
    fn an_unknown_or_misshapen_skin_becomes_the_default() {
        for refused in [
            PlayerSkin::BuiltIn("Nobody".into()),
            PlayerSkin::BuiltIn("../secret".into()),
            PlayerSkin::Custom(vec![]),
            PlayerSkin::Custom(vec![0; SKIN_BYTES - 1]),
            PlayerSkin::Custom(vec![0; 64 * 32 * 4]),
            PlayerSkin::Custom(vec![0; SKIN_BYTES + 4]),
        ] {
            assert_eq!(refused.validated(), PlayerSkin::default());
        }
        assert_eq!(
            PlayerSkin::default(),
            PlayerSkin::BuiltIn("Wanderer".into())
        );
    }

    #[test]
    fn the_skins_of_the_game_have_their_image() {
        let folder =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/graphics/skins");
        for name in BUILT_IN_SKINS {
            assert!(folder.join(format!("{name}.png")).is_file(), "{name}");
        }
    }

    #[test]
    fn the_skin_variants_are_pinned() {
        use bincode::Options;
        let index = |skin: &PlayerSkin| bincode::options().serialize(skin).unwrap()[0];
        assert_eq!(
            index(&PlayerSkin::default()),
            0,
            "append new variants at the end"
        );
        assert_eq!(
            index(&PlayerSkin::Custom(vec![])),
            1,
            "append new variants at the end"
        );
    }
}
