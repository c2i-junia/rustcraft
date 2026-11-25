//! What the game and the hub share, with no Bevy: the identity certificates, the JSON types of
//! the hub API and the status packet codec.

pub mod api;
pub mod identity;
pub mod status;

/// The hub the game talks to unless `--hub` says otherwise.
pub const DEFAULT_HUB: &str = "https://hub.rustcraft.dev";

/// The header carrying a hosted world's token to the hub: `Authorization: Realm <token>`.
pub const REALM_AUTH_SCHEME: &str = "Realm";
