//! What a hosted world does with what its hub tells of its members: the lists follow them, the
//! players removed are dismissed, and a world the hub no longer knows stops.

use std::sync::mpsc::TryRecvError;
use std::sync::Mutex;

use bevy::prelude::*;
use bevy_renet::RenetServer;

use super::permissions::{Dismissals, Permissions, REMOVED};
use crate::hub::{RealmEvent, RealmWatch};
use crate::init::ServerLobby;
use crate::shutdown::save_and_exit;
use crate::world::data::ServerWorldMap;
use crate::world::save::SaveRequestEvent;

/// What the thread that reads the members sends, taken once per tick. In a mutex only for the
/// resource to be shared between threads: nobody else reads it.
#[derive(Resource)]
pub struct RealmEvents(Mutex<std::sync::mpsc::Receiver<RealmEvent>>);

impl RealmEvents {
    pub fn new(watch: RealmWatch) -> Self {
        Self(Mutex::new(watch.events))
    }
}

/// Applies the last members the hub sent, and dismisses the connected players that are no
/// longer let in. A hub that does not know the world any more stops the server: it saves, then
/// exits, and the hub starts it again, with a new token, if it must.
pub fn realm_system(
    events: Res<RealmEvents>,
    mut permissions: ResMut<Permissions>,
    lobby: Res<ServerLobby>,
    mut dismissals: ResMut<Dismissals>,
    mut server: ResMut<RenetServer>,
    world_map: Res<ServerWorldMap>,
    mut save: MessageWriter<SaveRequestEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    let mut latest = None;
    loop {
        let event = events
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .try_recv();
        match event {
            Ok(RealmEvent::Config(config)) => latest = Some(config),
            Ok(RealmEvent::Gone) => {
                error!("The hub no longer knows this hosted world or refuses its token: stopping");
                save_and_exit(&world_map, &mut save, &mut exit);
                return;
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
        }
    }
    let Some(config) = latest else {
        return;
    };
    permissions.follow(&config);
    let removed: Vec<_> = lobby
        .players
        .iter()
        .filter(|(_, player)| permissions.admission(&player.name, player.account).is_err())
        .map(|(id, _)| *id)
        .collect();
    for id in removed {
        dismissals.dismiss(&mut server, id, REMOVED.to_string());
    }
}
