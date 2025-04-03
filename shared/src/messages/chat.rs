use bevy::prelude::Resource;
use serde::{Deserialize, Serialize};

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
