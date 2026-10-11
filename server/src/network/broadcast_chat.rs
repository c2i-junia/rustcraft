use bevy::prelude::*;
use shared::messages::{ChatConversation, FullChatMessage, CHAT_MAX_MESSAGE_LENGTH};

/// Number of messages kept by the server and replayed to newly authenticated clients.
pub const CHAT_MAX_HISTORY: usize = 100;

pub fn setup_chat_resources(app: &mut App) {
    app.insert_resource(ChatConversation { ..default() });
}

/// Stores a message from `author` in `history` and returns it for broadcasting, or `None` when
/// the message is empty (or only whitespace) or longer than `CHAT_MAX_MESSAGE_LENGTH`.
pub fn record_chat_message(
    history: &mut ChatConversation,
    author: &str,
    content: String,
    timestamp: u64,
) -> Option<FullChatMessage> {
    if content.trim().is_empty() || content.chars().count() > CHAT_MAX_MESSAGE_LENGTH {
        return None;
    }
    let message = FullChatMessage {
        author: author.to_string(),
        content,
        timestamp,
    };
    history.messages.push(message.clone());
    if history.messages.len() > CHAT_MAX_HISTORY {
        history.messages.remove(0);
    }
    Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(history: &mut ChatConversation, content: &str) -> Option<FullChatMessage> {
        record_chat_message(history, "steve", content.to_string(), 7)
    }

    #[test]
    fn accepted_message_is_stored_and_returned() {
        let mut history = ChatConversation::default();
        let message = record(&mut history, "hello").unwrap();
        assert_eq!(message.author, "steve");
        assert_eq!(message.content, "hello");
        assert_eq!(message.timestamp, 7);
        assert_eq!(history.messages, vec![message]);
    }

    #[test]
    fn empty_and_blank_messages_are_rejected() {
        let mut history = ChatConversation::default();
        assert_eq!(record(&mut history, ""), None);
        assert_eq!(record(&mut history, "  \t"), None);
        assert!(history.messages.is_empty());
    }

    #[test]
    fn length_limit_counts_characters() {
        let mut history = ChatConversation::default();
        assert!(record(&mut history, &"é".repeat(CHAT_MAX_MESSAGE_LENGTH)).is_some());
        assert_eq!(
            record(&mut history, &"é".repeat(CHAT_MAX_MESSAGE_LENGTH + 1)),
            None
        );
        assert_eq!(history.messages.len(), 1);
    }

    #[test]
    fn history_keeps_only_the_latest_messages() {
        let mut history = ChatConversation::default();
        for i in 0..CHAT_MAX_HISTORY + 5 {
            record(&mut history, &i.to_string());
        }
        assert_eq!(history.messages.len(), CHAT_MAX_HISTORY);
        assert_eq!(history.messages[0].content, "5");
        assert_eq!(
            history.messages.last().unwrap().content,
            (CHAT_MAX_HISTORY + 4).to_string()
        );
    }
}
