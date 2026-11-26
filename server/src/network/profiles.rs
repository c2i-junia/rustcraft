//! The accounts of the names that the operators give to `/op`, `/ban`, `/pardon` and
//! `/whitelist` on a server in hub mode: a player online has its account in the lobby, the
//! others are asked to the hub, in a thread of their own so that the game does not wait for it.
//! The author of the command gets its answer when the hub has answered.

use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::{Mutex, PoisonError};

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use bevy_renet::RenetServer;
use rustcraft_protocol::api::Profile;
use shared::messages::{FullChatMessage, ServerToClientMessage};
use shared::utils::unix_time_ms;

use super::commands::SERVER_AUTHOR;
use super::extensions::SendGameMessageExtension;
use super::handlers::run_admin;
use super::permissions::{Admin, Dismissals, Permissions};
use crate::hub::fetch_profile;
use crate::init::ServerLobby;

/// Asks a hub for the account of a name: `None` when it knows none, an error when it cannot be
/// reached.
pub type ProfileLookup = fn(hub: &str, name: &str) -> Result<Option<Profile>, String>;

static LOOKUP: Mutex<ProfileLookup> = Mutex::new(fetch_profile);

/// Replaces the question to the hub of the servers started after it, for the tests: they have
/// no hub to ask.
pub fn set_profile_lookup(lookup: ProfileLookup) {
    *LOOKUP.lock().unwrap_or_else(PoisonError::into_inner) = lookup;
}

/// A command waiting for the account of its player.
struct Pending {
    author: ClientId,
    admin: Admin,
    name: String,
    /// In a mutex only for the resource to be shared between threads: nobody else reads it.
    answer: Mutex<Receiver<Result<Option<Profile>, String>>>,
}

/// The commands waiting for the hub.
#[derive(Resource)]
pub struct ProfileLookups {
    lookup: ProfileLookup,
    pending: Vec<Pending>,
}

impl Default for ProfileLookups {
    fn default() -> Self {
        Self {
            lookup: *LOOKUP.lock().unwrap_or_else(PoisonError::into_inner),
            pending: Vec::new(),
        }
    }
}

impl ProfileLookups {
    /// Asks the hub at `hub` for the account named `name`, to run `admin` for `author` with it.
    pub(super) fn start(&mut self, hub: &str, author: ClientId, admin: Admin, name: &str) {
        let (send, answer) = channel();
        let (lookup, hub, asked) = (self.lookup, hub.to_string(), name.to_string());
        std::thread::spawn(move || {
            // The game stopped caring when the receiver is gone
            let _ = send.send(lookup(&hub, &asked));
        });
        self.pending.push(Pending {
            author,
            admin,
            name: name.to_string(),
            answer: Mutex::new(answer),
        });
    }
}

fn say(server: &mut RenetServer, client_id: ClientId, content: String) {
    let message = FullChatMessage {
        author: SERVER_AUTHOR.to_string(),
        content,
        timestamp: unix_time_ms(),
    };
    server.send_game_message(client_id, ServerToClientMessage::ChatMessage(message));
}

/// Runs the commands whose hub answered, and tells their authors, if they are still there.
pub fn finish_lookups_system(
    mut lookups: ResMut<ProfileLookups>,
    mut permissions: ResMut<Permissions>,
    mut dismissals: ResMut<Dismissals>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
) {
    let mut waiting = Vec::new();
    for pending in std::mem::take(&mut lookups.pending) {
        let received = pending
            .answer
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .try_recv();
        let found = match received {
            Ok(found) => found,
            Err(TryRecvError::Empty) => {
                waiting.push(pending);
                continue;
            }
            Err(TryRecvError::Disconnected) => Err("the question to the hub was lost".into()),
        };
        let answer = match found {
            Ok(Some(profile)) => run_admin(
                &mut server,
                &lobby,
                (&mut permissions, &mut dismissals),
                pending.admin,
                Some(profile.id),
            ),
            Ok(None) => format!("The hub does not know a player named {}", pending.name),
            Err(err) => format!("Could not ask the hub for {}: {err}", pending.name),
        };
        if lobby.players.contains_key(&pending.author) {
            say(&mut server, pending.author, answer);
        }
    }
    lookups.pending = waiting;
}
