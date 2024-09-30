use bevy::prelude::ResMut;
use bevy_renet::renet::RenetClient;
use bincode::Options;
use shared::messages::ChatMessage;

pub enum NetworkAction {
    ChatMessage(String),
}

pub fn send_network_action(client: &mut ResMut<RenetClient>, action: NetworkAction) {
    match action {
        NetworkAction::ChatMessage(msg) => {
            let timestamp_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let input_message = bincode::options()
                .serialize(&ChatMessage {
                    author_name: "User".into(),
                    content: msg,
                    date: timestamp_ms,
                })
                .unwrap();

            client.send_message(shared::ClientChannel::ChatMessage, input_message);
        }
    }
}