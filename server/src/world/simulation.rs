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
    world::{world_position_to_chunk_position, Chunk, ChunkMap, ServerMob, WorldSeed},
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crate::{
    init::ServerTime,
    network::extensions::SendGameMessageExtension,
    world::{
        chunk_files::{read_chunk, CHUNKS_DIR},
        container_files::{insert_with_contents, read_chunk_contents, ChunkContents},
        data::ServerWorldMap,
        generation::WorldGenerator,
    },
};

use super::broadcast_world::active_chunks;
use super::pool::generation_pool;

#[derive(Message, Debug)]
pub struct PlayerInputsEvent {
    pub client_id: ClientId,
    pub input: PlayerFrameInput,
}

/// Chunks being loaded or generated in the background at most: a few ticks of work for the
/// server pool, so that the chunks nearest to a player who moved do not wait behind many others.
const CHUNKS_IN_FLIGHT: usize = 8192;

/// Chunks made on one task of the pool.
const CHUNKS_PER_TASK: usize = 16;

/// A chunk that was not loaded, read from its file or generated with the animals born in it.
enum Made {
    Read(Chunk, ChunkContents),
    Generated(Chunk, Vec<ServerMob>),
}

/// The chunk at `chunk_pos`: from its file in the folder of the world `saved`, else generated.
fn make_chunk(chunk_pos: IVec3, saved: Option<&Path>, generator: &WorldGenerator) -> Made {
    if let Some(dir) = saved {
        if let Some(chunk) = read_chunk(&dir.join(CHUNKS_DIR), chunk_pos) {
            return Made::Read(chunk, read_chunk_contents(dir, chunk_pos));
        }
    }
    Made::Generated(
        generator.generate_chunk(chunk_pos),
        generator.chunk_animals(chunk_pos),
    )
}

/// Inserts a chunk just made. A generated chunk is dirty: it has to be saved, as inserting it
/// is not logged as a block change; but not one made of a single kind of cell, such as the air
/// above the ground, which is generated again at once. A chunk read from its file is not dirty.
/// The animals born with a generated chunk are added to `born`.
fn insert_made(
    chunks: &mut ChunkMap,
    dirty_chunks: &mut HashSet<IVec3>,
    born: &mut Vec<ServerMob>,
    chunk_pos: IVec3,
    made: Made,
) {
    match made {
        Made::Read(chunk, contents) => insert_with_contents(chunks, chunk_pos, chunk, contents),
        Made::Generated(chunk, animals) => {
            if chunk.uniform().is_none() {
                dirty_chunks.insert(chunk_pos);
            }
            chunks.insert_chunk(chunk_pos, chunk);
            born.extend(animals);
        }
    }
}

/// Loads the chunks within `radius` of each of `players` that are missing, at once and on the
/// calling thread: from their files in the folder of the world `saved`, else generated. The
/// animals born with the generated chunks are added to `born`.
#[allow(clippy::too_many_arguments)]
pub(super) fn generate_missing_chunks<'a>(
    players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
    radius: impl Fn(PlayerId) -> i32,
    chunks: &mut ChunkMap,
    dirty_chunks: &mut HashSet<IVec3>,
    born: &mut Vec<ServerMob>,
    saved: Option<&Path>,
    generator: &WorldGenerator,
) {
    let missing: Vec<IVec3> = active_chunks(players, radius)
        .filter(|c| !chunks.is_chunk_loaded(*c))
        .collect();
    for c in missing {
        let made = make_chunk(c, saved, generator);
        insert_made(chunks, dirty_chunks, born, c, made);
    }
}

/// Loads and generates the chunks within the reach of the players on the server pool, in the
/// background: a tick inserts the chunks made since the last one and asks for the next, and
/// the pool goes on between the ticks.
pub struct ChunkLoader {
    generator: Arc<WorldGenerator>,
    /// The chunks asked for and not inserted yet.
    in_flight: HashSet<IVec3>,
    sender: Sender<Vec<(IVec3, Made)>>,
    receiver: Receiver<Vec<(IVec3, Made)>>,
    /// Most chunks in flight.
    most_in_flight: usize,
}

impl ChunkLoader {
    pub fn new(seed: u32) -> Self {
        Self::with_most_in_flight(seed, CHUNKS_IN_FLIGHT)
    }

    fn with_most_in_flight(seed: u32, most_in_flight: usize) -> Self {
        let (sender, receiver) = channel();
        Self {
            generator: Arc::new(WorldGenerator::new(seed)),
            in_flight: HashSet::new(),
            sender,
            receiver,
            most_in_flight,
        }
    }

    pub fn generator(&self) -> &WorldGenerator {
        &self.generator
    }

    /// Inserts the chunks made since the last call, but those loaded meanwhile, by
    /// `generate_missing_chunks`: they may have changed since. The animals born with the
    /// generated ones are added to `born`.
    pub(super) fn insert_made(
        &mut self,
        chunks: &mut ChunkMap,
        dirty_chunks: &mut HashSet<IVec3>,
        born: &mut Vec<ServerMob>,
    ) {
        for made in self.receiver.try_iter() {
            for (c, made) in made {
                self.in_flight.remove(&c);
                if !chunks.is_chunk_loaded(c) {
                    insert_made(chunks, dirty_chunks, born, c, made);
                }
            }
        }
    }

    /// Asks for the missing chunks within `radius` of each of `players`, the nearest to each
    /// player first, past the first `asked` in that order (`active_chunks`), which are known to
    /// be loaded or in flight: those of a large render distance would otherwise be looked at
    /// again on every tick until the last is loaded. `asked` then counts those asked for now.
    /// Says whether every chunk within reach is loaded.
    pub(super) fn ask<'a>(
        &mut self,
        players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
        radius: impl Fn(PlayerId) -> i32,
        chunks: &ChunkMap,
        saved: Option<&Path>,
        asked: &mut usize,
    ) -> bool {
        let mut active = active_chunks(players, radius).skip(*asked);
        let mut missing = Vec::new();
        let mut reached_all = false;
        while self.in_flight.len() + missing.len() < self.most_in_flight {
            let Some(c) = active.next() else {
                reached_all = true;
                break;
            };
            *asked += 1;
            if !chunks.is_chunk_loaded(c) && !self.in_flight.contains(&c) {
                missing.push(c);
            }
        }
        for batch in missing.chunks(CHUNKS_PER_TASK) {
            self.in_flight.extend(batch);
            let batch = batch.to_vec();
            let generator = self.generator.clone();
            let sender = self.sender.clone();
            let saved = saved.map(Path::to_path_buf);
            generation_pool()
                .spawn(async move {
                    let made = batch
                        .into_iter()
                        .map(|c| (c, make_chunk(c, saved.as_deref(), &generator)))
                        .collect();
                    // the loader is gone with its world: nobody waits for these
                    let _ = sender.send(made);
                })
                .detach();
        }
        reached_all && self.in_flight.is_empty()
    }
}

/// The chunk and the render distance of each player, with how many of the chunks within their
/// reach, in the order they are reached, are all loaded or in flight, and whether they are all
/// loaded. The server keeps the chunks within reach: while the players stay in the same
/// chunks, the hundreds of thousands of chunks of a large render distance are not looked at
/// again.
#[derive(Default)]
pub struct LoadedAround {
    around: Vec<(PlayerId, IVec3, i32)>,
    asked: usize,
    complete: bool,
}

pub fn handle_player_inputs_system(
    mut events: MessageReader<PlayerInputsEvent>,
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
    mut loader: Local<Option<ChunkLoader>>,
    mut loaded_around: Local<LoadedAround>,
) {
    // The noises are built once per seed, not once per chunk
    if loader.as_ref().is_none_or(|l| l.generator.seed() != seed.0) {
        *loader = Some(ChunkLoader::new(seed.0));
        *loaded_around = LoadedAround::default();
    }
    let loader = loader.as_mut().unwrap();
    let mut rng = rand::thread_rng();
    let world_map = world_map.as_mut();
    let players = &mut world_map.players;
    let chunks = &mut world_map.chunks;
    let dropped_items = &mut world_map.dropped_items;
    let dirty_chunks = &mut world_map.dirty_chunks;
    let render_distances = &world_map.render_distances;
    let saved = world_map.saved.as_deref();
    let spawn = world_map.spawn;
    // The animals born with the chunks generated, and those hatched from spawn eggs
    let mut born = Vec::new();

    loader.insert_made(chunks, dirty_chunks, &mut born);
    let mut around: Vec<_> = players
        .values()
        .map(|player| {
            let chunk = world_position_to_chunk_position(player.position);
            (player.id, chunk, render_distances.get(player.id))
        })
        .collect();
    around.sort_by_key(|&(id, ..)| id);
    if loaded_around.around != around {
        *loaded_around = LoadedAround {
            around,
            ..default()
        };
        // A player who joined or came into another chunk gets the chunks next to it in this
        // tick, the rest of its render distance as it is made
        generate_missing_chunks(
            &*players,
            |_| 1,
            chunks,
            dirty_chunks,
            &mut born,
            saved,
            loader.generator(),
        );
    }
    if !loaded_around.complete {
        // In the order of their ids, which the count of the chunks asked for relies on
        let mut by_id: Vec<_> = players.iter().collect();
        by_id.sort_by_key(|&(id, _)| *id);
        loaded_around.complete = loader.ask(
            by_id,
            |id| render_distances.get(id),
            chunks,
            saved,
            &mut loaded_around.asked,
        );
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
        for stack in outcome.dropped {
            dropped_items.drop_from_player(stack, player);
        }
        born.extend(
            outcome
                .spawned
                .iter()
                .map(|mob| ServerMob::new(mob.kind, mob.position)),
        );
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
            &mut born,
            saved,
            loader.generator(),
        );
    }

    for mob in born {
        crate::mob::add_mob(&mut world_map.mobs, &mut world_map.changed_mobs, mob);
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
/// chest or furnace; the others only see it move.
pub fn broadcast_player_updates_system(
    world_map: Res<ServerWorldMap>,
    mut server: ResMut<RenetServer>,
) {
    for player in world_map.players.values() {
        let update = |inventory, open_chest, open_furnace| {
            shared::messages::ServerToClientMessage::PlayerUpdate(PlayerUpdateEvent {
                id: player.id,
                movement: player.movement_state(),
                orientation: player.camera_transform.rotation,
                last_ack_time: player.last_input_processed,
                inventory,
                open_chest,
                open_furnace,
                health: player.health,
                game_mode: player.game_mode,
                death_cause: player.death_cause,
            })
        };
        let open_chest = player
            .inventory
            .open_chest
            .and_then(|chest| world_map.chunks.container(chest).cloned().map(Box::new));
        let open_furnace = player
            .inventory
            .open_furnace
            .and_then(|furnace| world_map.chunks.furnace(furnace).cloned());
        server.broadcast_game_message_except(player.id, update(None, None, None));
        server.send_game_message(
            player.id,
            update(Some(player.inventory.clone()), open_chest, open_furnace),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::broadcast_world::get_all_active_chunks;
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
        // and one high in the air
        let air = IVec3::new(0, 20, 0);
        players.insert(
            2,
            Player::new(
                2,
                "q".into(),
                (air * shared::CHUNK_SIZE).as_vec3() + 8.,
                Transform::default(),
            ),
        );
        let mut chunks = ChunkMap::default();
        chunks.insert_chunk(IVec3::ZERO, shared::world::Chunk::new());
        let mut dirty = std::collections::HashSet::new();
        generate_missing_chunks(
            &players,
            |_| 2,
            &mut chunks,
            &mut dirty,
            &mut Vec::new(),
            None,
            &WorldGenerator::new(1),
        );

        assert!(!dirty.contains(&IVec3::ZERO));
        // The bedrock at the bottom of the chunk
        assert!(dirty.contains(&IVec3::X));
        // Air only
        assert!(chunks.is_chunk_loaded(air));
        assert!(!dirty.contains(&air));
        let active = get_all_active_chunks(&players, |_| 2);
        assert!(active.iter().all(|&c| chunks.is_chunk_loaded(c)));
        for c in active.into_iter().filter(|&c| c != IVec3::ZERO) {
            let uniform = chunks.chunk(c).unwrap().uniform().is_some();
            assert_eq!(dirty.contains(&c), !uniform, "{c}");
        }
    }

    /// Inserts what `loader` makes until nothing is in flight.
    fn insert_all_made(
        loader: &mut ChunkLoader,
        chunks: &mut ChunkMap,
        dirty: &mut HashSet<IVec3>,
    ) {
        let start = std::time::Instant::now();
        while !loader.in_flight.is_empty() {
            assert!(start.elapsed().as_secs() < 60, "the chunks are never made");
            loader.insert_made(chunks, dirty, &mut Vec::new());
            std::thread::yield_now();
        }
    }

    fn player_at_the_origin() -> std::collections::HashMap<PlayerId, Player> {
        let mut players = std::collections::HashMap::new();
        players.insert(
            1,
            Player::new(1, "p".into(), Vec3::new(8., 8., 8.), Transform::default()),
        );
        players
    }

    #[test]
    fn the_loader_asks_for_the_nearest_chunks_first_and_no_more_than_it_may_have_in_flight() {
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        let mut dirty = HashSet::new();
        let mut loader = ChunkLoader::with_most_in_flight(1, 7);
        let loaded = |chunks: &ChunkMap| chunks.chunk_positions().collect::<Vec<_>>();
        let mut asked = 0;

        assert!(!loader.ask(&players, |_| 2, &chunks, None, &mut asked));
        // The chunk of the player and its six face neighbours
        assert_eq!(asked, 7);
        assert!(!loader.ask(&players, |_| 2, &chunks, None, &mut asked));
        assert_eq!(asked, 7, "no room is left");
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        assert_eq!(loaded(&chunks).len(), 7);
        assert!(loaded(&chunks).iter().all(|c| c.abs().element_sum() <= 1));

        loader.ask(&players, |_| 2, &chunks, None, &mut asked);
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        assert_eq!(
            loaded(&chunks).len(),
            14,
            "the next ones are asked for where the last stopped"
        );
        assert!(loaded(&chunks).iter().all(|c| c.abs().max_element() <= 1));

        let within = active_chunks(&players, |_| 2).count();
        for _ in 0..within {
            if loader.ask(&players, |_| 2, &chunks, None, &mut asked) {
                break;
            }
            insert_all_made(&mut loader, &mut chunks, &mut dirty);
        }
        assert_eq!(loaded(&chunks).len(), within);
    }

    #[test]
    fn the_chunks_known_to_be_loaded_are_skipped_and_counted() {
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        let mut dirty = HashSet::new();
        let within = active_chunks(&players, |_| 2).count();
        let mut loader = ChunkLoader::new(1);

        let mut asked = 0;
        assert!(!loader.ask(&players, |_| 2, &chunks, None, &mut asked));
        assert_eq!(asked, within);
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        // A chunk among the first, removed, is not looked for again
        chunks.remove_chunk(IVec3::ZERO);
        assert!(loader.ask(&players, |_| 2, &chunks, None, &mut asked));
        assert_eq!(asked, within);
        assert_eq!(chunks.chunk_positions().count(), within - 1);

        let mut asked = 0;
        assert!(!loader.ask(&players, |_| 2, &chunks, None, &mut asked));
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        assert!(chunks.is_chunk_loaded(IVec3::ZERO));
        assert!(loader.ask(&players, |_| 2, &chunks, None, &mut asked));
    }

    #[test]
    fn a_chunk_loaded_while_it_was_in_flight_is_kept() {
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        let mut dirty = HashSet::new();
        let mut loader = ChunkLoader::new(1);

        loader.ask(&players, |_| 1, &chunks, None, &mut 0);
        // as by `generate_missing_chunks`, then changed by a player
        let mut changed = Chunk::new();
        changed.set(
            IVec3::ONE,
            shared::world::BlockData::new(
                shared::world::BlockId::Dirt,
                shared::world::BlockDirection::Front,
            ),
        );
        chunks.insert_chunk(IVec3::ZERO, changed.clone());
        insert_all_made(&mut loader, &mut chunks, &mut dirty);

        assert_eq!(chunks.chunk(IVec3::ZERO), Some(&changed));
        assert_eq!(chunks.chunk_positions().count(), 27);
    }

    #[test]
    fn a_saved_chunk_is_read_from_its_file_with_its_chests_and_furnaces_and_is_not_dirty() {
        use crate::world::chunk_files::write_chunk;
        use crate::world::container_files::{write_containers, CONTAINERS_DIR, FURNACES_DIR};
        use shared::world::{BlockData, BlockDirection, BlockId, Container, Furnace};

        let dir = tempfile::tempdir().unwrap();
        let mut saved = Chunk::new();
        saved.set(
            IVec3::new(1, 2, 3),
            BlockData::new(BlockId::Chest, BlockDirection::Front),
        );
        saved.set(
            IVec3::new(4, 5, 6),
            BlockData::new(BlockId::LitFurnace, BlockDirection::Back),
        );
        let burning = Furnace {
            burn_left: 50,
            burn_total: 1600,
            ..Default::default()
        };
        let furnaces_dir = dir.path().join(FURNACES_DIR);
        std::fs::create_dir_all(&furnaces_dir).unwrap();
        write_containers(
            &furnaces_dir,
            IVec3::ZERO,
            &vec![(IVec3::new(4, 5, 6), burning.clone())],
        )
        .unwrap();
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
            &mut Vec::new(),
            Some(dir.path()),
            &WorldGenerator::new(1),
        );

        assert_eq!(chunks.chunk(IVec3::ZERO), Some(&saved));
        assert_eq!(chunks.container(IVec3::new(1, 2, 3)), Some(&held));
        assert_eq!(chunks.furnace(IVec3::new(4, 5, 6)), Some(&burning));
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
