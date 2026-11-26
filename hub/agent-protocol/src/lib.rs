//! What the hub and its realm agent say to each other over the agent's Unix socket: one JSON
//! request per line, answered by one JSON response per line, in order.
//!
//! The agent takes from a request only what varies from one world to the next (which world,
//! its slot, its token and the settings of a new world); the image, the limits, the ports and
//! the security options come from the agent's own configuration.

pub mod allowlist;

use rustcraft_protocol::api::RealmSettings;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use allowlist::{DIFFICULTIES, GAME_MODES, WORLD_TYPES};

/// Checks the settings of a new world against the names the game's server takes, the way the
/// hub checks them when the world is created and the agent before they reach a command line.
pub fn check_settings(settings: &RealmSettings) -> Result<(), String> {
    if settings.seed.is_some_and(|seed| seed > u64::from(u32::MAX)) {
        return Err("the seed is beyond 4294967295".into());
    }
    let allowed = |value: &Option<String>, names: &[&str], what: &str| match value {
        Some(value) if !names.contains(&value.as_str()) => {
            Err(format!("{what} must be one of {}", names.join(", ")))
        }
        _ => Ok(()),
    };
    allowed(&settings.world_type, &WORLD_TYPES, "the world type")?;
    allowed(&settings.game_mode, &GAME_MODES, "the game mode")?;
    allowed(&settings.difficulty, &DIFFICULTIES, "the difficulty")
}

/// A request to the agent, tagged by `request`: `{"request":"stop","realm":"<uuid>"}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// Starts the world in a new container, publishing the ports of `slot`, with `token` as its
    /// realm token; `settings` apply when the world is created.
    Start {
        realm: Uuid,
        slot: u16,
        token: String,
        #[serde(default)]
        settings: RealmSettings,
    },
    /// Stops the world, which saves it, and removes its container; its volume stays.
    Stop { realm: Uuid },
    /// Tells whether the world's container runs.
    Status { realm: Uuid },
    /// Stops the world if it runs and moves its volume aside, to be purged later.
    Archive { realm: Uuid },
    /// Removes the archived worlds older than the agent's configured number of days.
    Purge,
}

/// The agent's answer to a request, tagged by `result`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Response {
    /// The request was carried out (`start`, `stop`, `archive`).
    Done,
    /// The state of a world's container (`status`).
    Status { state: RealmState },
    /// The names of the volumes `purge` removed.
    Purged { volumes: Vec<String> },
    /// The request is not one the agent accepts: malformed, or out of its bounds.
    Refused { reason: String },
    /// The request was valid but Docker failed to carry it out.
    Failed { error: String },
}

/// The state of a world's container, tagged by `state`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum RealmState {
    /// The world has no container: never started, or stopped.
    Absent,
    /// The container runs.
    Running,
    /// The container is there but does not run: the server exited by itself, with this code
    /// when Docker knows it.
    Exited { exit_code: Option<i64> },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const REALM: Uuid = Uuid::from_u128(0x7a);

    #[test]
    fn requests_are_tagged_by_their_name() {
        let start: Request = serde_json::from_value(json!({
            "request": "start",
            "realm": REALM,
            "slot": 3,
            "token": "abc",
            "settings": {"seed": 7, "world_type": "archipelago"},
        }))
        .unwrap();
        assert_eq!(
            start,
            Request::Start {
                realm: REALM,
                slot: 3,
                token: "abc".into(),
                settings: RealmSettings {
                    seed: Some(7),
                    world_type: Some("archipelago".into()),
                    ..Default::default()
                },
            }
        );
        assert_eq!(
            serde_json::to_value(Request::Purge).unwrap(),
            json!({"request": "purge"})
        );
        assert_eq!(
            serde_json::to_value(Request::Stop { realm: REALM }).unwrap(),
            json!({"request": "stop", "realm": "00000000-0000-0000-0000-00000000007a"})
        );
    }

    #[test]
    fn a_request_with_a_bad_id_or_an_unknown_field_is_not_read() {
        let parse = |value| serde_json::from_value::<Request>(value);
        assert!(parse(json!({"request": "stop", "realm": "nonsense"})).is_err());
        assert!(parse(json!({"request": "stop", "realm": REALM, "image": "evil"})).is_err());
        assert!(parse(json!({"request": "run", "realm": REALM})).is_err());
    }

    #[test]
    fn responses_are_tagged_by_their_result() {
        assert_eq!(
            serde_json::to_value(Response::Status {
                state: RealmState::Exited { exit_code: Some(1) }
            })
            .unwrap(),
            json!({"result": "status", "state": {"state": "exited", "exit_code": 1}})
        );
        assert_eq!(
            serde_json::to_value(Response::Done).unwrap(),
            json!({"result": "done"})
        );
    }
}
