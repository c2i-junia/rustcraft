mod auth;
mod chat;
pub mod mob;
pub mod player;
mod world;

pub use auth::*;
pub use chat::*;
use mob::MobUpdateEvent;
pub use player::*;
use serde::{Deserialize, Serialize};
pub use world::*;

pub type PlayerId = u64;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientToServerMessage {
    AuthRegisterRequest(AuthRegisterRequest),
    ChatMessage(ChatMessageRequest),
    Exit,
    PlayerInputs(Vec<PlayerFrameInput>),
    SaveWorldRequest,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerToClientMessage {
    AuthRegisterResponse(AuthRegisterResponse),
    ChatConversation(ChatConversation),
    WorldUpdate(WorldUpdate),
    PlayerSpawn(PlayerSpawnEvent),
    MobUpdate(MobUpdateEvent),
    PlayerUpdate(PlayerUpdateEvent),
}
