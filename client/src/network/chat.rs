use bevy::prelude::*;
use shared::messages::FullChatMessage;

/// Chat messages received from the server and not displayed yet, oldest first.
#[derive(Resource, Default, Debug)]
pub struct CachedChatConversation {
    pub unread: Vec<FullChatMessage>,
}
