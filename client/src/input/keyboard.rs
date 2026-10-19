use crate::state::KeyMap;
use crate::ui::hud::hotbar::HOTBAR_KEYS;
use crate::{constants::BINDS_PATH, input::data::GameAction};
use bevy::prelude::*;
use bevy::{input::ButtonInput, prelude::KeyCode};
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

/// How many keys an action has: a primary one and a secondary one.
pub const MAX_KEYS_PER_ACTION: usize = 2;

/// The bindings of a game folder without a keybindings file.
pub fn default_bindings() -> KeyMap {
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
    map.insert(GameAction::CycleFpsCounter, vec![KeyCode::F8]);
    map.insert(GameAction::ToggleFlyMode, vec![KeyCode::KeyF]);
    map.insert(GameAction::FlyUp, vec![KeyCode::Space]);
    map.insert(GameAction::FlyDown, vec![KeyCode::ShiftLeft]);
    map.insert(GameAction::ToggleInventory, vec![KeyCode::KeyE]);
    map.insert(GameAction::OpenChat, vec![KeyCode::KeyT]);
    map.insert(GameAction::RenderDistanceMinus, vec![KeyCode::KeyO]);
    map.insert(GameAction::RenderDistancePlus, vec![KeyCode::KeyP]);
    map.insert(GameAction::ReloadChunks, vec![KeyCode::KeyR]);
    KeyMap { map }
}

/// The saved bindings over the default ones, action by action. An action keeps its first
/// `MAX_KEYS_PER_ACTION` distinct keys, and `Escape` always pauses.
pub fn get_bindings(game_folder_paths: &GameFolderPaths) -> KeyMap {
    let binds_path: PathBuf = game_folder_paths.game_folder_path.join(BINDS_PATH);

    let mut binds = default_bindings();

    if let Ok(content) = fs::read_to_string(binds_path.as_path()) {
        if let Ok(key_map) = from_str::<KeyMap>(&content) {
            for (action, keys) in key_map.map {
                let mut kept: Vec<KeyCode> = Vec::new();
                for key in keys {
                    if !kept.contains(&key) && kept.len() < MAX_KEYS_PER_ACTION {
                        kept.push(key);
                    }
                }
                binds.map.insert(action, kept);
            }
        }
    }
    binds.map.insert(GameAction::Escape, vec![KeyCode::Escape]);

    binds
}

/// Binds `key` to the slot `slot` (0 for the primary key, 1 for the secondary one) of `action`,
/// in place of the key there, or after the keys of the action when the slot is empty. A key
/// already in the other slot is not repeated.
pub fn set_key(map: &mut KeyMap, action: GameAction, slot: usize, key: KeyCode) {
    let keys = map.map.entry(action).or_default();
    if let Some(bound) = keys.get_mut(slot) {
        *bound = key;
    } else {
        keys.push(key);
    }
    let mut kept = Vec::new();
    keys.retain(|key| {
        let first = !kept.contains(key);
        kept.push(*key);
        first
    });
}

/// Removes the key in the slot `slot` of `action`: clearing the primary key makes the secondary
/// key the primary one.
pub fn clear_key(map: &mut KeyMap, action: GameAction, slot: usize) {
    if let Some(keys) = map.map.get_mut(&action) {
        if slot < keys.len() {
            keys.remove(slot);
        }
    }
}

/// Pairs that share a key on purpose: Space jumps on foot and flies up in fly mode.
pub const SHARED_KEYS: [(GameAction, GameAction); 1] = [(GameAction::Jump, GameAction::FlyUp)];

/// What else a key bound to an action does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyUse {
    /// Another action bound to it.
    Action(GameAction),
    /// Selecting the hotbar slot of this index, hard-wired to the digits: `Digit1` is slot 0.
    HotbarSlot(usize),
}

/// For each action, the keys it shares with other actions or with the hotbar digits, and with
/// which. Escape always pauses: it is the key of the action `Escape`.
pub fn conflicts(map: &KeyMap) -> BTreeMap<GameAction, Vec<(KeyCode, KeyUse)>> {
    let shared_on_purpose = |a: GameAction, b: GameAction| {
        SHARED_KEYS.contains(&(a, b)) || SHARED_KEYS.contains(&(b, a))
    };
    let mut found = BTreeMap::new();
    for (&action, keys) in &map.map {
        let mut shared = Vec::new();
        for &key in keys {
            for (&other, other_keys) in &map.map {
                if other != action && other_keys.contains(&key) && !shared_on_purpose(action, other)
                {
                    shared.push((key, KeyUse::Action(other)));
                }
            }
            if let Some(slot) = HOTBAR_KEYS.iter().position(|&digit| digit == key) {
                shared.push((key, KeyUse::HotbarSlot(slot)));
            }
        }
        if !shared.is_empty() {
            found.insert(action, shared);
        }
    }
    found
}

/// Writes the bindings to `keybindings.ron` in the game folder.
pub fn write_keybindings(key_map: &KeyMap, paths: &GameFolderPaths) {
    let binds_path = paths.game_folder_path.join(BINDS_PATH);

    let pretty_config = PrettyConfig::new()
        .with_depth_limit(3)
        .with_separate_tuple_members(true)
        .with_enumerate_arrays(true);

    if let Ok(serialized) = ron::ser::to_string_pretty(key_map, pretty_config) {
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

        write_keybindings(&key_map, &paths);

        let reloaded = get_bindings(&paths);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(reloaded.map[&GameAction::Jump], vec![KeyCode::KeyJ]);
    }

    fn temp_paths(name: &str) -> GameFolderPaths {
        let dir = std::env::temp_dir().join(format!("rustcraft-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        }
    }

    #[test]
    fn an_old_keybindings_file_still_loads() {
        let paths = temp_paths("old-keybindings");
        // What the game wrote before the settings screen
        let old = "(
    map: {
        MoveForward: [
            /*[0]*/ KeyW,
            /*[1]*/ ArrowUp,
            /*[2]*/ KeyZ,
        ],
        Jump: [
            /*[0]*/ KeyJ,
        ],
        Escape: [
            /*[0]*/ Escape,
        ],
    },
)";
        fs::write(paths.game_folder_path.join(BINDS_PATH), old).unwrap();

        let loaded = get_bindings(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        let mut expected = default_bindings().map;
        expected.insert(GameAction::Jump, vec![KeyCode::KeyJ]);
        expected.insert(
            GameAction::MoveForward,
            vec![KeyCode::KeyW, KeyCode::ArrowUp],
        );
        assert_eq!(loaded.map, expected);
    }

    #[test]
    fn escape_always_pauses() {
        let paths = temp_paths("escape-keybindings");
        fs::write(
            paths.game_folder_path.join(BINDS_PATH),
            "(map: {Escape: [KeyP]})",
        )
        .unwrap();
        let loaded = get_bindings(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(loaded.map[&GameAction::Escape], vec![KeyCode::Escape]);
    }

    #[test]
    fn setting_and_clearing_key_slots() {
        let mut map = KeyMap {
            map: [(GameAction::Jump, vec![KeyCode::Space])].into(),
        };
        let keys = |map: &KeyMap| map.map[&GameAction::Jump].clone();
        set_key(&mut map, GameAction::Jump, 1, KeyCode::KeyJ);
        assert_eq!(keys(&map), [KeyCode::Space, KeyCode::KeyJ]);
        set_key(&mut map, GameAction::Jump, 0, KeyCode::KeyK);
        assert_eq!(keys(&map), [KeyCode::KeyK, KeyCode::KeyJ]);
        // a key already in the other slot is not repeated
        set_key(&mut map, GameAction::Jump, 1, KeyCode::KeyK);
        assert_eq!(keys(&map), [KeyCode::KeyK]);

        // an empty slot is filled by appending
        map.map.insert(GameAction::Jump, vec![]);
        set_key(&mut map, GameAction::Jump, 1, KeyCode::KeyJ);
        assert_eq!(keys(&map), [KeyCode::KeyJ]);

        // clearing the primary key promotes the secondary one
        map.map
            .insert(GameAction::Jump, vec![KeyCode::KeyK, KeyCode::KeyJ]);
        clear_key(&mut map, GameAction::Jump, 0);
        assert_eq!(keys(&map), [KeyCode::KeyJ]);
        clear_key(&mut map, GameAction::Jump, 1);
        assert_eq!(keys(&map), [KeyCode::KeyJ]);
    }

    #[test]
    fn conflicts_are_reported_on_both_actions() {
        let mut map = default_bindings();
        map.map
            .insert(GameAction::Jump, vec![KeyCode::Space, KeyCode::KeyW]);
        let expected = BTreeMap::from([
            (
                GameAction::Jump,
                vec![(KeyCode::KeyW, KeyUse::Action(GameAction::MoveForward))],
            ),
            (
                GameAction::MoveForward,
                vec![(KeyCode::KeyW, KeyUse::Action(GameAction::Jump))],
            ),
        ]);
        assert_eq!(conflicts(&map), expected);
    }

    #[test]
    fn the_keys_wired_in_code_are_conflicts_too() {
        let mut map = default_bindings();
        map.map.insert(
            GameAction::ToggleInventory,
            vec![KeyCode::KeyE, KeyCode::Digit1],
        );
        map.map.insert(GameAction::OpenChat, vec![KeyCode::Digit9]);
        // Escape always pauses
        map.map
            .insert(GameAction::ReloadChunks, vec![KeyCode::Escape]);
        let expected = BTreeMap::from([
            (
                GameAction::Escape,
                vec![(KeyCode::Escape, KeyUse::Action(GameAction::ReloadChunks))],
            ),
            (
                GameAction::ToggleInventory,
                vec![(KeyCode::Digit1, KeyUse::HotbarSlot(0))],
            ),
            (
                GameAction::OpenChat,
                vec![(KeyCode::Digit9, KeyUse::HotbarSlot(8))],
            ),
            (
                GameAction::ReloadChunks,
                vec![(KeyCode::Escape, KeyUse::Action(GameAction::Escape))],
            ),
        ]);
        assert_eq!(conflicts(&map), expected);
    }

    #[test]
    fn jump_and_fly_up_may_share_space() {
        assert!(conflicts(&default_bindings()).is_empty());
    }

    #[test]
    fn an_unreadable_keybindings_file_gives_the_defaults() {
        let paths = temp_paths("garbage-keybindings");
        fs::write(paths.game_folder_path.join(BINDS_PATH), "garbage").unwrap();
        let loaded = get_bindings(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(loaded.map, default_bindings().map);
    }
}
