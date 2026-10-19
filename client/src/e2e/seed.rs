//! The game folder a test starts the client with.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use bevy::prelude::{Transform, Vec3};
use shared::players::Player;
use shared::world::{ItemId, ItemStack};
use tempfile::TempDir;

use crate::constants::{SAVE_PATH, SERVER_LIST_SAVE_NAME};
use crate::network::hash_string_to_u64;
use crate::network::status::ServerStatus;
use crate::ui::menus::servers::ServerItem;

/// The status of the server at `ip` without the network, as tests ask it: two servers online,
/// one of another version, one offline, one host that does not resolve, one more online for a
/// changed address, and one address that is not one; anything else is offline.
pub fn fixed_status(ip: &str) -> ServerStatus {
    let online = |players, max_players, ms| ServerStatus::Online {
        players,
        max_players,
        latency: Duration::from_millis(ms),
    };
    match ip {
        "10.0.0.1:8000" => online(3, 20, 42),
        "10.0.0.2" => online(18, 20, 180),
        "10.0.0.3:8000" => ServerStatus::Incompatible,
        "nowhere.invalid" => ServerStatus::UnknownHost,
        "10.0.0.9" => online(1, 8, 20),
        "10.0.0.5:0" => ServerStatus::BadAddress,
        _ => ServerStatus::Offline,
    }
}

/// `fixed_status`, answered after 300 ms: long enough to see the servers pinging.
pub fn slow_status(ip: &str) -> ServerStatus {
    thread::sleep(Duration::from_millis(300));
    fixed_status(ip)
}

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

    /// A temporary game folder in `parent`, such as a folder on the disk the game saves to.
    pub fn new_in(parent: &Path) -> Self {
        Self {
            dir: Arc::new(TempDir::new_in(parent).expect("a temporary game folder")),
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
        self.with_seeded_world(name, 42, time, played_ago)
    }

    /// Adds a saved world like `with_played_world`, of the seed `seed`.
    pub fn with_seeded_world(self, name: &str, seed: u32, time: u64, played_ago: Duration) -> Self {
        let world = self.path().join(SAVE_PATH).join(name);
        fs::create_dir_all(&world).expect("a world folder");
        let meta = format!(
            "(format_version: 2, name: {name:?}, seed: ({seed}), time: {time}, mobs: {{}})"
        );
        let path = world.join("world.ron");
        fs::write(&path, meta).expect("a world.ron");
        fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|file| file.set_modified(SystemTime::now() - played_ago))
            .expect("the date of a world.ron");
        self
    }

    /// Adds a copy of the world folder `source` as the world `name`.
    pub fn with_world_copy(self, name: &str, source: &Path) -> Self {
        fn copy(from: &Path, to: &Path) {
            fs::create_dir_all(to).expect("a world folder");
            for entry in fs::read_dir(from).expect("a world to copy") {
                let entry = entry.expect("an entry of a world");
                let target = to.join(entry.file_name());
                if entry.file_type().expect("a file type").is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    fs::copy(entry.path(), target).expect("a file of a world");
                }
            }
        }
        copy(source, &self.path().join(SAVE_PATH).join(name));
        self
    }

    /// Adds a world whose `world.ron` cannot be read: the server stops on it.
    pub fn with_broken_world(self, name: &str) -> Self {
        let world = self.path().join(SAVE_PATH).join(name);
        fs::create_dir_all(&world).expect("a world folder");
        fs::write(world.join("world.ron"), "not ron").expect("a world.ron");
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

    /// Saves the servers that `fixed_status` knows: `Survival` and `Creative` online, `Old
    /// friends` of another version, `Friends` offline and `Typo`, whose host does not resolve.
    pub fn with_status_servers(self) -> Self {
        self.with_servers(&[
            ("Survival", "10.0.0.1:8000"),
            ("Creative", "10.0.0.2"),
            ("Old friends", "10.0.0.3:8000"),
            ("Friends", "10.0.0.4:8000"),
            ("Typo", "nowhere.invalid"),
        ])
    }
}
