//! Types shared by every domain of the client, kept out of `main.rs` so that plugins do not
//! import from the crate root.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::input::data::GameAction;

/// The order of one in-game frame. `game_plugin` chains these sets in `Update`; each domain
/// plugin puts its systems in the set that names their role, so the order lives in one place.
///
/// A system that needs the UI mode but takes part in no other stage only says
/// `.after(GameSet::UiMode)`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameSet {
    /// `set_ui_mode` recomputes `UIMode`, which every reader below sees updated.
    UiMode,
    /// Gathers this frame's inputs (camera, hotbar, view mode) into `CurrentFrameInputs`.
    Input,
    /// Applies the frame's inputs to the local player: block interactions, then movement
    /// (client-side prediction).
    Simulation,
    /// Applies the server's state to the local simulation and corrects it if they differ.
    Reconciliation,
    /// Places the camera on the player, once the player's state is final for the frame.
    Camera,
    /// Remeshes the changed chunks. Runs in `PostUpdate`, so after all of the above by schedule
    /// order alone.
    Meshing,
}

#[derive(Component)]
pub struct MenuCamera;

pub const TEXT_COLOR: Color = Color::srgb(0.9, 0.9, 0.9);

// Enum that will be used as a global state for the game
#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum GameState {
    #[default]
    Menu,
    PreGameLoading,
    Game,
}

#[derive(Resource, Serialize, Deserialize)]
pub struct KeyMap {
    pub map: BTreeMap<GameAction, Vec<KeyCode>>,
}

#[derive(Resource)]
pub struct PreLoadingCompletion {
    pub textures_loaded: bool,
}

#[derive(Resource, Debug)]
pub struct TexturePath {
    pub path: String,
}

#[derive(Resource, Debug)]
pub struct PlayerNameSupplied {
    pub name: String,
}
