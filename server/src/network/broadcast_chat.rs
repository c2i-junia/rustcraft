use bevy::prelude::*;
use shared::messages::ChatConversation;

#[derive(Event)]
pub struct ChatMessageEvent;

pub fn setup_chat_resources(app: &mut App) {
    app.insert_resource(ChatConversation { ..default() });
    app.add_event::<ChatMessageEvent>();
}
