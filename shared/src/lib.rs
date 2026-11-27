//! What the client and the server have in common: the messages and their encoding, the
//! world containers, and the player simulation both sides run.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use bevy_ecs::resource::Resource;

pub mod channels;
pub mod codec;
pub mod codex;
pub mod commands;
pub mod constants;
pub mod content;
pub mod messages;
pub mod paths;
pub mod players;
pub mod utils;
pub mod world;

pub use channels::*;
pub use codec::*;
pub use constants::*;
pub use paths::*;
pub use rustcraft_protocol::status;

use players::game_mode::GameMode;
use world::hostiles::Difficulty;
use world::WorldGenSettings;

/// The saves of a world its embedded server finished writing, and the ones it failed to write
/// in full, for the client of the same process.
#[derive(Clone, Default, Debug)]
pub struct SaveCounter {
    saved: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
}

impl SaveCounter {
    /// The saves written in full.
    pub fn get(&self) -> u64 {
        self.saved.load(Ordering::Acquire)
    }

    pub fn bump(&self) {
        self.saved.fetch_add(1, Ordering::Release);
    }

    /// The saves of which something could not be written.
    pub fn failures(&self) -> u64 {
        self.failed.load(Ordering::Acquire)
    }

    pub fn bump_failure(&self) {
        self.failed.fetch_add(1, Ordering::Release);
    }
}

#[derive(Resource)]
pub struct GameServerConfig {
    pub world_name: String,
    pub is_solo: bool,
    /// Counts the saves written and failed, for the client that runs this server in its own
    /// process.
    pub saves: Option<SaveCounter>,
    /// The largest render distance a player may ask for, in chunks.
    pub max_render_distance: u32,
    /// The most players connected at once, at most `MAX_PLAYERS`.
    pub max_players: usize,
    /// What a new world is made with: ignored for a world that already exists.
    pub new_world: NewWorldSettings,
    /// Who may join: anyone by the name it gives, or the accounts of a hub. A solo world is
    /// always `Offline`.
    pub auth: ServerAuth,
}

/// How a server knows who its players are.
#[derive(Clone, Debug, Default)]
pub enum ServerAuth {
    /// A player is who it says it is: today's behaviour, for servers among friends.
    #[default]
    Offline,
    /// A player proves its account with a certificate of this hub.
    Hub(HubTrust),
}

/// What a server trusts of its hub: the keys that sign the certificates, and the certificates
/// revoked since, which a thread keeps up to date.
#[derive(Clone, Debug)]
pub struct HubTrust {
    /// The URL of the hub without its trailing `/`, sent to the clients in the challenge.
    pub url: String,
    /// The addresses players reach this server at (`--public-address`), normalized: a proof
    /// made for another address is refused.
    pub addresses: Vec<String>,
    /// The accepted keys of the hub, with their ids.
    pub keys: Vec<(u8, ed25519_dalek::VerifyingKey)>,
    /// The ids of the revoked certificates.
    pub revoked: Arc<RwLock<HashSet<uuid::Uuid>>>,
}

/// What a new world is made with where the player chose it, as the command line of a dedicated
/// server does. What is `None` is left to the game: a random seed, and the default type, mode
/// and difficulty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NewWorldSettings {
    pub seed: Option<u32>,
    pub generation: Option<WorldGenSettings>,
    pub game_mode: Option<GameMode>,
    pub difficulty: Option<Difficulty>,
}

impl NewWorldSettings {
    /// Whether nothing was chosen.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}
