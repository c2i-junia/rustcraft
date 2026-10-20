//! Forgets the chunks that no player needs any more: the server would otherwise keep every chunk
//! a player ever came near, tens of thousands more at each walk with a large render distance.

use bevy::prelude::*;
use shared::world::{chunk_in_reach, vertical_render_distance, world_position_to_chunk_position};
use shared::{TICKS_PER_SECOND, UNLOAD_MARGIN};

use crate::init::ServerTime;
use crate::world::data::ServerWorldMap;
use crate::world::writer::WorldWriter;

/// Game ticks between two looks for the chunks to forget: 5 seconds.
const UNLOAD_INTERVAL_TICKS: u64 = 5 * TICKS_PER_SECOND;

/// How many chunks beyond its render distance a player keeps its chunks on the server: more than
/// its client keeps (`UNLOAD_MARGIN`), so that a chunk is forgotten once no client has it, and
/// a player who turns back does not have the chunks read again at once.
const KEPT_MARGIN: i32 = UNLOAD_MARGIN + 2;

/// Every `UNLOAD_INTERVAL_TICKS`, forgets the chunks beyond the reach of every player. A chunk
/// that changed since its last save stays until a save writes it; and none is forgotten while
/// the writer has saves to write, as its file could be read back before it is written.
///
/// Runs after the save, which is after the broadcast: the blocks that changed in the tick are
/// in `dirty_chunks` then.
pub fn unload_far_chunks_system(
    mut world_map: ResMut<ServerWorldMap>,
    writer: Res<WorldWriter>,
    time: Res<ServerTime>,
    mut last_unload: Local<u64>,
) {
    if time.0.saturating_sub(*last_unload) < UNLOAD_INTERVAL_TICKS || !writer.is_idle() {
        return;
    }
    *last_unload = time.0;
    let world_map = world_map.as_mut();
    // A chunk the writer could not write is dirty again
    world_map.dirty_chunks.extend(writer.take_failed_chunks());
    let unloaded = unload_far_chunks(world_map);
    if unloaded > 0 {
        debug!("[{}] Unloaded {unloaded} chunks", world_map.name);
    }
}

/// Forgets the chunks of `world_map` beyond `KEPT_MARGIN` of the reach of every player but
/// those not saved yet, and says how many.
fn unload_far_chunks(world_map: &mut ServerWorldMap) -> usize {
    let reach: Vec<(IVec3, i32)> = world_map
        .players
        .values()
        .map(|player| {
            (
                world_position_to_chunk_position(player.position),
                world_map.render_distances.get(player.id),
            )
        })
        .collect();
    let far: Vec<IVec3> = world_map
        .chunks
        .chunk_positions()
        .filter(|chunk| !world_map.dirty_chunks.contains(chunk))
        .filter(|chunk| {
            !reach.iter().any(|&(center, distance)| {
                chunk_in_reach(
                    *chunk,
                    center,
                    distance + KEPT_MARGIN,
                    vertical_render_distance(distance) + KEPT_MARGIN,
                )
            })
        })
        .collect();
    for chunk in &far {
        world_map.chunks.remove_chunk(*chunk);
    }
    far.len()
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::sync::Mutex;

    use shared::players::Player;
    use shared::world::ChunkMap;
    use shared::CHUNK_SIZE;

    use super::*;

    const DISTANCE: i32 = 2;
    const KEPT: i32 = DISTANCE + KEPT_MARGIN;

    /// A world whose chunks span `-span..=span` along x, at y = 0, with a player at the origin
    /// whose render distance is `DISTANCE`.
    fn world(span: i32) -> ServerWorldMap {
        let mut map = ServerWorldMap {
            chunks: ChunkMap::default(),
            ..default()
        };
        for x in -span..=span {
            map.chunks
                .insert_chunk(IVec3::new(x, 0, 0), Default::default());
        }
        let player = Player::new(1, "PlayerOne".into(), Vec3::splat(0.5), default());
        map.render_distances
            .set(player.id, DISTANCE as u32, DISTANCE as u32);
        map.players.insert(player.id, player);
        map
    }

    fn loaded(map: &ServerWorldMap) -> Vec<i32> {
        let mut xs: Vec<i32> = map.chunks.chunk_positions().map(|c| c.x).collect();
        xs.sort();
        xs
    }

    #[test]
    fn the_chunks_beyond_the_reach_of_every_player_are_forgotten_but_those_not_saved() {
        let mut map = world(KEPT + 3);
        let unsaved = IVec3::new(-KEPT - 2, 0, 0);
        map.dirty_chunks.insert(unsaved);

        // Three chunks beyond the reach on each side, but the one not saved
        assert_eq!(unload_far_chunks(&mut map), 5);

        let mut kept: Vec<i32> = (-KEPT..=KEPT).collect();
        kept.insert(0, unsaved.x);
        assert_eq!(loaded(&map), kept);
    }

    #[test]
    fn a_second_player_keeps_the_chunks_around_it() {
        let mut map = world(KEPT + 3);
        let at = Vec3::new(((2 * KEPT + 1) * CHUNK_SIZE) as f32, 0.5, 0.5);
        let other = Player::new(2, "PlayerTwo".into(), at, default());
        map.render_distances
            .set(other.id, DISTANCE as u32, DISTANCE as u32);
        map.players.insert(other.id, other);

        unload_far_chunks(&mut map);

        // Its reach starts at KEPT + 1
        let kept: Vec<i32> = (-KEPT..=KEPT + 3).collect();
        assert_eq!(loaded(&map), kept);
    }

    #[test]
    fn no_chunk_is_forgotten_while_the_writer_has_saves_to_write_nor_between_two_looks() {
        let (release, wait) = mpsc::channel::<()>();
        let wait = Mutex::new(wait);
        let writer = WorldWriter::spawn_with(move |_| {
            wait.lock().unwrap().recv().unwrap();
            Vec::new()
        })
        .unwrap();
        let mut app = App::new();
        app.insert_resource(world(KEPT + 1))
            .insert_resource(ServerTime(UNLOAD_INTERVAL_TICKS))
            .add_systems(Update, unload_far_chunks_system);
        let count = |app: &App| {
            app.world()
                .resource::<ServerWorldMap>()
                .chunks
                .chunk_positions()
                .count() as i32
        };
        assert!(writer
            .submit(crate::world::writer::SaveJob {
                meta: crate::world::format::WorldMeta::new(
                    "w".into(),
                    shared::world::WorldSeed(1),
                    0,
                    Default::default()
                ),
                players: Vec::new(),
                chunks: Vec::new(),
            })
            .is_ok());
        app.insert_resource(writer);

        app.update();
        assert_eq!(count(&app), 2 * KEPT + 3);

        release.send(()).unwrap();
        while !app.world().resource::<WorldWriter>().is_idle() {
            std::thread::yield_now();
        }
        app.update();
        assert_eq!(count(&app), 2 * KEPT + 1);

        // A chunk that comes back far away waits for the next look
        let far = IVec3::new(KEPT + 1, 0, 0);
        app.world_mut()
            .resource_mut::<ServerWorldMap>()
            .chunks
            .insert_chunk(far, Default::default());
        app.world_mut().resource_mut::<ServerTime>().0 += UNLOAD_INTERVAL_TICKS - 1;
        app.update();
        assert_eq!(count(&app), 2 * KEPT + 2);
        app.world_mut().resource_mut::<ServerTime>().0 += 1;
        app.update();
        assert_eq!(count(&app), 2 * KEPT + 1);
    }
}
