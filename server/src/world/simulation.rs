use bevy::prelude::*;
use bevy_renet::{renet::ClientId, RenetServer};
use shared::{
    messages::{PlayerFrameInput, PlayerId, PlayerUpdateEvent},
    players::{
        blocks::CallerType,
        game_mode::{fall_damage, DeathCause, MAX_HEALTH, REGENERATION_TICKS},
        movement::FALL_LIMIT,
        simulation::simulate_player_actions,
        Player,
    },
    world::{world_position_to_chunk_position, Chunk, ChunkMap, WorldSeed},
};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::{
    init::ServerTime,
    network::extensions::SendGameMessageExtension,
    world::{
        chunk_files::{read_chunk, CHUNKS_DIR},
        container_files::{
            insert_with_chests, read_chunk_containers, ChunkContainers, CONTAINERS_DIR,
        },
        data::ServerWorldMap,
        generation::{generate_chunk, WorldGenerator},
    },
};

use super::broadcast_world::get_all_active_chunks;
use super::pool::server_pool;

#[derive(Message, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

/// How long the render distances of the players may take to be loaded or generated in a tick, at
/// most: a joining player would otherwise freeze the server for its whole render distance at
/// once. The chunks of one kind of cell, most of a large render distance, take next to nothing:
/// a count of chunks would hold them back.
const CHUNK_LOADING_PER_TICK: Duration = Duration::from_millis(20);

/// Chunks loaded or generated together, between two looks at the time spent.
const CHUNKS_PER_BATCH: usize = 512;

/// Chunks made on one task of the pool: fewer are made on the tick's thread.
const CHUNKS_PER_TASK: usize = 16;

/// A chunk that was not loaded, read from its file or generated.
enum Made {
    Read(Chunk, ChunkContainers),
    Generated(Chunk),
}

/// The chunk at `chunk_pos`: from its file in the folder of the world `saved`, else generated.
fn make_chunk(chunk_pos: IVec3, saved: Option<&Path>, seed: u32) -> Made {
    if let Some(dir) = saved {
        if let Some(chunk) = read_chunk(&dir.join(CHUNKS_DIR), chunk_pos) {
            let chests = read_chunk_containers(&dir.join(CONTAINERS_DIR), chunk_pos);
            return Made::Read(chunk, chests);
        }
    }
    Made::Generated(generate_chunk(chunk_pos, seed))
}

/// Makes `chunks` on the threads of the server pool, in their order.
fn make_chunks(chunks: &[IVec3], saved: Option<&Path>, seed: u32) -> Vec<Made> {
    if chunks.len() <= CHUNKS_PER_TASK {
        return chunks.iter().map(|&c| make_chunk(c, saved, seed)).collect();
    }
    server_pool()
        .scope(|scope| {
            for batch in chunks.chunks(CHUNKS_PER_TASK) {
                scope.spawn(async move {
                    batch
                        .iter()
                        .map(|&c| make_chunk(c, saved, seed))
                        .collect::<Vec<_>>()
                });
            }
        })
        .into_iter()
        .flatten()
        .collect()
}

/// Loads the chunks within `radius` of each of `players`, at most `budget` of them and for at most
/// about `CHUNK_LOADING_PER_TICK`, the nearest to each player first: from their files in the
/// folder of the world `saved`, else generated.
/// A generated chunk is dirty: it has to be saved, as inserting it is not logged as a block
/// change; but not one made of a single kind of cell, such as the air above the ground or the
/// stone below it, which is generated again at once. A chunk read from its file is not dirty.
/// Says whether every chunk within reach is loaded now.
pub(super) fn generate_missing_chunks<'a>(
    players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
    radius: impl Fn(PlayerId) -> i32,
    chunks: &mut ChunkMap,
    dirty_chunks: &mut std::collections::HashSet<IVec3>,
    saved: Option<&Path>,
    generator: &WorldGenerator,
    budget: usize,
) -> bool {
    let start = Instant::now();
    let missing: Vec<IVec3> = get_all_active_chunks(players, radius)
        .into_iter()
        .filter(|c| !chunks.is_chunk_loaded(*c))
        .take(budget)
        .collect();
    if missing.is_empty() {
        return true;
    }
    debug!("Loading up to {} chunks", missing.len());
    for batch in missing.chunks(CHUNKS_PER_BATCH) {
        if start.elapsed() > CHUNK_LOADING_PER_TICK {
            return false;
        }
        for (c, made) in batch.iter().zip(make_chunks(batch, saved, generator.seed())) {
            match made {
                Made::Read(chunk, chests) => insert_with_chests(chunks, *c, chunk, chests),
                Made::Generated(chunk) => {
                    // Generating a chunk of one kind of cell again costs nothing: it is not saved
                    if chunk.uniform().is_none() {
                        dirty_chunks.insert(*c);
                    }
                    chunks.insert_chunk(*c, chunk);
                }
            }
        }
    }
    missing.len() < budget
}

/// The chunk and the render distance of each player, the last time every chunk within their
/// reach was loaded. The server keeps the chunks it loaded: while the players stay in the same
/// chunks, the tens of thousands of chunks of a large render distance are not looked at again.
#[derive(Default, PartialEq)]
pub struct LoadedAround(Vec<(PlayerId, IVec3, i32)>);

pub fn handle_player_inputs_system(
    mut events: MessageReader<PlayerInputsEvent>,
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
    mut generator: Local<Option<WorldGenerator>>,
    mut loaded_around: Local<LoadedAround>,
) {
    // The noises are built once per seed, not once per chunk
    if generator.as_ref().is_none_or(|g| g.seed() != seed.0) {
        *generator = Some(WorldGenerator::new(seed.0));
    }
    let generator = generator.as_ref().unwrap();
    let mut rng = rand::thread_rng();
    let world_map = world_map.as_mut();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;
    let dropped_items = &mut world_map.dropped_items;
    let dirty_chunks = &mut world_map.dirty_chunks;
    let render_distances = &world_map.render_distances;
    let saved = world_map.saved.as_deref();
    let spawn = world_map.spawn;

    let mut around: Vec<_> = players
        .values()
        .map(|player| {
            let chunk = world_position_to_chunk_position(player.position);
            (player.id, chunk, render_distances.get(player.id))
        })
        .collect();
    around.sort_by_key(|&(id, ..)| id);
    let around = LoadedAround(around);
    if *loaded_around != around
        && generate_missing_chunks(
            &*players,
            |id| render_distances.get(id),
            chunks,
            dirty_chunks,
            saved,
            generator,
            usize::MAX,
        )
    {
        *loaded_around = around;
    }

    for ev in events.read() {
        let Some(player) = players.get_mut(&ev.client_id) else {
            warn!("Ignoring inputs from unknown player {}", ev.client_id);
            continue;
        };

        let outcome = simulate_player_actions(player, chunks, &ev.input, CallerType::Server);
        for block in &outcome.broken {
            dropped_items.drop_block_items(block, &mut rng);
        }
        if let Some(fallen) = outcome.fallen {
            player.hurt(fall_damage(fallen), DeathCause::Fall);
        }
        if player.position.y < FALL_LIMIT {
            fall_out_of_the_world(player, spawn);
        }

        player.last_input_processed = ev.input.time_ms;

        // The simulation holds a player still while its chunk or the one below is missing, and
        // a falling player can cross a chunk within one batch of inputs. The chunks next to
        // the player's are enough: the rest of the render distance waits for the next tick.
        // The other players did not move
        generate_missing_chunks(
            [(&ev.client_id, &*player)],
            |_| 1,
            chunks,
            dirty_chunks,
            saved,
            generator,
            usize::MAX,
        );
    }
}

/// A player below `FALL_LIMIT` is taken back to `spawn`, and dies on the way in survival.
fn fall_out_of_the_world(player: &mut Player, spawn: Vec3) {
    player.hurt(player.health, DeathCause::Void);
    player.set_movement_state(shared::players::MovementState {
        position: spawn,
        is_flying: player.is_flying,
        ..Default::default()
    });
}

/// Every `REGENERATION_TICKS`, each living player gets a point of health back.
pub fn regenerate_health_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    if !time.0.is_multiple_of(REGENERATION_TICKS) {
        return;
    }
    for player in world_map.players.values_mut() {
        if !player.is_dead() && player.health < MAX_HEALTH {
            player.health += 1;
        }
    }
}

/// Sends every player's state to every client, once the tick has simulated the inputs and
/// given the players the items they picked up. A player is sent its own inventory and open
/// chest; the others only see it move.
pub fn broadcast_player_updates_system(
    world_map: Res<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
) {
    for player in world_map.players.values() {
        let update = |inventory, open_chest| {
            shared::messages::ServerToClientMessage::PlayerUpdate(PlayerUpdateEvent {
                id: player.id,
                movement: player.movement_state(),
                orientation: player.camera_transform.rotation,
                last_ack_time: player.last_input_processed,
                inventory,
                open_chest,
                health: player.health,
                game_mode: player.game_mode,
                death_cause: player.death_cause,
            })
        };
        let open_chest = player
            .inventory
            .open_chest
            .and_then(|chest| world_map.chunks.container(chest).cloned().map(Box::new));
        server.broadcast_game_message_except(player.id, update(None, None));
        server.send_game_message(
            player.id,
            update(Some(player.inventory.clone()), open_chest),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::players::game_mode::GameMode;

    #[test]
    fn a_player_out_of_the_world_comes_back_at_the_spawn_and_dies_in_survival() {
        let spawn = Vec3::new(0.5, 65.0, 0.5);
        for (mode, dies) in [(GameMode::Survival, true), (GameMode::Creative, false)] {
            let mut player = Player {
                game_mode: mode,
                position: Vec3::new(3.0, FALL_LIMIT - 1.0, 4.0),
                velocity: Vec3::new(0.0, -40.0, 0.0),
                ..Default::default()
            };
            fall_out_of_the_world(&mut player, spawn);
            assert_eq!(player.position, spawn);
            assert_eq!(player.velocity, Vec3::ZERO);
            assert_eq!(player.is_dead(), dies, "{mode:?}");
            if dies {
                assert_eq!(player.death_cause, Some(DeathCause::Void));
            }
        }
    }

    #[test]
    fn living_players_get_their_health_back_point_by_point() {
        let mut app = App::new();
        let mut world_map = ServerWorldMap::default();
        let mut hurt = Player::new(1, "hurt".into(), Vec3::ZERO, Transform::default());
        hurt.health = MAX_HEALTH - 2;
        let mut dead = Player::new(2, "dead".into(), Vec3::ZERO, Transform::default());
        dead.health = 0;
        world_map.players.insert(1, hurt);
        world_map.players.insert(2, dead);
        app.insert_resource(world_map)
            .insert_resource(ServerTime(1))
            .add_systems(Update, regenerate_health_system);
        let health = |app: &App, id| app.world().resource::<ServerWorldMap>().players[&id].health;

        app.update();
        assert_eq!(health(&app, 1), MAX_HEALTH - 2, "not a regeneration tick");
        for tick in [
            REGENERATION_TICKS,
            2 * REGENERATION_TICKS,
            3 * REGENERATION_TICKS,
        ] {
            app.world_mut().resource_mut::<ServerTime>().0 = tick;
            app.update();
        }
        assert_eq!(health(&app, 1), MAX_HEALTH);
        assert_eq!(health(&app, 2), 0);
    }

    #[test]
    fn generated_chunks_are_dirty_and_existing_ones_and_the_uniform_ones_are_not() {
        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        let mut chunks = ChunkMap::default();
        chunks.insert_chunk(IVec3::ZERO, shared::world::Chunk::new());
        let mut dirty = std::collections::HashSet::new();

        generate_missing_chunks(
            &players,
            |_| 2,
            &mut chunks,
            &mut dirty,
            None,
            &WorldGenerator::new(1),
            usize::MAX,
        );

        assert!(!dirty.contains(&IVec3::ZERO));
        // The bedrock at the bottom of the chunk
        assert!(dirty.contains(&IVec3::X));
        // Stone only
        assert!(chunks.is_chunk_loaded(IVec3::NEG_Y));
        assert!(!dirty.contains(&IVec3::NEG_Y));
        let active = get_all_active_chunks(&players, |_| 2);
        assert!(active.iter().all(|&c| chunks.is_chunk_loaded(c)));
        for c in active.into_iter().filter(|&c| c != IVec3::ZERO) {
            let uniform = chunks.chunk(c).unwrap().uniform().is_some();
            assert_eq!(dirty.contains(&c), !uniform, "{c}");
        }
    }

    #[test]
    fn a_tick_generates_its_budget_of_chunks_nearest_first() {
        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        let mut chunks = ChunkMap::default();
        let mut dirty = std::collections::HashSet::new();
        let generator = WorldGenerator::new(1);

        let loaded = |chunks: &ChunkMap| chunks.chunk_positions().collect::<Vec<_>>();
        assert!(!generate_missing_chunks(
            &players,
            |_| 2,
            &mut chunks,
            &mut dirty,
            None,
            &generator,
            7
        ));
        // The chunk of the player and its six face neighbours
        assert_eq!(loaded(&chunks).len(), 7);
        assert!(loaded(&chunks).iter().all(|c| c.abs().element_sum() <= 1));

        generate_missing_chunks(
            &players,
            |_| 2,
            &mut chunks,
            &mut dirty,
            None,
            &generator,
            7,
        );
        assert_eq!(
            loaded(&chunks).len(),
            14,
            "the next tick goes on where the last stopped"
        );
        assert!(loaded(&chunks).iter().all(|c| c.abs().max_element() <= 1));
        assert!(generate_missing_chunks(
            &players,
            |_| 2,
            &mut chunks,
            &mut dirty,
            None,
            &generator,
            200
        ));
    }

    #[test]
    fn a_saved_chunk_is_read_from_its_file_with_its_chests_and_is_not_dirty() {
        use crate::world::chunk_files::write_chunk;
        use crate::world::container_files::write_containers;
        use shared::world::{BlockData, BlockDirection, BlockId, Container};

        let dir = tempfile::tempdir().unwrap();
        let mut saved = Chunk::new();
        saved.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Chest, BlockDirection::Front),
        );
        let chunks_dir = dir.path().join(CHUNKS_DIR);
        let containers_dir = dir.path().join(CONTAINERS_DIR);
        std::fs::create_dir_all(&chunks_dir).unwrap();
        std::fs::create_dir_all(&containers_dir).unwrap();
        write_chunk(&chunks_dir, IVec3::ZERO, &saved).unwrap();
        let mut held = Container::default();
        held.slots[3] = Some(shared::world::ItemStack {
            item_id: shared::world::ItemId::Dirt,
            item_type: shared::world::ItemId::Dirt.get_default_type(),
            nb: 5,
        });
        write_containers(
            &containers_dir,
            IVec3::ZERO,
            &vec![(IVec3::new(1, 2, 3), held.clone())],
        )
        .unwrap();

        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        let mut chunks = ChunkMap::default();
        let mut dirty = std::collections::HashSet::new();
        generate_missing_chunks(
            &players,
            |_| 1,
            &mut chunks,
            &mut dirty,
            Some(dir.path()),
            &WorldGenerator::new(1),
            usize::MAX,
        );

        assert_eq!(chunks.chunk(IVec3::ZERO), Some(&saved));
        assert_eq!(chunks.container(IVec3::new(1, 2, 3)), Some(&held));
        assert!(
            !dirty.contains(&IVec3::ZERO),
            "a chunk read is saved already"
        );
        assert_eq!(
            chunks.chunk_positions().count(),
            27,
            "the chunks without a file are generated"
        );
    }
}
