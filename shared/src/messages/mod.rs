mod auth;
mod chat;
pub mod mob;
pub mod player;
mod world;

pub use auth::*;
use bevy::math::IVec3;
pub use chat::*;
use mob::MobUpdateEvent;
pub use player::*;
use serde::{Deserialize, Serialize};
pub use world::*;

pub type PlayerId = u64;

/// Append only, like `ServerToClientMessage` and `NetworkAction`: a message starts with the
/// index of its variant. Bump `PROTOCOL_ID` with any change.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientToServerMessage {
    AuthRegisterRequest(AuthRegisterRequest),
    ChatMessage(ChatMessageRequest),
    Exit,
    PlayerInputs(Vec<PlayerFrameInput>),
    SaveWorldRequest,
    /// The player changed its render distance, in chunks.
    SetRenderDistance(u32),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerToClientMessage {
    AuthRegisterResponse(AuthRegisterResponse),
    ChatMessage(FullChatMessage),
    WorldUpdate(WorldUpdate),
    BlockUpdates(Vec<BlockUpdate>),
    PlayerSpawn(PlayerSpawnEvent),
    MobUpdate(MobUpdateEvent),
    PlayerUpdate(PlayerUpdateEvent),
    /// The chunks the client must forget: its player went too far from them. The server
    /// forgets it sent them, and sends them again once the player comes back.
    ChunkUnloads(Vec<IVec3>),
    DroppedItems(DroppedItemsUpdate),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::inventory::InventoryAction;
    use crate::world::{MobAction, MobKind, MobTarget};
    use bevy::math::{IVec3, Vec3};

    const ENCODING_WARNING: &str = "the order of the variants is the wire format: reordering \
        them makes a client and a server misread each other. New variants go at the END of the \
        enum; then add them to this table and bump PROTOCOL_ID.";

    /// The index a message starts with on the wire.
    fn variant_index<T: Serialize>(value: &T) -> u8 {
        use bincode::Options;
        bincode::options().serialize(value).unwrap()[0]
    }

    #[test]
    fn client_to_server_variants_are_pinned() {
        let request = AuthRegisterRequest {
            username: String::new(),
            render_distance: 0,
        };
        let chat = ChatMessageRequest {
            content: String::new(),
        };
        for (message, expected) in [
            (ClientToServerMessage::AuthRegisterRequest(request), 0),
            (ClientToServerMessage::ChatMessage(chat), 1),
            (ClientToServerMessage::Exit, 2),
            (ClientToServerMessage::PlayerInputs(vec![]), 3),
            (ClientToServerMessage::SaveWorldRequest, 4),
            (ClientToServerMessage::SetRenderDistance(0), 5),
        ] {
            assert_eq!(variant_index(&message), expected, "{ENCODING_WARNING}");
        }
    }

    #[test]
    fn server_to_client_variants_are_pinned() {
        let response = AuthRegisterResponse {
            username: String::new(),
            session_token: 0,
            tick: 0,
            timestamp_ms: 0,
            players: vec![],
            max_render_distance: 0,
        };
        let chat = FullChatMessage {
            content: String::new(),
            author: String::new(),
            timestamp: 0,
        };
        let spawn = PlayerSpawnEvent {
            id: 0,
            name: String::new(),
            data: PlayerSave::default(),
        };
        let mob = MobUpdateEvent {
            id: 0,
            mob: crate::world::ServerMob {
                kind: MobKind::Fox,
                target: MobTarget::None,
                action: MobAction::Walk,
                position: Vec3::ZERO,
                rotation: Default::default(),
                height: 1.0,
                width: 1.0,
                on_ground: true,
                velocity: Vec3::ZERO,
                depth: 1.0,
            },
        };
        let update = PlayerUpdateEvent {
            id: 0,
            movement: Default::default(),
            orientation: Default::default(),
            last_ack_time: 0,
            inventory: Default::default(),
        };
        for (message, expected) in [
            (ServerToClientMessage::AuthRegisterResponse(response), 0),
            (ServerToClientMessage::ChatMessage(chat), 1),
            (
                ServerToClientMessage::WorldUpdate(WorldUpdate::default()),
                2,
            ),
            (ServerToClientMessage::BlockUpdates(vec![]), 3),
            (ServerToClientMessage::PlayerSpawn(spawn), 4),
            (ServerToClientMessage::MobUpdate(mob), 5),
            (ServerToClientMessage::PlayerUpdate(update), 6),
            (ServerToClientMessage::ChunkUnloads(vec![]), 7),
            (
                ServerToClientMessage::DroppedItems(DroppedItemsUpdate::default()),
                8,
            ),
        ] {
            assert_eq!(variant_index(&message), expected, "{ENCODING_WARNING}");
        }
    }

    #[test]
    fn network_actions_and_mob_enums_are_pinned() {
        for (action, expected) in [
            (NetworkAction::MoveForward, 0),
            (NetworkAction::MoveRight, 1),
            (NetworkAction::MoveBackward, 2),
            (NetworkAction::MoveLeft, 3),
            (NetworkAction::JumpOrFlyUp, 4),
            (NetworkAction::SneakOrFlyDown, 5),
            (NetworkAction::ToggleFlyMode, 6),
            (NetworkAction::LeftClick, 7),
            (NetworkAction::RightClick, 8),
        ] {
            assert_eq!(variant_index(&action), expected, "{ENCODING_WARNING}");
        }
        for (action, expected) in [
            (InventoryAction::LeftClickSlot(0), 0),
            (InventoryAction::RightClickSlot(0), 1),
            (InventoryAction::Close, 2),
            (InventoryAction::TakeCraftResult, 3),
            (InventoryAction::OpenCraftingTable(IVec3::ZERO), 4),
            (InventoryAction::QuickMoveSlot(0), 5),
            (InventoryAction::SwapWithHotbar { slot: 0, hotbar: 0 }, 6),
        ] {
            assert_eq!(variant_index(&action), expected, "{ENCODING_WARNING}");
        }
        assert_eq!(variant_index(&MobKind::Fox), 0, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobTarget::None), 0, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobTarget::Position(Vec3::ZERO)), 1);
        assert_eq!(variant_index(&MobTarget::Player(0)), 2);
        assert_eq!(variant_index(&MobAction::Walk), 0, "{ENCODING_WARNING}");
        assert_eq!(variant_index(&MobAction::Flee), 1, "{ENCODING_WARNING}");
        for (removal, expected) in [
            (ItemRemoval::Gone, 0),
            (ItemRemoval::PickedUp(0), 1),
            (ItemRemoval::Merged(0), 2),
        ] {
            assert_eq!(variant_index(&removal), expected, "{ENCODING_WARNING}");
        }
    }
}
