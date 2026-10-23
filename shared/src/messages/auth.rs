use serde::{Deserialize, Serialize};

use super::{ClientToServerMessage, PlayerSpawnEvent, ServerToClientMessage};
use crate::world::{BiomeDef, WeatherKind, WorldGenSettings};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AuthRegisterRequest {
    pub username: String,
    /// The render distance the player asks for, in chunks.
    pub render_distance: u32,
}

impl From<AuthRegisterRequest> for ClientToServerMessage {
    fn from(val: AuthRegisterRequest) -> Self {
        ClientToServerMessage::AuthRegisterRequest(val)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct AuthRegisterResponse {
    pub username: String,
    pub session_token: u64,
    pub tick: u64,
    pub timestamp_ms: u64,
    pub players: Vec<PlayerSpawnEvent>, // all players (including the new one)
    /// The largest render distance this server grants, in chunks.
    pub max_render_distance: u32,
    /// The seed of the world, its generation settings and its biomes, in the order of their
    /// ids, from which the client finds the biome of each column: its colours, what falls on
    /// it, and the name the debug overlay shows. The biomes of the mods of the server come with
    /// them, so the client needs none of their files.
    pub seed: u32,
    pub generation: WorldGenSettings,
    pub biomes: Vec<BiomeDef>,
    /// The chat commands the mods of the server add, by name.
    pub mod_commands: Vec<String>,
    /// The weather at the time of the answer, which the client takes at once.
    pub weather: WeatherKind,
}

impl From<AuthRegisterResponse> for ServerToClientMessage {
    fn from(val: AuthRegisterResponse) -> Self {
        ServerToClientMessage::AuthRegisterResponse(val)
    }
}

/// Longest username, in characters, once trimmed.
pub const MAX_USERNAME_LENGTH: usize = 32;

/// The username a server accepts for `requested`: trimmed, 1 to `MAX_USERNAME_LENGTH`
/// characters, no control character. `None` when it must be refused.
pub fn sanitize_username(requested: &str) -> Option<&str> {
    let name = requested.trim();
    let length = name.chars().count();
    let acceptable =
        (1..=MAX_USERNAME_LENGTH).contains(&length) && !name.chars().any(char::is_control);
    acceptable.then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_trimmed_and_checked() {
        assert_eq!(sanitize_username("  Verif "), Some("Verif"));
        assert_eq!(sanitize_username("Player"), Some("Player"));
        assert_eq!(sanitize_username("éé"), Some("éé"));
        assert!(sanitize_username(&"x".repeat(MAX_USERNAME_LENGTH)).is_some());
        for refused in ["", "  \t", "a\nb", "a\0", "a\u{7f}"] {
            assert_eq!(sanitize_username(refused), None, "{refused:?}");
        }
        assert_eq!(
            sanitize_username(&"x".repeat(MAX_USERNAME_LENGTH + 1)),
            None
        );
    }
}
