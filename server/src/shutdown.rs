use crate::world::data::ServerWorldMap;
use crate::world::save::SaveRequestEvent;
use bevy::prelude::*;
use shared::GameServerConfig;
use std::sync::atomic::{AtomicBool, Ordering};

/// Set by the signal handler of the `server` binary: the process was asked to stop.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Records a request to stop and tells whether one was already pending, which a second signal
/// turns into an immediate exit.
pub fn request_shutdown() -> bool {
    SHUTDOWN_REQUESTED.swap(true, Ordering::SeqCst)
}

/// Saves the world and the connected players, then ends the app, once the process was asked
/// to stop. A solo world keeps its own exit path, when its player leaves.
///
/// Runs before `save_world_system` so that the same tick hands the save to the writer, which
/// the app waits for when it is dropped.
pub fn shutdown_system(
    config: Res<GameServerConfig>,
    world_map: Res<ServerWorldMap>,
    mut save: MessageWriter<SaveRequestEvent>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.is_solo || !SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        return;
    }
    info!("Stopping: saving the world and the players");
    save_and_exit(&world_map, &mut save, &mut exit);
}

/// Asks for the world and the connected players to be saved, and the app to end.
pub fn save_and_exit(
    world_map: &ServerWorldMap,
    save: &mut MessageWriter<SaveRequestEvent>,
    exit: &mut MessageWriter<AppExit>,
) {
    save.write(SaveRequestEvent::World);
    for &id in world_map.players.keys() {
        save.write(SaveRequestEvent::Player(id));
    }
    exit.write(AppExit::Success);
}
