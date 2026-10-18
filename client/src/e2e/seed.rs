//! The game folder a test starts the client with.

use std::fs;
use std::path::Path;

use bevy::prelude::{Transform, Vec3};
use shared::players::Player;
use shared::world::{ItemId, ItemStack};
use tempfile::TempDir;

use crate::constants::{SAVE_PATH, SERVER_LIST_SAVE_NAME};
use crate::network::setup::hash_string_to_u64;

/// A temporary game folder, deleted when it is dropped.
pub struct GameFolder {
    dir: TempDir,
}

impl GameFolder {
    pub fn new() -> Self {
        Self {
            dir: TempDir::new().expect("a temporary game folder"),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Adds a saved world, with a fixed seed so that journeys are repeatable.
    pub fn with_world(self, name: &str) -> Self {
        let world = self.path().join(SAVE_PATH).join(name);
        fs::create_dir_all(&world).expect("a world folder");
        let meta = format!("(format_version: 2, name: {name:?}, seed: (42), time: 0, mobs: {{}})");
        fs::write(world.join("world.ron"), meta).expect("a world.ron");
        self
    }

    /// Saves the inventory of `player` in `world`: each item is `(slot, item, count)`.
    pub fn with_items(self, world: &str, player: &str, items: &[(u32, ItemId, u32)]) -> Self {
        let id = hash_string_to_u64(player);
        let mut saved = Player::new(
            id,
            player.to_string(),
            Vec3::new(0., 80., 0.),
            Transform::default(),
        );
        for &(slot, item_id, nb) in items {
            let item_type = item_id.get_default_type();
            saved.inventory.inner.insert(
                slot,
                ItemStack {
                    item_id,
                    item_type,
                    nb,
                },
            );
        }
        let players = self.path().join(SAVE_PATH).join(world).join("players");
        fs::create_dir_all(&players).expect("a players folder");
        let data = ron::ser::to_string(&saved).expect("a serialised player");
        fs::write(players.join(format!("{id}.ron")), data).expect("a player file");
        self
    }

    /// Saves the server list, as `(name, ip)` pairs.
    pub fn with_servers(self, servers: &[(&str, &str)]) -> Self {
        let list: Vec<String> = servers
            .iter()
            .map(|(name, ip)| format!("(name: {name:?}, ip: {ip:?})"))
            .collect();
        let data = format!("[{}]", list.join(", "));
        fs::write(self.path().join(SERVER_LIST_SAVE_NAME), data).expect("a servers.ron");
        self
    }
}
