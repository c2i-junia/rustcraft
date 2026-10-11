use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use bevy::math::IVec3;
use bevy_ecs::resource::Resource;
use shared::{
    messages::PlayerId,
    players::Player,
    world::{ChunkMap, MobId, ServerMob},
    GameFolderPaths,
};

pub const SAVE_PATH: &str = "saves/";

/// Everything the server knows about the world it runs.
#[derive(Resource, Default)]
pub struct ServerWorldMap {
    pub name: String,
    pub chunks: ChunkMap,
    /// For each connected client, the chunks it has received.
    pub sent_chunks: HashMap<PlayerId, HashSet<IVec3>>,
    pub players: HashMap<PlayerId, Player>,
    pub mobs: HashMap<MobId, ServerMob>,
    /// The chunks that are new or changed since they were last handed to the writer.
    pub dirty_chunks: HashSet<IVec3>,
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
