use crate::i18n::tr;
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
    Sprint,
    Escape,
    ToggleFps,
    CycleFpsCounter,
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
    /// Was `ToggleMinimap`, which showed or hid the minimap: a key saved for it opens the map.
    #[serde(alias = "ToggleMinimap")]
    OpenMap,
    MinimapZoomIn,
    MinimapZoomOut,
    Zoom,
    /// Throws an item out of the inventory, with Ctrl a whole stack.
    Drop,
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
            ActionCategory::Movement => tr("controls.category.movement"),
            ActionCategory::Interface => tr("controls.category.interface"),
            ActionCategory::View => tr("controls.category.view"),
            ActionCategory::Debug => tr("controls.category.debug"),
        }
    }
}

impl GameAction {
    /// Every action but Escape, which always pauses and cannot be rebound, in display order.
    pub const LISTED: [GameAction; 25] = [
        GameAction::MoveForward,
        GameAction::MoveBackward,
        GameAction::MoveLeft,
        GameAction::MoveRight,
        GameAction::Jump,
        GameAction::Sprint,
        GameAction::ToggleFlyMode,
        GameAction::FlyUp,
        GameAction::FlyDown,
        GameAction::ToggleInventory,
        GameAction::Drop,
        GameAction::OpenChat,
        GameAction::CycleFpsCounter,
        GameAction::OpenMap,
        GameAction::MinimapZoomIn,
        GameAction::MinimapZoomOut,
        GameAction::ToggleViewMode,
        GameAction::Zoom,
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
            GameAction::MoveForward => tr("action.forward"),
            GameAction::MoveBackward => tr("action.backward"),
            GameAction::MoveLeft => tr("action.left"),
            GameAction::MoveRight => tr("action.right"),
            GameAction::Jump => tr("action.jump"),
            GameAction::Sprint => tr("action.sprint"),
            GameAction::Escape => tr("action.pause"),
            GameAction::ToggleFlyMode => tr("action.toggle_fly"),
            GameAction::FlyUp => tr("action.fly_up"),
            GameAction::FlyDown => tr("action.fly_down"),
            GameAction::ToggleInventory => tr("action.inventory"),
            GameAction::Drop => tr("action.drop"),
            GameAction::OpenChat => tr("action.chat"),
            GameAction::CycleFpsCounter => tr("action.fps"),
            GameAction::OpenMap => tr("action.map"),
            GameAction::MinimapZoomIn => tr("action.map_zoom_in"),
            GameAction::MinimapZoomOut => tr("action.map_zoom_out"),
            GameAction::ToggleViewMode => tr("action.camera"),
            GameAction::Zoom => tr("action.zoom"),
            GameAction::RenderDistanceMinus => tr("action.render_down"),
            GameAction::RenderDistancePlus => tr("action.render_up"),
            GameAction::ToggleFps => tr("action.debug"),
            GameAction::ToggleChunkDebugMode => tr("action.chunk_borders"),
            GameAction::ToggleBlockWireframeDebugMode => tr("action.wireframe"),
            GameAction::ToggleRaycastDebugMode => tr("action.raycast"),
            GameAction::ReloadChunks => tr("action.reload_chunks"),
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
            | GameAction::Sprint
            | GameAction::ToggleFlyMode
            | GameAction::FlyUp
            | GameAction::FlyDown => ActionCategory::Movement,
            GameAction::Escape
            | GameAction::ToggleInventory
            | GameAction::Drop
            | GameAction::OpenChat
            | GameAction::CycleFpsCounter
            | GameAction::OpenMap
            | GameAction::MinimapZoomIn
            | GameAction::MinimapZoomOut => ActionCategory::Interface,
            GameAction::ToggleViewMode
            | GameAction::Zoom
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
        assert_eq!(distinct.len(), 25);
        assert!(!GameAction::LISTED.contains(&GameAction::Escape));
        assert_eq!(GameAction::MoveForward.name(), "Move forward");
        assert_eq!(GameAction::ToggleFps.name(), "Debug overlay");
        assert_eq!(GameAction::Jump.category(), ActionCategory::Movement);
        assert_eq!(GameAction::OpenChat.category(), ActionCategory::Interface);
        assert_eq!(GameAction::ToggleViewMode.category(), ActionCategory::View);
        assert_eq!(GameAction::ReloadChunks.category(), ActionCategory::Debug);
    }

    #[test]
    fn drop_item_is_listed_under_interface_after_the_inventory() {
        assert_eq!(GameAction::Drop.name(), "Drop item");
        assert_eq!(GameAction::Drop.category(), ActionCategory::Interface);
        let inventory = GameAction::LISTED
            .iter()
            .position(|&action| action == GameAction::ToggleInventory)
            .unwrap();
        assert_eq!(GameAction::LISTED[inventory + 1], GameAction::Drop);
    }

    #[test]
    fn a_key_saved_for_the_former_minimap_toggle_opens_the_map() {
        let action: GameAction = ron::from_str("ToggleMinimap").unwrap();
        assert_eq!(action, GameAction::OpenMap);
        assert_eq!(ron::to_string(&action).unwrap(), "OpenMap");
    }
}
