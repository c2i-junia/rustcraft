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

/// The groups the actions are listed in on the Controls tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActionCategory {
    Movement,
    Interface,
    View,
    Debug,
}

impl ActionCategory {
    /// Every category, in display order.
    pub const ALL: [ActionCategory; 4] = [
        ActionCategory::Movement,
        ActionCategory::Interface,
        ActionCategory::View,
        ActionCategory::Debug,
    ];

    /// The heading of the category on the Controls tab.
    pub fn label(self) -> &'static str {
        match self {
            ActionCategory::Movement => "Movement",
            ActionCategory::Interface => "Interface",
            ActionCategory::View => "View",
            ActionCategory::Debug => "Debug",
        }
    }
}

impl GameAction {
    /// Every action but Escape, which always pauses and cannot be rebound, in display order.
    pub const LISTED: [GameAction; 18] = [
        GameAction::MoveForward,
        GameAction::MoveBackward,
        GameAction::MoveLeft,
        GameAction::MoveRight,
        GameAction::Jump,
        GameAction::ToggleFlyMode,
        GameAction::FlyUp,
        GameAction::FlyDown,
        GameAction::ToggleInventory,
        GameAction::OpenChat,
        GameAction::ToggleViewMode,
        GameAction::RenderDistanceMinus,
        GameAction::RenderDistancePlus,
        GameAction::ToggleFps,
        GameAction::ToggleChunkDebugMode,
        GameAction::ToggleBlockWireframeDebugMode,
        GameAction::ToggleRaycastDebugMode,
        GameAction::ReloadChunks,
    ];

    /// The name of the action for the player.
    pub fn name(self) -> &'static str {
        match self {
            GameAction::MoveForward => "Move forward",
            GameAction::MoveBackward => "Move backward",
            GameAction::MoveLeft => "Move left",
            GameAction::MoveRight => "Move right",
            GameAction::Jump => "Jump",
            GameAction::Escape => "Pause",
            GameAction::ToggleFlyMode => "Toggle flying",
            GameAction::FlyUp => "Fly up",
            GameAction::FlyDown => "Fly down",
            GameAction::ToggleInventory => "Inventory",
            GameAction::OpenChat => "Chat",
            GameAction::ToggleViewMode => "Camera view",
            GameAction::RenderDistanceMinus => "Lower render distance",
            GameAction::RenderDistancePlus => "Raise render distance",
            GameAction::ToggleFps => "Debug overlay",
            GameAction::ToggleChunkDebugMode => "Chunk borders",
            GameAction::ToggleBlockWireframeDebugMode => "Block wireframe",
            GameAction::ToggleRaycastDebugMode => "Raycast debug",
            GameAction::ReloadChunks => "Reload chunks",
        }
    }

    /// The group the action is listed in.
    pub fn category(self) -> ActionCategory {
        match self {
            GameAction::MoveForward
            | GameAction::MoveBackward
            | GameAction::MoveLeft
            | GameAction::MoveRight
            | GameAction::Jump
            | GameAction::ToggleFlyMode
            | GameAction::FlyUp
            | GameAction::FlyDown => ActionCategory::Movement,
            GameAction::Escape | GameAction::ToggleInventory | GameAction::OpenChat => {
                ActionCategory::Interface
            }
            GameAction::ToggleViewMode
            | GameAction::RenderDistanceMinus
            | GameAction::RenderDistancePlus => ActionCategory::View,
            GameAction::ToggleFps
            | GameAction::ToggleChunkDebugMode
            | GameAction::ToggleBlockWireframeDebugMode
            | GameAction::ToggleRaycastDebugMode
            | GameAction::ReloadChunks => ActionCategory::Debug,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_listed_action_has_a_human_name_and_a_category() {
        let distinct: BTreeSet<_> = GameAction::LISTED.iter().collect();
        assert_eq!(distinct.len(), 18);
        assert!(!GameAction::LISTED.contains(&GameAction::Escape));
        assert_eq!(GameAction::MoveForward.name(), "Move forward");
        assert_eq!(GameAction::ToggleFps.name(), "Debug overlay");
        assert_eq!(GameAction::Jump.category(), ActionCategory::Movement);
        assert_eq!(GameAction::OpenChat.category(), ActionCategory::Interface);
        assert_eq!(GameAction::ToggleViewMode.category(), ActionCategory::View);
        assert_eq!(GameAction::ReloadChunks.category(), ActionCategory::Debug);
    }
}
