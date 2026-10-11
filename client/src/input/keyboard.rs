use crate::state::KeyMap;
use crate::{constants::BINDS_PATH, input::data::GameAction};
use bevy::prelude::*;
use bevy::{
    input::ButtonInput,
    prelude::{KeyCode, Res},
};
use ron::{from_str, ser::PrettyConfig};
use shared::GameFolderPaths;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Write,
    path::PathBuf,
};

pub fn is_action_pressed(
    action: GameAction,
    keyboard_input: &ButtonInput<KeyCode>,
    key_map: &KeyMap,
) -> bool {
    if let Some(key_codes) = key_map.map.get(&action) {
        for key_code in key_codes {
            if keyboard_input.pressed(*key_code) {
                return true;
            }
        }
    }
    false
}

pub fn is_action_just_pressed(
    action: GameAction,
    keyboard_input: &ButtonInput<KeyCode>,
    key_map: &KeyMap,
) -> bool {
    if let Some(key_codes) = key_map.map.get(&action) {
        for key_code in key_codes {
            if keyboard_input.just_pressed(*key_code) {
                return true;
            }
        }
    }
    false
}

pub fn is_action_just_released(
    action: GameAction,
    keyboard_input: &ButtonInput<KeyCode>,
    key_map: &KeyMap,
) -> bool {
    if let Some(key_codes) = key_map.map.get(&action) {
        for key_code in key_codes {
            if keyboard_input.just_released(*key_code) {
                return true;
            }
        }
    }
    false
}

pub fn get_action_keys(action: GameAction, key_map: &KeyMap) -> Vec<KeyCode> {
    key_map.map.get(&action).unwrap().to_vec()
}

pub fn get_bindings(game_folder_paths: &GameFolderPaths) -> KeyMap {
    let binds_path: PathBuf = game_folder_paths.game_folder_path.join(BINDS_PATH);

    let mut binds = KeyMap {
        map: {
            let mut map = BTreeMap::new();
            map.insert(
                GameAction::MoveForward,
                vec![KeyCode::KeyW, KeyCode::ArrowUp],
            );
            map.insert(
                GameAction::MoveBackward,
                vec![KeyCode::KeyS, KeyCode::ArrowDown],
            );
            map.insert(
                GameAction::MoveLeft,
                vec![KeyCode::KeyA, KeyCode::ArrowLeft],
            );
            map.insert(
                GameAction::MoveRight,
                vec![KeyCode::KeyD, KeyCode::ArrowRight],
            );
            map.insert(GameAction::Jump, vec![KeyCode::Space]);
            map.insert(GameAction::Escape, vec![KeyCode::Escape]);
            map.insert(GameAction::ToggleFps, vec![KeyCode::F3]);
            map.insert(GameAction::ToggleChunkDebugMode, vec![KeyCode::F4]);
            map.insert(GameAction::ToggleViewMode, vec![KeyCode::F5]);
            map.insert(GameAction::ToggleBlockWireframeDebugMode, vec![KeyCode::F6]);
            map.insert(GameAction::ToggleRaycastDebugMode, vec![KeyCode::F7]);
            map.insert(GameAction::ToggleFlyMode, vec![KeyCode::KeyF]);
            map.insert(GameAction::FlyUp, vec![KeyCode::Space]);
            map.insert(GameAction::FlyDown, vec![KeyCode::ShiftLeft]);
            map.insert(GameAction::ToggleInventory, vec![KeyCode::KeyE]);
            map.insert(GameAction::OpenChat, vec![KeyCode::KeyT]);
            map.insert(GameAction::RenderDistanceMinus, vec![KeyCode::KeyO]);
            map.insert(GameAction::RenderDistancePlus, vec![KeyCode::KeyP]);
            map.insert(GameAction::ReloadChunks, vec![KeyCode::KeyR]);
            map
        },
    };

    // Try to get & serialize existing binds
    if let Ok(content) = fs::read_to_string(binds_path.as_path()) {
        if let Ok(key_map) = from_str::<KeyMap>(&content) {
            for (k, v) in key_map.map.iter() {
                binds.map.insert(*k, v.clone());
            }
        }
    }

    binds

    // Get default binds
}

pub fn save_keybindings(key_map: Res<KeyMap>, game_folder_path: Res<GameFolderPaths>) {
    let binds_path = game_folder_path.game_folder_path.join(BINDS_PATH);

    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    if let Ok(serialized) = ron::ser::to_string_pretty(key_map.into_inner(), pretty_config) {
        match File::create(&binds_path) {
            Ok(mut file) => {
                if let Err(e) = file.write_all(serialized.as_bytes()) {
                    error!("Error while saving keybindings to {:?}: {}", binds_path, e);
                } else {
                    info!("Keybindings successfully saved to {:?}", binds_path);
                }
            }
            Err(e) => error!(
                "Failed to create keybindings file at {:?}: {}",
                binds_path, e
            ),
        }
    } else {
        error!("Failed to serialize keybindings");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_bindings_are_loaded_back_from_the_game_folder() {
        let dir =
            std::env::temp_dir().join(format!("rustcraft-keybindings-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.clone(),
            // Nothing is saved there: bindings must not be read from it
            assets_folder_path: dir.join("assets"),
        };

        let mut key_map = get_bindings(&paths);
        key_map.map.insert(GameAction::Jump, vec![KeyCode::KeyJ]);

        let mut app = App::new();
        app.insert_resource(key_map)
            .insert_resource(paths.clone())
            .add_systems(Update, save_keybindings);
        app.update();

        let reloaded = get_bindings(&paths);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(reloaded.map[&GameAction::Jump], vec![KeyCode::KeyJ]);
    }
}
