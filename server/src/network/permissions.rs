//! Who may do what on a server: its operators, who alone run the commands that change the game
//! (`commands::refusal`), the players it bans, and its optional whitelist.
//!
//! A dedicated server keeps each list in a file of its game folder, next to `saves/`: read when
//! it starts, written whenever a command changes it. A solo world keeps none: its player is
//! always an operator, and a list it changes lasts as long as its server.
//!
//! Names are compared without their case, as players type them in commands: `/ban Steve` bans
//! `steve`. A list keeps them as they were typed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use bevy_renet::RenetServer;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use shared::messages::ServerToClientMessage;

use crate::network::extensions::SendGameMessageExtension;
use crate::world::format::write_atomically;

/// The operators of a dedicated server: a list of names.
pub const OPS_FILE: &str = "ops.ron";
/// The players a dedicated server bans: their names, each with the reason given, if any.
pub const BANNED_FILE: &str = "banned.ron";
/// The whitelist of a dedicated server, and whether it is on.
pub const WHITELIST_FILE: &str = "whitelist.ron";

/// How long a dismissed client stays connected after its `Dismissed` message, so that the
/// message reaches it before the connection closes: the client ends its session on the message
/// itself, and this is only for a client that does not.
pub const DISMISS_DELAY: Duration = Duration::from_millis(500);

/// The whitelist: when it is on, only the players on it, and the operators, may join.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Whitelist {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub players: Vec<String>,
}

/// What `/whitelist` does.
#[derive(Debug, Clone, PartialEq)]
pub enum WhitelistAction {
    On,
    Off,
    Add(String),
    Remove(String),
    List,
}

/// A command that changes who may do what, or who is connected.
#[derive(Debug, Clone, PartialEq)]
pub enum Admin {
    /// `/op <player>`
    Op(String),
    /// `/deop <player>`
    Deop(String),
    /// `/kick <player> [reason]`
    Kick {
        name: String,
        reason: Option<String>,
    },
    /// `/ban <player> [reason]`: kicked too when connected.
    Ban {
        name: String,
        reason: Option<String>,
    },
    /// `/pardon <player>`
    Pardon(String),
    /// `/whitelist on|off|add|remove|list`
    Whitelist(WhitelistAction),
}

/// What an `Admin` command did: the answer to its author, and the player to dismiss, by name,
/// with the message the player is shown.
#[derive(Debug, PartialEq)]
pub struct AdminOutcome {
    pub answer: String,
    pub dismiss: Option<(String, String)>,
}

/// The lists of a server, and the folder they are saved in (`None` for a solo world).
#[derive(Resource, Debug, Default)]
pub struct Permissions {
    folder: Option<PathBuf>,
    ops: Vec<String>,
    /// The banned names, each with its reason.
    banned: BTreeMap<String, Option<String>>,
    whitelist: Whitelist,
}

/// Whether `a` and `b` are the same name, whatever their case.
pub fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// The message a player is dismissed with: `what`, then the reason when there is one.
fn with_reason(what: &str, reason: &Option<String>) -> String {
    match reason {
        Some(reason) => format!("{what}: {reason}"),
        None => format!("{what}."),
    }
}

/// Reads the RON file `name` of `folder`: the default value when it does not exist.
fn read_list<T: DeserializeOwned + Default>(folder: &Path, name: &str) -> Result<T, String> {
    let path = folder.join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            ron::from_str(&text).map_err(|err| format!("Could not read {}: {err}", path.display()))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(format!("Could not read {}: {err}", path.display())),
    }
}

fn write_list<T: Serialize>(folder: &Path, name: &str, value: &T) -> Result<(), String> {
    let path = folder.join(name);
    let text = ron::ser::to_string_pretty(value, ron::ser::PrettyConfig::default())
        .map_err(|err| format!("Could not write {}: {err}", path.display()))?;
    std::fs::create_dir_all(folder)
        .and_then(|()| write_atomically(&path, text.as_bytes()))
        .map_err(|err| format!("Could not write {}: {err}", path.display()))
}

impl Permissions {
    /// The lists saved in `folder`, empty where it has no file. A file that cannot be read is
    /// an error: the server does not start rather than forget its bans and overwrite them.
    pub fn load(folder: &Path) -> Result<Self, String> {
        Ok(Self {
            folder: Some(folder.to_path_buf()),
            ops: read_list(folder, OPS_FILE)?,
            banned: read_list(folder, BANNED_FILE)?,
            whitelist: read_list(folder, WHITELIST_FILE)?,
        })
    }

    /// Makes `names` operators of the server whose game folder is `folder`, in its file: the
    /// `--op` option of the dedicated server, which makes its first operator.
    pub fn add_operators(folder: &Path, names: &[String]) -> Result<(), String> {
        let mut permissions = Self::load(folder)?;
        for name in names {
            if !permissions.is_operator(name) {
                permissions.ops.push(name.clone());
            }
        }
        permissions.save(OPS_FILE)
    }

    /// Whether `name` is on the list of the operators. A solo player is one whatever the list
    /// says: see `may_run`.
    pub fn is_operator(&self, name: &str) -> bool {
        self.ops.iter().any(|op| same_name(op, name))
    }

    /// Whether the player `name` may run the commands of the operators: always in solo.
    pub fn may_run(&self, name: &str, is_solo: bool) -> bool {
        is_solo || self.is_operator(name)
    }

    /// Whether `name` may join, or the message it is refused with: a banned player never, a
    /// player off the whitelist only when it is off, an operator always.
    pub fn admission(&self, name: &str) -> Result<(), String> {
        if let Some((_, reason)) = self
            .banned
            .iter()
            .find(|(banned, _)| same_name(banned, name))
        {
            return Err(with_reason("You are banned from this server", reason));
        }
        let listed = self.whitelist.players.iter().any(|p| same_name(p, name));
        if self.whitelist.enabled && !listed && !self.is_operator(name) {
            return Err("You are not on the whitelist of this server.".to_string());
        }
        Ok(())
    }

    /// Writes the list in `file`, when the server keeps its lists in a folder.
    fn save(&self, file: &str) -> Result<(), String> {
        let Some(folder) = &self.folder else {
            return Ok(());
        };
        match file {
            OPS_FILE => write_list(folder, file, &self.ops),
            BANNED_FILE => write_list(folder, file, &self.banned),
            _ => write_list(folder, file, &self.whitelist),
        }
    }

    /// Runs `admin`; `online` tells whether a player of that name is connected. A list that
    /// cannot be written is changed all the same, for as long as the server runs, and the
    /// answer says so.
    pub fn apply(&mut self, admin: Admin, online: impl Fn(&str) -> bool) -> AdminOutcome {
        let answer = |answer: String| AdminOutcome {
            answer,
            dismiss: None,
        };
        let (outcome, changed) = match admin {
            Admin::Op(name) => {
                if self.is_operator(&name) {
                    (answer(format!("{name} is already an operator")), None)
                } else {
                    self.ops.push(name.clone());
                    (answer(format!("Made {name} an operator")), Some(OPS_FILE))
                }
            }
            Admin::Deop(name) => {
                if self.is_operator(&name) {
                    self.ops.retain(|op| !same_name(op, &name));
                    (
                        answer(format!("{name} is no longer an operator")),
                        Some(OPS_FILE),
                    )
                } else {
                    (answer(format!("{name} is not an operator")), None)
                }
            }
            Admin::Kick { name, reason } => {
                if online(&name) {
                    let message = with_reason("You were kicked from the server", &reason);
                    let outcome = AdminOutcome {
                        answer: format!("Kicked {name}"),
                        dismiss: Some((name, message)),
                    };
                    (outcome, None)
                } else {
                    (answer(format!("No player named {name} is online")), None)
                }
            }
            Admin::Ban { name, reason } => {
                self.banned.retain(|banned, _| !same_name(banned, &name));
                self.banned.insert(name.clone(), reason.clone());
                let dismiss = online(&name).then(|| {
                    let message = with_reason("You are banned from this server", &reason);
                    (name.clone(), message)
                });
                let outcome = AdminOutcome {
                    answer: format!("Banned {name}"),
                    dismiss,
                };
                (outcome, Some(BANNED_FILE))
            }
            Admin::Pardon(name) => {
                let before = self.banned.len();
                self.banned.retain(|banned, _| !same_name(banned, &name));
                if self.banned.len() < before {
                    (answer(format!("Pardoned {name}")), Some(BANNED_FILE))
                } else {
                    (answer(format!("{name} is not banned")), None)
                }
            }
            Admin::Whitelist(action) => self.whitelist(action),
        };
        let Some(file) = changed else {
            return outcome;
        };
        match self.save(file) {
            Ok(()) => outcome,
            Err(err) => {
                error!("{err}");
                AdminOutcome {
                    answer: format!(
                        "{}, until the server stops: the list could not be saved",
                        outcome.answer
                    ),
                    ..outcome
                }
            }
        }
    }

    fn whitelist(&mut self, action: WhitelistAction) -> (AdminOutcome, Option<&'static str>) {
        let answer = |answer: &str| AdminOutcome {
            answer: answer.to_string(),
            dismiss: None,
        };
        let list = &mut self.whitelist;
        match action {
            WhitelistAction::On => {
                list.enabled = true;
                (answer("The whitelist is on"), Some(WHITELIST_FILE))
            }
            WhitelistAction::Off => {
                list.enabled = false;
                (answer("The whitelist is off"), Some(WHITELIST_FILE))
            }
            WhitelistAction::Add(name) => {
                if list.players.iter().any(|p| same_name(p, &name)) {
                    (answer(&format!("{name} is already on the whitelist")), None)
                } else {
                    list.players.push(name.clone());
                    let added = format!("Added {name} to the whitelist");
                    (answer(&added), Some(WHITELIST_FILE))
                }
            }
            WhitelistAction::Remove(name) => {
                let before = list.players.len();
                list.players.retain(|p| !same_name(p, &name));
                if list.players.len() < before {
                    let removed = format!("Removed {name} from the whitelist");
                    (answer(&removed), Some(WHITELIST_FILE))
                } else {
                    (answer(&format!("{name} is not on the whitelist")), None)
                }
            }
            WhitelistAction::List => {
                let state = if list.enabled { "on" } else { "off" };
                let names = if list.players.is_empty() {
                    "nobody".to_string()
                } else {
                    list.players.join(", ")
                };
                (answer(&format!("The whitelist is {state}: {names}")), None)
            }
        }
    }
}

/// The clients told why they are dismissed (`ServerToClientMessage::Dismissed`), and when the
/// server disconnects them.
#[derive(Resource, Debug, Default)]
pub struct Dismissals(Vec<(ClientId, Instant)>);

impl Dismissals {
    /// Tells `client_id` why it is dismissed, and disconnects it after `DISMISS_DELAY`.
    pub fn dismiss(&mut self, server: &mut RenetServer, client_id: ClientId, message: String) {
        info!("Dismissing client {client_id}: {message}");
        server.send_game_message(client_id, ServerToClientMessage::Dismissed(message));
        if !self.contains(client_id) {
            self.0.push((client_id, Instant::now() + DISMISS_DELAY));
        }
    }

    /// Whether `client_id` is being dismissed: the server no longer reads what it sends.
    pub fn contains(&self, client_id: ClientId) -> bool {
        self.0.iter().any(|(id, _)| *id == client_id)
    }

    /// Forgets a client that left by itself: it may come back under the same id.
    pub fn forget(&mut self, client_id: ClientId) {
        self.0.retain(|(id, _)| *id != client_id);
    }

    /// Takes the clients whose time has come at `now`.
    fn due(&mut self, now: Instant) -> Vec<ClientId> {
        let (due, waiting) = self.0.drain(..).partition(|(_, at)| *at <= now);
        self.0 = waiting;
        due.into_iter().map(|(id, _)| id).collect()
    }
}

/// Disconnects the dismissed clients whose delay is over.
pub fn disconnect_dismissed_system(
    mut dismissals: ResMut<Dismissals>,
    mut server: ResMut<RenetServer>,
) {
    for client_id in dismissals.due(Instant::now()) {
        server.disconnect(client_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nobody_online(_: &str) -> bool {
        false
    }

    #[test]
    fn an_operator_is_found_whatever_the_case_and_a_solo_player_is_always_one() {
        let mut permissions = Permissions::default();
        assert!(!permissions.may_run("Alice", false));
        assert!(permissions.may_run("Alice", true));
        permissions.apply(Admin::Op("alice".into()), nobody_online);
        assert!(permissions.is_operator("ALICE"));
        assert!(permissions.may_run("Alice", false));
        let outcome = permissions.apply(Admin::Op("Alice".into()), nobody_online);
        assert_eq!(outcome.answer, "Alice is already an operator");
        permissions.apply(Admin::Deop("Alice".into()), nobody_online);
        assert!(!permissions.may_run("alice", false));
        let outcome = permissions.apply(Admin::Deop("alice".into()), nobody_online);
        assert_eq!(outcome.answer, "alice is not an operator");
    }

    #[test]
    fn a_kick_dismisses_an_online_player_with_its_reason() {
        let mut permissions = Permissions::default();
        let outcome = permissions.apply(
            Admin::Kick {
                name: "bob".into(),
                reason: Some("too loud".into()),
            },
            |name| name == "bob",
        );
        assert_eq!(outcome.answer, "Kicked bob");
        assert_eq!(
            outcome.dismiss,
            Some((
                "bob".into(),
                "You were kicked from the server: too loud".into()
            ))
        );
        let outcome = permissions.apply(
            Admin::Kick {
                name: "carol".into(),
                reason: None,
            },
            nobody_online,
        );
        assert_eq!(outcome.answer, "No player named carol is online");
        assert_eq!(outcome.dismiss, None);
    }

    #[test]
    fn a_banned_player_is_refused_until_pardoned_and_dismissed_when_online() {
        let mut permissions = Permissions::default();
        let outcome = permissions.apply(
            Admin::Ban {
                name: "Bob".into(),
                reason: None,
            },
            |_| true,
        );
        assert_eq!(
            outcome.dismiss,
            Some(("Bob".into(), "You are banned from this server.".into()))
        );
        assert_eq!(
            permissions.admission("bob"),
            Err("You are banned from this server.".into())
        );
        // banned again, with a reason: still one entry
        permissions.apply(
            Admin::Ban {
                name: "bob".into(),
                reason: Some("griefing".into()),
            },
            nobody_online,
        );
        assert_eq!(permissions.banned.len(), 1);
        assert_eq!(
            permissions.admission("BOB"),
            Err("You are banned from this server: griefing".into())
        );
        let outcome = permissions.apply(Admin::Pardon("BoB".into()), nobody_online);
        assert_eq!(outcome.answer, "Pardoned BoB");
        assert_eq!(permissions.admission("bob"), Ok(()));
        let outcome = permissions.apply(Admin::Pardon("bob".into()), nobody_online);
        assert_eq!(outcome.answer, "bob is not banned");
    }

    #[test]
    fn the_whitelist_refuses_others_only_when_on_and_never_an_operator() {
        let mut permissions = Permissions::default();
        let whitelist = |action| Admin::Whitelist(action);
        permissions.apply(
            whitelist(WhitelistAction::Add("alice".into())),
            nobody_online,
        );
        assert_eq!(permissions.admission("bob"), Ok(()));
        permissions.apply(whitelist(WhitelistAction::On), nobody_online);
        assert_eq!(permissions.admission("Alice"), Ok(()));
        assert_eq!(
            permissions.admission("bob"),
            Err("You are not on the whitelist of this server.".into())
        );
        permissions.apply(Admin::Op("bob".into()), nobody_online);
        assert_eq!(permissions.admission("bob"), Ok(()));
        let outcome = permissions.apply(whitelist(WhitelistAction::List), nobody_online);
        assert_eq!(outcome.answer, "The whitelist is on: alice");
        permissions.apply(
            whitelist(WhitelistAction::Remove("ALICE".into())),
            nobody_online,
        );
        assert!(permissions.admission("alice").is_err());
        permissions.apply(whitelist(WhitelistAction::Off), nobody_online);
        assert_eq!(permissions.admission("alice"), Ok(()));
        let outcome = permissions.apply(whitelist(WhitelistAction::List), nobody_online);
        assert_eq!(outcome.answer, "The whitelist is off: nobody");
    }

    #[test]
    fn the_lists_come_back_from_their_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mut permissions = Permissions::load(tmp.path()).unwrap();
        assert!(!permissions.is_operator("alice"));
        permissions.apply(Admin::Op("alice".into()), nobody_online);
        permissions.apply(
            Admin::Ban {
                name: "bob".into(),
                reason: Some("griefing".into()),
            },
            nobody_online,
        );
        permissions.apply(
            Admin::Whitelist(WhitelistAction::Add("carol".into())),
            nobody_online,
        );
        permissions.apply(Admin::Whitelist(WhitelistAction::On), nobody_online);

        let loaded = Permissions::load(tmp.path()).unwrap();
        assert_eq!(loaded.ops, ["alice"]);
        assert_eq!(loaded.banned, permissions.banned);
        assert_eq!(loaded.whitelist, permissions.whitelist);
        assert!(loaded.whitelist.enabled);

        // the option of the server adds to the file
        Permissions::add_operators(tmp.path(), &["dave".into(), "Alice".into()]).unwrap();
        assert_eq!(
            Permissions::load(tmp.path()).unwrap().ops,
            ["alice", "dave"]
        );
    }

    #[test]
    fn a_list_that_cannot_be_read_is_an_error_naming_its_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(BANNED_FILE), "not ron {").unwrap();
        let err = Permissions::load(tmp.path()).unwrap_err();
        assert!(err.contains(BANNED_FILE), "{err}");
    }

    #[test]
    fn a_dismissed_client_is_disconnected_once_its_delay_is_over() {
        let mut dismissals = Dismissals::default();
        let now = Instant::now();
        dismissals.0.push((1, now + DISMISS_DELAY));
        dismissals.0.push((2, now));
        assert_eq!(dismissals.due(now), [2]);
        assert_eq!(dismissals.due(now + DISMISS_DELAY), [1]);
        assert!(dismissals.due(now + DISMISS_DELAY).is_empty());
    }
}
