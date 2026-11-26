//! Who may do what on a server: its operators, who alone run the commands that change the game
//! (`commands::refusal`), the players it bans, and its optional whitelist.
//!
//! A dedicated server keeps each list in a file of its game folder, next to `saves/`: read when
//! it starts, written whenever a command changes it. A solo world keeps none: its player is
//! always an operator, and a list it changes lasts as long as its server.
//!
//! Names are compared without their case, as players type them in commands: `/ban Steve` bans
//! `steve`. A list keeps them as they were typed.
//!
//! On a server in hub mode a list holds accounts: an entry is `(id: "<uuid>", name: "<name>")`,
//! and a player is its account whatever its name is. An entry that is only a name, as an offline
//! server writes it, matches the account that has this name when it joins, and is rewritten with
//! its id then.
//!
//! A hosted world keeps no list: its members and its operators are those the hub tells, and
//! only the hub changes them (`Permissions::hosted`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use bevy_renet::RenetServer;
use rustcraft_protocol::api::{MemberRole, RealmConfig};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use shared::messages::ServerToClientMessage;
use uuid::Uuid;

use crate::network::extensions::SendGameMessageExtension;
use crate::world::format::write_atomically;

/// The operators of a dedicated server: a list of names, or of accounts in hub mode.
pub const OPS_FILE: &str = "ops.ron";
/// The players a dedicated server bans: their names, each with the reason given, if any (a
/// list of accounts in hub mode).
pub const BANNED_FILE: &str = "banned.ron";
/// The whitelist of a dedicated server, and whether it is on.
pub const WHITELIST_FILE: &str = "whitelist.ron";

/// The answer to the commands that change the lists, on a hosted world.
pub const HOSTED_REFUSAL: &str =
    "Manage the members of a hosted world from the game's Hosted worlds tab.";

/// What a player is dismissed with when the hub no longer lists it among the members.
pub const REMOVED: &str = "You were removed from this world.";

/// How long a dismissed client stays connected after its `Dismissed` message, so that the
/// message reaches it before the connection closes: the client ends its session on the message
/// itself, and this is only for a client that does not.
pub const DISMISS_DELAY: Duration = Duration::from_millis(500);

/// A player of a list: an account, or only a name until the account that has it joins.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(from = "EntryFile", into = "EntryFile")]
pub struct Entry {
    pub id: Option<Uuid>,
    pub name: String,
}

/// How an `Entry` is written: a name alone, or an account.
#[derive(Serialize, Deserialize, Clone)]
#[serde(untagged)]
enum EntryFile {
    Name(String),
    Account(AccountFile),
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename = "")]
struct AccountFile {
    id: Uuid,
    name: String,
}

impl From<EntryFile> for Entry {
    fn from(file: EntryFile) -> Self {
        match file {
            EntryFile::Name(name) => Self { id: None, name },
            EntryFile::Account(AccountFile { id, name }) => Self { id: Some(id), name },
        }
    }
}

impl From<Entry> for EntryFile {
    fn from(entry: Entry) -> Self {
        match entry.id {
            None => Self::Name(entry.name),
            Some(id) => Self::Account(AccountFile {
                id,
                name: entry.name,
            }),
        }
    }
}

impl Entry {
    /// A player known by its name only: the only kind of an offline server.
    pub fn named(name: impl Into<String>) -> Self {
        Self {
            id: None,
            name: name.into(),
        }
    }

    pub fn account(id: Uuid, name: impl Into<String>) -> Self {
        Self {
            id: Some(id),
            name: name.into(),
        }
    }

    /// Whether this is the player `name`, of account `account` in hub mode: the accounts are
    /// compared when both sides have one, the names otherwise.
    fn is(&self, name: &str, account: Option<Uuid>) -> bool {
        match (self.id, account) {
            (Some(mine), Some(theirs)) => mine == theirs,
            _ => same_name(&self.name, name),
        }
    }

    /// Gives this entry the account and the name of the player that joins as `name`, when it is
    /// that player: its id, if it had none, and the name it has now. Whether it changed.
    fn bind(&mut self, name: &str, account: Uuid) -> bool {
        let ours = match self.id {
            Some(id) => id == account,
            None => same_name(&self.name, name),
        };
        let bound = Self::account(account, name);
        let changed = ours && *self != bound;
        if changed {
            *self = bound;
        }
        changed
    }
}

/// The whitelist: when it is on, only the players on it, and the operators, may join.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq)]
pub struct Whitelist {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub players: Vec<Entry>,
}

/// A banned player, with the reason given, if any.
#[derive(Debug, Clone, PartialEq)]
struct Ban {
    player: Entry,
    reason: Option<String>,
}

/// How the bans are written: the names with their reasons as an offline server does, or the
/// accounts once one is banned.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum BannedFile {
    Names(BTreeMap<String, Option<String>>),
    Accounts(Vec<BanFile>),
}

#[derive(Serialize, Deserialize)]
#[serde(rename = "")]
struct BanFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    id: Option<Uuid>,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl Default for BannedFile {
    fn default() -> Self {
        Self::Names(BTreeMap::new())
    }
}

impl From<BannedFile> for Vec<Ban> {
    fn from(file: BannedFile) -> Self {
        match file {
            BannedFile::Names(names) => names
                .into_iter()
                .map(|(name, reason)| Ban {
                    player: Entry::named(name),
                    reason,
                })
                .collect(),
            BannedFile::Accounts(bans) => bans
                .into_iter()
                .map(|ban| Ban {
                    player: Entry {
                        id: ban.id,
                        name: ban.name,
                    },
                    reason: ban.reason,
                })
                .collect(),
        }
    }
}

impl BannedFile {
    fn of(bans: &[Ban]) -> Self {
        if bans.iter().all(|ban| ban.player.id.is_none()) {
            return Self::Names(
                bans.iter()
                    .map(|ban| (ban.player.name.clone(), ban.reason.clone()))
                    .collect(),
            );
        }
        Self::Accounts(
            bans.iter()
                .map(|ban| BanFile {
                    id: ban.player.id,
                    name: ban.player.name.clone(),
                    reason: ban.reason.clone(),
                })
                .collect(),
        )
    }
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
/// with the message the player is shown. A player banned by account is found by `account`
/// instead of by name: the name of a connected player is the one of its certificate, which may
/// be old.
#[derive(Debug, PartialEq)]
pub struct AdminOutcome {
    pub answer: String,
    pub dismiss: Option<(String, String)>,
    pub account: Option<Uuid>,
}

impl Admin {
    /// This command for the player `name`, as the hub spells it.
    pub fn with_name(self, name: String) -> Self {
        match self {
            Self::Op(_) => Self::Op(name),
            Self::Deop(_) => Self::Deop(name),
            Self::Kick { reason, .. } => Self::Kick { name, reason },
            Self::Ban { reason, .. } => Self::Ban { name, reason },
            Self::Pardon(_) => Self::Pardon(name),
            Self::Whitelist(WhitelistAction::Add(_)) => Self::Whitelist(WhitelistAction::Add(name)),
            Self::Whitelist(WhitelistAction::Remove(_)) => {
                Self::Whitelist(WhitelistAction::Remove(name))
            }
            other => other,
        }
    }
}

/// The lists of a server, and the folder they are saved in (`None` for a solo world).
#[derive(Resource, Debug, Default)]
pub struct Permissions {
    folder: Option<PathBuf>,
    ops: Vec<Entry>,
    banned: Vec<Ban>,
    whitelist: Whitelist,
    /// Whether the lists are those of a hosted world: the hub's, and read only.
    hosted: bool,
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
            banned: read_list::<BannedFile>(folder, BANNED_FILE)?.into(),
            whitelist: read_list(folder, WHITELIST_FILE)?,
            hosted: false,
        })
    }

    /// The lists of a hosted world: the whitelist is on and holds the members, the operators
    /// are those of the members that are, and the owner. Nothing is read from nor written to
    /// files, and nobody is banned: a removed member is.
    pub fn hosted(config: &RealmConfig) -> Self {
        let mut permissions = Self {
            hosted: true,
            ..Self::default()
        };
        permissions.follow(config);
        permissions
    }

    /// Replaces the lists of a hosted world with the members of `config`.
    pub fn follow(&mut self, config: &RealmConfig) {
        let entry = |id, name: &str| Entry::account(id, name);
        self.ops = std::iter::once(entry(config.owner.id, &config.owner.username))
            .chain(
                config
                    .members
                    .iter()
                    .filter(|member| member.role == MemberRole::Operator)
                    .map(|member| entry(member.id, &member.username)),
            )
            .collect();
        self.whitelist = Whitelist {
            enabled: true,
            players: config
                .members
                .iter()
                .map(|member| entry(member.id, &member.username))
                .collect(),
        };
    }

    /// Whether a command that changes who may do what is refused: all of them on a hosted
    /// world, but `/kick`, whose hub is the one that knows its members.
    pub fn refuses(&self, admin: &Admin) -> bool {
        self.hosted && !matches!(admin, Admin::Kick { .. })
    }

    /// Makes `players` operators of the server whose game folder is `folder`, in its file: the
    /// `--op` option of the dedicated server, which makes its first operator.
    pub fn add_operators(folder: &Path, players: &[Entry]) -> Result<(), String> {
        let mut permissions = Self::load(folder)?;
        for player in players {
            if !permissions.is_operator(&player.name, player.id) {
                permissions.ops.push(player.clone());
            }
        }
        permissions.save(OPS_FILE)
    }

    /// Whether the player `name`, of account `account` in hub mode, is on the list of the
    /// operators. A solo player is one whatever the list says: see `may_run`.
    pub fn is_operator(&self, name: &str, account: Option<Uuid>) -> bool {
        self.ops.iter().any(|op| op.is(name, account))
    }

    /// Whether the player may run the commands of the operators: always in solo.
    pub fn may_run(&self, name: &str, account: Option<Uuid>, is_solo: bool) -> bool {
        is_solo || self.is_operator(name, account)
    }

    /// Whether the player may join, or the message it is refused with: a banned player never, a
    /// player off the whitelist only when it is off, an operator always.
    pub fn admission(&self, name: &str, account: Option<Uuid>) -> Result<(), String> {
        if let Some(ban) = self.banned.iter().find(|ban| ban.player.is(name, account)) {
            return Err(with_reason("You are banned from this server", &ban.reason));
        }
        let listed = self.whitelist.players.iter().any(|p| p.is(name, account));
        if self.whitelist.enabled && !listed && !self.is_operator(name, account) {
            return Err("You are not on the whitelist of this server.".to_string());
        }
        Ok(())
    }

    /// Gives the entries of the player `name` of account `account` that name it, or hold its
    /// account under another name, its account and its name of now, and writes the lists that
    /// changed: a name written by hand, or an offline file, becomes an account when its player
    /// first joins, and a player that was renamed is found by its name again.
    pub fn identify(&mut self, name: &str, account: Uuid) {
        let mut changed = Vec::new();
        let mut bind = |entry: &mut Entry, file| {
            if entry.bind(name, account) {
                changed.push(file);
            }
        };
        self.ops.iter_mut().for_each(|op| bind(op, OPS_FILE));
        for ban in &mut self.banned {
            bind(&mut ban.player, BANNED_FILE);
        }
        for player in &mut self.whitelist.players {
            bind(player, WHITELIST_FILE);
        }
        changed.sort_unstable();
        changed.dedup();
        for file in changed {
            if let Err(err) = self.save(file) {
                error!("{err}");
            }
        }
    }

    /// The names that the lists hold without an account, on a server in hub mode: they stay
    /// names until the account that has one joins.
    pub fn unbound_names(&self) -> Vec<(&'static str, &str)> {
        fn named<'a>(file: &'static str, entry: &'a Entry) -> Option<(&'static str, &'a str)> {
            entry.id.is_none().then_some((file, entry.name.as_str()))
        }
        let ops = self.ops.iter().filter_map(|e| named(OPS_FILE, e));
        let banned = self
            .banned
            .iter()
            .filter_map(|b| named(BANNED_FILE, &b.player));
        let listed = self
            .whitelist
            .players
            .iter()
            .filter_map(|e| named(WHITELIST_FILE, e));
        ops.chain(banned).chain(listed).collect()
    }

    /// The name of `admin` that must be resolved to an account before it runs, on a server in
    /// hub mode: of a player to make an operator, ban or whitelist, or one to remove that no
    /// entry names, which may have been renamed since.
    pub fn name_to_resolve<'a>(&self, admin: &'a Admin) -> Option<&'a str> {
        let named = |name: &str, mut players: std::slice::Iter<Entry>| {
            players.any(|entry| same_name(&entry.name, name))
        };
        match admin {
            Admin::Op(name)
            | Admin::Ban { name, .. }
            | Admin::Whitelist(WhitelistAction::Add(name)) => Some(name),
            Admin::Deop(name) => (!named(name, self.ops.iter())).then_some(name),
            Admin::Pardon(name) => {
                let banned = self
                    .banned
                    .iter()
                    .any(|ban| same_name(&ban.player.name, name));
                (!banned).then_some(name)
            }
            Admin::Whitelist(WhitelistAction::Remove(name)) => {
                (!named(name, self.whitelist.players.iter())).then_some(name)
            }
            _ => None,
        }
        .map(String::as_str)
    }

    /// Writes the list in `file`, when the server keeps its lists in a folder.
    fn save(&self, file: &str) -> Result<(), String> {
        let Some(folder) = &self.folder else {
            return Ok(());
        };
        match file {
            OPS_FILE => write_list(folder, file, &self.ops),
            BANNED_FILE => write_list(folder, file, &BannedFile::of(&self.banned)),
            _ => write_list(folder, file, &self.whitelist),
        }
    }

    /// Runs `admin`, on the account `account` that its player has in hub mode; `online` tells
    /// whether a player of that name is connected. A list that cannot be written is changed
    /// all the same, for as long as the server runs, and the answer says so.
    pub fn apply(
        &mut self,
        admin: Admin,
        online: impl Fn(&str) -> bool,
        account: Option<Uuid>,
    ) -> AdminOutcome {
        let answer = |answer: String| AdminOutcome {
            answer,
            dismiss: None,
            account: None,
        };
        let (outcome, changed) = match admin {
            Admin::Op(name) => {
                if self.is_operator(&name, account) {
                    (answer(format!("{name} is already an operator")), None)
                } else {
                    self.ops.push(Entry {
                        id: account,
                        name: name.clone(),
                    });
                    (answer(format!("Made {name} an operator")), Some(OPS_FILE))
                }
            }
            Admin::Deop(name) => {
                if self.is_operator(&name, account) {
                    self.ops.retain(|op| !op.is(&name, account));
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
                        account: None,
                    };
                    (outcome, None)
                } else {
                    (answer(format!("No player named {name} is online")), None)
                }
            }
            Admin::Ban { name, reason } => {
                self.banned.retain(|ban| !ban.player.is(&name, account));
                self.banned.push(Ban {
                    player: Entry {
                        id: account,
                        name: name.clone(),
                    },
                    reason: reason.clone(),
                });
                // A banned account is looked for among the connected players by its id
                let dismiss = (account.is_some() || online(&name)).then(|| {
                    let message = with_reason("You are banned from this server", &reason);
                    (name.clone(), message)
                });
                let outcome = AdminOutcome {
                    answer: format!("Banned {name}"),
                    dismiss,
                    account,
                };
                (outcome, Some(BANNED_FILE))
            }
            Admin::Pardon(name) => {
                let before = self.banned.len();
                self.banned.retain(|ban| !ban.player.is(&name, account));
                if self.banned.len() < before {
                    (answer(format!("Pardoned {name}")), Some(BANNED_FILE))
                } else {
                    (answer(format!("{name} is not banned")), None)
                }
            }
            Admin::Whitelist(action) => self.whitelist(action, account),
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

    fn whitelist(
        &mut self,
        action: WhitelistAction,
        account: Option<Uuid>,
    ) -> (AdminOutcome, Option<&'static str>) {
        let answer = |answer: &str| AdminOutcome {
            answer: answer.to_string(),
            dismiss: None,
            account: None,
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
                if list.players.iter().any(|p| p.is(&name, account)) {
                    (answer(&format!("{name} is already on the whitelist")), None)
                } else {
                    list.players.push(Entry {
                        id: account,
                        name: name.clone(),
                    });
                    let added = format!("Added {name} to the whitelist");
                    (answer(&added), Some(WHITELIST_FILE))
                }
            }
            WhitelistAction::Remove(name) => {
                let before = list.players.len();
                list.players.retain(|p| !p.is(&name, account));
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
                    let names: Vec<&str> = list.players.iter().map(|p| p.name.as_str()).collect();
                    names.join(", ")
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
        assert!(!permissions.may_run("Alice", None, false));
        assert!(permissions.may_run("Alice", None, true));
        permissions.apply(Admin::Op("alice".into()), nobody_online, None);
        assert!(permissions.is_operator("ALICE", None));
        assert!(permissions.may_run("Alice", None, false));
        let outcome = permissions.apply(Admin::Op("Alice".into()), nobody_online, None);
        assert_eq!(outcome.answer, "Alice is already an operator");
        permissions.apply(Admin::Deop("Alice".into()), nobody_online, None);
        assert!(!permissions.may_run("alice", None, false));
        let outcome = permissions.apply(Admin::Deop("alice".into()), nobody_online, None);
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
            None,
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
            None,
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
            None,
        );
        assert_eq!(
            outcome.dismiss,
            Some(("Bob".into(), "You are banned from this server.".into()))
        );
        assert_eq!(
            permissions.admission("bob", None),
            Err("You are banned from this server.".into())
        );
        // banned again, with a reason: still one entry
        permissions.apply(
            Admin::Ban {
                name: "bob".into(),
                reason: Some("griefing".into()),
            },
            nobody_online,
            None,
        );
        assert_eq!(permissions.banned.len(), 1);
        assert_eq!(
            permissions.admission("BOB", None),
            Err("You are banned from this server: griefing".into())
        );
        let outcome = permissions.apply(Admin::Pardon("BoB".into()), nobody_online, None);
        assert_eq!(outcome.answer, "Pardoned BoB");
        assert_eq!(permissions.admission("bob", None), Ok(()));
        let outcome = permissions.apply(Admin::Pardon("bob".into()), nobody_online, None);
        assert_eq!(outcome.answer, "bob is not banned");
    }

    #[test]
    fn the_whitelist_refuses_others_only_when_on_and_never_an_operator() {
        let mut permissions = Permissions::default();
        let whitelist = |action| Admin::Whitelist(action);
        permissions.apply(
            whitelist(WhitelistAction::Add("alice".into())),
            nobody_online,
            None,
        );
        assert_eq!(permissions.admission("bob", None), Ok(()));
        permissions.apply(whitelist(WhitelistAction::On), nobody_online, None);
        assert_eq!(permissions.admission("Alice", None), Ok(()));
        assert_eq!(
            permissions.admission("bob", None),
            Err("You are not on the whitelist of this server.".into())
        );
        permissions.apply(Admin::Op("bob".into()), nobody_online, None);
        assert_eq!(permissions.admission("bob", None), Ok(()));
        let outcome = permissions.apply(whitelist(WhitelistAction::List), nobody_online, None);
        assert_eq!(outcome.answer, "The whitelist is on: alice");
        permissions.apply(
            whitelist(WhitelistAction::Remove("ALICE".into())),
            nobody_online,
            None,
        );
        assert!(permissions.admission("alice", None).is_err());
        permissions.apply(whitelist(WhitelistAction::Off), nobody_online, None);
        assert_eq!(permissions.admission("alice", None), Ok(()));
        let outcome = permissions.apply(whitelist(WhitelistAction::List), nobody_online, None);
        assert_eq!(outcome.answer, "The whitelist is off: nobody");
    }

    #[test]
    fn the_lists_come_back_from_their_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mut permissions = Permissions::load(tmp.path()).unwrap();
        assert!(!permissions.is_operator("alice", None));
        permissions.apply(Admin::Op("alice".into()), nobody_online, None);
        permissions.apply(
            Admin::Ban {
                name: "bob".into(),
                reason: Some("griefing".into()),
            },
            nobody_online,
            None,
        );
        permissions.apply(
            Admin::Whitelist(WhitelistAction::Add("carol".into())),
            nobody_online,
            None,
        );
        permissions.apply(Admin::Whitelist(WhitelistAction::On), nobody_online, None);

        let loaded = Permissions::load(tmp.path()).unwrap();
        assert_eq!(loaded.ops, [Entry::named("alice")]);
        assert_eq!(loaded.banned, permissions.banned);
        // an offline server writes names, as it always did
        let file = |name| std::fs::read_to_string(tmp.path().join(name)).unwrap();
        assert_eq!(file(OPS_FILE), "[\n    \"alice\",\n]");
        assert_eq!(file(BANNED_FILE), "{\n    \"bob\": Some(\"griefing\"),\n}");
        assert_eq!(loaded.whitelist, permissions.whitelist);
        assert!(loaded.whitelist.enabled);

        // the option of the server adds to the file
        Permissions::add_operators(tmp.path(), &[Entry::named("dave"), Entry::named("Alice")])
            .unwrap();
        assert_eq!(
            Permissions::load(tmp.path()).unwrap().ops,
            [Entry::named("alice"), Entry::named("dave")]
        );
    }

    #[test]
    fn a_list_that_cannot_be_read_is_an_error_naming_its_file() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(BANNED_FILE), "not ron {").unwrap();
        let err = Permissions::load(tmp.path()).unwrap_err();
        assert!(err.contains(BANNED_FILE), "{err}");
    }

    fn account(n: u128) -> Option<Uuid> {
        Some(Uuid::from_u128(n))
    }

    #[test]
    fn an_account_is_an_operator_whatever_its_name() {
        let mut permissions = Permissions::default();
        permissions.apply(Admin::Op("Alex".into()), nobody_online, account(1));
        assert!(permissions.may_run("Alex", account(1), false));
        // renamed: still its account
        assert!(permissions.may_run("Steve", account(1), false));
        // another account that took the name is not
        assert!(!permissions.may_run("Alex", account(2), false));
        let outcome = permissions.apply(Admin::Op("Steve".into()), nobody_online, account(1));
        assert_eq!(outcome.answer, "Steve is already an operator");
        permissions.apply(Admin::Deop("Steve".into()), nobody_online, account(1));
        assert!(!permissions.may_run("Alex", account(1), false));
    }

    #[test]
    fn a_banned_account_is_refused_whatever_its_name_and_pardoned_by_its_name_on_the_list() {
        let mut permissions = Permissions::default();
        let ban = Admin::Ban {
            name: "Alex".into(),
            reason: Some("griefing".into()),
        };
        permissions.apply(ban, nobody_online, account(1));
        let refused = Err("You are banned from this server: griefing".to_string());
        assert_eq!(permissions.admission("Steve", account(1)), refused);
        assert_eq!(permissions.admission("Alex", account(2)), Ok(()));
        // the entry names the player, so that no lookup is asked to pardon it
        let pardon = Admin::Pardon("alex".into());
        assert_eq!(permissions.name_to_resolve(&pardon), None);
        permissions.apply(pardon, nobody_online, None);
        assert_eq!(permissions.admission("Steve", account(1)), Ok(()));
    }

    #[test]
    fn the_names_to_resolve_are_those_a_list_cannot_find() {
        let mut permissions = Permissions::default();
        let op = Admin::Op("Alex".into());
        assert_eq!(permissions.name_to_resolve(&op), Some("Alex"));
        let deop = Admin::Deop("Alex".into());
        assert_eq!(permissions.name_to_resolve(&deop), Some("Alex"));
        permissions.apply(op, nobody_online, account(1));
        assert_eq!(permissions.name_to_resolve(&deop), None);
        let kick = Admin::Kick {
            name: "Alex".into(),
            reason: None,
        };
        assert_eq!(permissions.name_to_resolve(&kick), None);
        let list = Admin::Whitelist(WhitelistAction::List);
        assert_eq!(permissions.name_to_resolve(&list), None);
    }

    #[test]
    fn a_name_entry_becomes_its_account_when_it_first_joins_and_follows_its_renames() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(OPS_FILE), r#"["alex", "bob"]"#).unwrap();
        std::fs::write(
            tmp.path().join(WHITELIST_FILE),
            r#"(enabled: true, players: ["Alex"])"#,
        )
        .unwrap();
        std::fs::write(tmp.path().join(BANNED_FILE), r#"{"Eve": Some("spam")}"#).unwrap();
        let mut permissions = Permissions::load(tmp.path()).unwrap();
        permissions.identify("Alex", Uuid::from_u128(1));
        permissions.identify("Eve", Uuid::from_u128(3));
        let ops = std::fs::read_to_string(tmp.path().join(OPS_FILE)).unwrap();
        assert!(
            ops.contains(r#"id: "00000000-0000-0000-0000-000000000001""#),
            "{ops}"
        );
        assert!(ops.contains(r#""bob""#), "{ops}");
        let loaded = Permissions::load(tmp.path()).unwrap();
        assert!(loaded.may_run("Whoever", account(1), false));
        assert!(loaded.may_run("bob", account(2), false));
        assert_eq!(loaded.admission("Alex", account(1)), Ok(()));
        assert_eq!(
            loaded.admission("Eve", account(3)),
            Err("You are banned from this server: spam".into())
        );
        // the entry has the name of its account now
        permissions.identify("Alexandra", Uuid::from_u128(1));
        let loaded = Permissions::load(tmp.path()).unwrap();
        assert_eq!(
            loaded.ops[0],
            Entry::account(Uuid::from_u128(1), "Alexandra")
        );
        assert_eq!(
            loaded.whitelist.players,
            [Entry::account(Uuid::from_u128(1), "Alexandra")]
        );
    }

    #[test]
    fn the_accounts_of_the_lists_come_back_from_their_files() {
        let tmp = tempfile::tempdir().unwrap();
        let mut permissions = Permissions::load(tmp.path()).unwrap();
        permissions.apply(Admin::Op("Alex".into()), nobody_online, account(1));
        permissions.apply(
            Admin::Ban {
                name: "Eve".into(),
                reason: Some("spam".into()),
            },
            nobody_online,
            account(3),
        );
        permissions.apply(
            Admin::Ban {
                name: "Mallory".into(),
                reason: None,
            },
            nobody_online,
            None,
        );
        permissions.apply(
            Admin::Whitelist(WhitelistAction::Add("Bob".into())),
            nobody_online,
            account(2),
        );
        let loaded = Permissions::load(tmp.path()).unwrap();
        assert_eq!(loaded.ops, permissions.ops);
        assert_eq!(loaded.banned, permissions.banned);
        assert_eq!(loaded.whitelist, permissions.whitelist);
        assert_eq!(loaded.banned.len(), 2);
        let ops = std::fs::read_to_string(tmp.path().join(OPS_FILE)).unwrap();
        assert_eq!(
            ops,
            "[\n    (\n        id: \"00000000-0000-0000-0000-000000000001\",\n        name: \"Alex\",\n    ),\n]"
        );
    }

    #[test]
    fn the_names_without_an_account_are_listed_by_file() {
        let mut permissions = Permissions::default();
        permissions.apply(Admin::Op("Alex".into()), nobody_online, account(1));
        permissions.apply(Admin::Op("carol".into()), nobody_online, None);
        let ban = Admin::Ban {
            name: "eve".into(),
            reason: None,
        };
        permissions.apply(ban, nobody_online, None);
        assert_eq!(
            permissions.unbound_names(),
            [(OPS_FILE, "carol"), (BANNED_FILE, "eve")]
        );
    }

    #[test]
    fn a_ban_by_account_dismisses_by_account_and_takes_the_name_of_the_hub() {
        let mut permissions = Permissions::default();
        let ban = Admin::Ban {
            name: "bob".into(),
            reason: None,
        };
        let outcome = permissions.apply(ban.with_name("Bob".into()), nobody_online, account(1));
        assert_eq!(outcome.account, account(1));
        assert!(outcome.dismiss.is_some());
        assert_eq!(outcome.answer, "Banned Bob");
        assert_eq!(permissions.banned[0].player.name, "Bob");
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

    #[test]
    fn a_hosted_world_lists_its_members_and_makes_operators_of_the_owner_and_the_operators() {
        use rustcraft_protocol::api::{AccountSummary, RealmMember};
        let member = |n: u128, name: &str, role| RealmMember {
            id: Uuid::from_u128(n),
            username: name.into(),
            role,
        };
        let config = |members| RealmConfig {
            members,
            owner: AccountSummary {
                id: Uuid::from_u128(9),
                username: "Olivia".into(),
            },
        };
        let mut permissions = Permissions::hosted(&config(vec![
            member(1, "Alex", MemberRole::Member),
            member(2, "Bob", MemberRole::Operator),
        ]));
        let (alex, bob, owner) = (Uuid::from_u128(1), Uuid::from_u128(2), Uuid::from_u128(9));
        assert!(permissions.admission("Alex", Some(alex)).is_ok());
        assert!(permissions.admission("Olivia", Some(owner)).is_ok());
        assert!(permissions
            .admission("Eve", Some(Uuid::from_u128(3)))
            .is_err());
        assert!(!permissions.is_operator("Alex", Some(alex)));
        assert!(permissions.is_operator("Bob", Some(bob)));
        assert!(permissions.is_operator("Olivia", Some(owner)));
        assert!(permissions.refuses(&Admin::Op("Alex".into())));
        assert!(permissions.refuses(&Admin::Whitelist(WhitelistAction::List)));
        assert!(!permissions.refuses(&Admin::Kick {
            name: "Alex".into(),
            reason: None
        }));
        assert!(!Permissions::default().refuses(&Admin::Op("Alex".into())));

        // a member renamed on the hub is still the same account, and a removed one is out
        permissions.follow(&config(vec![member(2, "Robert", MemberRole::Member)]));
        assert!(permissions.admission("Robert", Some(bob)).is_ok());
        assert!(!permissions.is_operator("Bob", Some(bob)));
        assert!(permissions.admission("Alex", Some(alex)).is_err());
    }
}
