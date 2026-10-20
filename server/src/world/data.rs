use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use bevy::math::{IVec3, Vec3};
use bevy_ecs::resource::Resource;

use super::dropped_items::DroppedItems;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{ChunkMap, DroppedItemId, MobId, ServerMob},
    GameFolderPaths, MIN_RENDER_DISTANCE,
};

pub const SAVE_PATH: &str = "saves/";

/// Everything the server knows about the world it runs.
#[derive(Resource, Default)]
pub struct ServerWorldMap {
    pub name: String,
    pub chunks: ChunkMap,
    /// For each connected client, what it has received of the world.
    pub sent_chunks: HashMap<PlayerId, SentChunks>,
    pub render_distances: RenderDistances,
    pub players: HashMap<PlayerId, Player>,
    pub mobs: HashMap<MobId, ServerMob>,
    /// The mobs that appeared, moved or changed what they do since the last broadcast.
    pub changed_mobs: HashSet<MobId>,
    /// For each connected client, the mobs it has been sent and holds.
    pub sent_mobs: HashMap<PlayerId, HashSet<MobId>>,
    pub dropped_items: DroppedItems,
    /// For each connected client, the dropped items it has been told of.
    pub sent_items: HashMap<PlayerId, HashSet<DroppedItemId>>,
    /// The chunks that are new or changed since they were last handed to the writer.
    pub dirty_chunks: HashSet<IVec3>,
    /// The files of the players that the next save hands to the writer, as they read.
    pub players_to_save: Vec<(PlayerId, String)>,
    /// The folder of the world, whose files hold the saved chunks not loaded yet: a chunk is
    /// read from there when a player first needs it. `None` generates every chunk.
    pub saved: Option<PathBuf>,
    /// Where a new player appears, and a dead one comes back (`generation::spawn_position`).
    pub spawn: Vec3,
}

/// What one client has received of the world.
#[derive(Default, Debug)]
pub struct SentChunks {
    /// The chunks it has received.
    pub chunks: HashSet<IVec3>,
    /// The chunk of its player and the render distance, with how many of the chunks within
    /// that distance, nearest first (`render_distance_offsets`), it holds all of: until either
    /// changes, those are not looked at again. A client is sent every chunk within reach that
    /// the server holds, so it lacks nothing once that is all of them.
    pub progress: Option<((IVec3, i32), usize)>,
}

/// The render distance of each authenticated player, in chunks, as the server grants it.
#[derive(Default, Debug)]
pub struct RenderDistances(HashMap<PlayerId, i32>);

impl RenderDistances {
    /// The render distance of `player`: `MIN_RENDER_DISTANCE` until it asked for one.
    pub fn get(&self, player: PlayerId) -> i32 {
        self.0
            .get(&player)
            .copied()
            .unwrap_or(MIN_RENDER_DISTANCE as i32)
    }

    /// Grants `player` the distance it asked for, kept between `MIN_RENDER_DISTANCE` and `max`.
    pub fn set(&mut self, player: PlayerId, asked: u32, max: u32) {
        let granted = asked.min(max).max(MIN_RENDER_DISTANCE);
        self.0.insert(player, granted as i32);
    }

    pub fn remove(&mut self, player: PlayerId) {
        self.0.remove(&player);
    }
}

/// The folder of the world `world_name`. The name must be a single plain path component, so
/// that no path is ever built from a name that could point outside the saves folder.
pub fn world_dir(game_folder_paths: &GameFolderPaths, world_name: &str) -> Result<PathBuf, String> {
    let mut components = Path::new(world_name).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(component)), None) if component == world_name => {}
        _ => return Err(format!("invalid world name {world_name:?}")),
    }
    Ok(game_folder_paths
        .game_folder_path
        .join(SAVE_PATH)
        .join(world_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_distance_is_kept_within_the_bounds_and_defaults_to_the_least() {
        let mut distances = RenderDistances::default();
        assert_eq!(distances.get(1), MIN_RENDER_DISTANCE as i32);

        distances.set(1, 4, 6);
        distances.set(2, 40, 6);
        distances.set(3, 0, 6);
        assert_eq!(distances.get(1), 4);
        assert_eq!(distances.get(2), 6);
        assert_eq!(distances.get(3), MIN_RENDER_DISTANCE as i32);
        // a server whose maximum is below the least still grants the least
        distances.set(4, 3, 0);
        assert_eq!(distances.get(4), MIN_RENDER_DISTANCE as i32);

        distances.remove(1);
        assert_eq!(distances.get(1), MIN_RENDER_DISTANCE as i32);
    }

    #[test]
    fn world_names_that_are_not_a_single_component_are_refused() {
        let paths = GameFolderPaths {
            game_folder_path: "game".into(),
            assets_folder_path: "assets".into(),
        };
        assert_eq!(
            world_dir(&paths, "my world").unwrap(),
            Path::new("game/saves/my world")
        );
        for name in ["", ".", "..", "../x", "a/b", "/abs", "a/", "a/.."] {
            assert!(world_dir(&paths, name).is_err(), "{name:?}");
        }
    }
}
