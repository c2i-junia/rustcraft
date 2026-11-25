//! What the game and the hub share, with no Bevy: the identity certificates, the JSON types of
//! the hub API and the status packet codec.

pub mod api;
pub mod identity;
pub mod status;

/// Bump it whenever a network message or an enum sent in one (`BlockId`, `BlockDirection`,
/// `ItemId`, ...) changes, so that a client and a server of different builds refuse to talk
/// instead of misreading each other. The hub announces it in its well-known document.
pub const PROTOCOL_ID: u64 = 50;

/// The hub the game talks to unless `--hub` says otherwise.
pub const DEFAULT_HUB: &str = "https://hub.rustcraft.dev";

/// The header carrying a hosted world's token to the hub: `Authorization: Realm <token>`.
pub const REALM_AUTH_SCHEME: &str = "Realm";
