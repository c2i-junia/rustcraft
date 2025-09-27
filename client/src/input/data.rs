use bevy::prelude::Component;
use serde::{Deserialize, Serialize};

#[derive(
    Eq, Hash, PartialEq, Component, Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, Ord,
)]
pub enum GameAction {
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    Jump,
    Escape,
    ToggleFps,
    ToggleViewMode,
    ToggleChunkDebugMode,
    ToggleFlyMode,
    FlyUp,
    FlyDown,
    ToggleBlockWireframeDebugMode,
    ToggleRaycastDebugMode,
    ToggleInventory,
    OpenChat,
    RenderDistanceMinus,
    RenderDistancePlus,
    ReloadChunks,
}
