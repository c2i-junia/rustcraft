use crate::{constants::BINDS_PATH, input::data::GameAction, KeyMap};
use bevy::prelude::*;
use bevy::{
    input::ButtonInput,
    prelude::{KeyCode, Res},
};
use ron::{from_str, ser::PrettyConfig};
use shared::world::get_game_folder;
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

pub fn get_bindings(game_folder_path: &String) -> KeyMap {
    let game_folder_path_struct = GameFolderPaths {
        game_folder_path: game_folder_path.clone(),
        assets_folder_path: format!("{}/data", game_folder_path),
    };
    let binds_path: PathBuf = get_game_folder(Some(&game_folder_path_struct)).join(BINDS_PATH);

    // Try to get & serialize existing binds
    if let Ok(content) = fs::read_to_string(binds_path.as_path()) {
        if let Ok(key_map) = from_str::<KeyMap>(&content) {
            return key_map;
        }
    }

    // If binds cannot be loaded, get default ones
    KeyMap {
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
            map.insert(GameAction::DebugGetBlock, vec![KeyCode::KeyI]);
            map
        },
    }
}

pub fn save_keybindings(key_map: Res<KeyMap>, game_folder_path: Res<GameFolderPaths>) {
    let binds_path: PathBuf = get_game_folder(Some(&game_folder_path)).join(BINDS_PATH);

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
