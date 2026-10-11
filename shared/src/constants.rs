use bevy::prelude::*;

/// Bump it whenever a network message or an enum sent in one (`BlockId`, `BlockDirection`,
/// `ItemId`, ...) changes, so that a client and a server of different builds refuse to talk
/// instead of misreading each other.
pub const PROTOCOL_ID: u64 = 1;
pub const TICKS_PER_SECOND: u64 = 20;
pub const CHUNK_SIZE: i32 = 16;
pub const MAX_INVENTORY_SLOTS: u32 = 4 * 9;
pub const HALF_BLOCK: Vec3 = Vec3 {
    x: 0.5,
    y: 0.5,
    z: 0.5,
};

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

/// Most inputs the server accepts from one client in one tick; the excess is dropped.
///
/// A client records one input per rendered frame and uploads them at the tick rate (20 Hz), so
/// a message holds the frames of 50 ms: 3 at 60 fps, 12 at 240 fps, 50 at 1000 fps. The
/// messages a stalled server received meanwhile are read together in its next tick: 512 covers
/// a two second stall of a 240 fps client, and bounds the work one client can ask per tick to
/// what a 10000 fps client would.
pub const MAX_INPUTS_PER_TICK: usize = 512;

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
