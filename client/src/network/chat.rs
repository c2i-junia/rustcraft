use bevy::prelude::*;
use shared::messages::ChatConversation;

#[derive(Resource, Default, Debug)]
pub struct CachedChatConversation {
    pub last_update: u64,
    pub data: Option<ChatConversation>,
}

pub fn update_cached_chat_state(
    chat_state: &mut ResMut<CachedChatConversation>,
    new_state: ChatConversation,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    chat_state.last_update = now;
    chat_state.data = Some(new_state);

    trace!("new CachedChatConversation: {:?}", &chat_state);
}
