use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

/// Longest chat message the server accepts, in characters.
pub const CHAT_MAX_MESSAGE_LENGTH: usize = 256;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ChatMessageRequest {
    pub content: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct FullChatMessage {
    pub content: String,
    pub author: String,
    pub timestamp: u64,
}

#[derive(Resource, Default, Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct ChatConversation {
    pub messages: Vec<FullChatMessage>,
}
