//! The game folder a test starts the client with.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use bevy::prelude::{Transform, Vec3};
use shared::players::Player;
use shared::world::{ItemId, ItemStack};
use tempfile::TempDir;

use crate::constants::{SAVE_PATH, SERVER_LIST_SAVE_NAME};
use crate::network::hash_string_to_u64;
use crate::ui::menus::multi::ServerItem;

/// A temporary game folder, deleted when its last clone is dropped.
#[derive(Clone)]
pub struct GameFolder {
    dir: Arc<TempDir>,
}

impl GameFolder {
    pub fn new() -> Self {
        Self {
            dir: Arc::new(TempDir::new().expect("a temporary game folder")),
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Adds a file to the game folder, such as a settings file.
    pub fn with_file(self, name: &str, contents: &str) -> Self {
        fs::write(self.path().join(name), contents).expect("a file in the game folder");
        self
    }

    /// Adds a saved world, with a fixed seed so that journeys are repeatable.
    pub fn with_world(self, name: &str) -> Self {
        self.with_played_world(name, 0, Duration::ZERO)
    }

    /// Adds a saved world like `with_world`, at the game time `time`, last played `played_ago`:
    /// its `world.ron` was last modified then.
    pub fn with_played_world(self, name: &str, time: u64, played_ago: Duration) -> Self {
        let world = self.path().join(SAVE_PATH).join(name);
        fs::create_dir_all(&world).expect("a world folder");
        let meta =
            format!("(format_version: 2, name: {name:?}, seed: (42), time: {time}, mobs: {{}})");
        let path = world.join("world.ron");
        fs::write(&path, meta).expect("a world.ron");
        fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(SystemTime::now() - played_ago))
            .expect("the date of a world.ron");
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
        let list: Vec<ServerItem> = servers
            .iter()
            .map(|(name, ip)| ServerItem {
                name: name.to_string(),
                ip: ip.to_string(),
            })
            .collect();
        let data = ron::ser::to_string(&list).expect("a serialised server list");
        fs::write(self.path().join(SERVER_LIST_SAVE_NAME), data).expect("a servers.ron");
        self
    }
}
