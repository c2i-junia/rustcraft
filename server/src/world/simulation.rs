use bevy::prelude::*;
use bevy_renet::{renet::ClientId, RenetServer};
use rand::Rng;
use shared::{
    messages::{PlayerFrameInput, PlayerId, PlayerUpdateEvent},
    players::{
        blocks::CallerType,
        constants::EYE_HEIGHT,
        experience::smelting_experience,
        fire::{body_in_fire, body_in_lava, is_doused},
        game_mode::{fall_damage, DamageSource, REGENERATION_TICKS},
        movement::FALL_LIMIT,
        progress::Achievement,
        simulation::simulate_player_actions,
        Player,
    },
    world::{
        depths::DepthTier, liquid::eye_in_liquid, BlockId, Chunk, ChunkMap, ChunkReach, ServerMob,
        WeatherKind, WorldSeed,
    },
};
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use crate::{
    init::ServerTime,
    mob::{
        arrow::arrow,
        combat::{self, MobWorld},
    },
    network::extensions::SendGameMessageExtension,
    world::{
        chunk_files::{read_chunk, CHUNKS_DIR},
        container_files::{insert_with_contents, read_chunk_contents, ChunkContents},
        data::ServerWorldMap,
        dropped_items::random_points,
        generation::{WorldGeneration, WorldGenerator},
        portals::is_believable,
        weather::ServerWeather,
    },
};

use super::broadcast_world::{self, active_chunks, active_chunks_of};
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

/// A chunk that was not loaded, read from its file or generated with the animals born in it
/// and what its chests hold.
enum Made {
    Read(Chunk, ChunkContents),
    Generated(Chunk, Vec<ServerMob>, ChunkContents),
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
        ChunkContents {
            chests: generator.chunk_containers(chunk_pos),
            ..ChunkContents::default()
        },
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
        Made::Generated(chunk, animals, contents) => {
            if chunk.uniform().is_none() {
                dirty_chunks.insert(chunk_pos);
            }
            // only the chunks of the villages and the dungeons hold chests and furnaces when generated
            let holds = chunk.may_hold(|block| {
                block.is_some_and(|block| block.id.is_container() || block.id == BlockId::Furnace)
            });
            if holds {
                insert_with_contents(chunks, chunk_pos, chunk, contents);
            } else {
                chunks.insert_chunk(chunk_pos, chunk);
            }
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
    let missing: Vec<IVec3> = active_chunks(players, radius, &Default::default())
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
    /// The loader of a default world of `seed`.
    #[cfg(test)]
    pub fn new(seed: u32) -> Self {
        Self::with(WorldGenerator::new(seed))
    }

    /// The loader of the chunks `generator` makes.
    pub fn with(generator: WorldGenerator) -> Self {
        Self::with_most_in_flight(generator, CHUNKS_IN_FLIGHT)
    }

    fn with_most_in_flight(generator: WorldGenerator, most_in_flight: usize) -> Self {
        let (sender, receiver) = channel();
        Self {
            generator: Arc::new(generator),
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

    /// `ask`, for the reaches of `players` with the render distance `radius`, listed anew.
    #[cfg(test)]
    pub(super) fn ask_players<'a>(
        &mut self,
        players: impl IntoIterator<Item = (&'a PlayerId, &'a Player)>,
        radius: impl Fn(PlayerId) -> i32,
        chunks: &ChunkMap,
        saved: Option<&Path>,
        asked: &mut usize,
    ) -> bool {
        let reaches: Vec<ChunkReach> = players
            .into_iter()
            .map(|(&id, player)| ChunkReach::of_player(player, radius(id), chunks.portals()))
            .collect();
        let behind = broadcast_world::behind_portals(&reaches);
        self.ask(&reaches, behind, saved, chunks, asked)
    }

    /// Asks for the missing chunks within each of `reaches`, the nearest to each player first,
    /// then those around the twins, `behind_portals` (`active_chunks_of`), past the first
    /// `asked` in that order, which are known to be loaded or in flight: those of a large render
    /// distance would otherwise be looked at again on every tick until the last is loaded.
    /// `asked` then counts those asked for now. Says whether every chunk within reach is loaded.
    pub(super) fn ask(
        &mut self,
        reaches: &[ChunkReach],
        behind_portals: Arc<[IVec3]>,
        saved: Option<&Path>,
        chunks: &ChunkMap,
        asked: &mut usize,
    ) -> bool {
        let mut active = active_chunks_of(reaches, behind_portals).skip(*asked);
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

/// The reach of each player (its chunk, its render distance and the portals), with how many of
/// the chunks within their reach, in the order they are reached, are all loaded or in flight,
/// and whether they are all loaded. The server keeps the chunks within reach: while the players
/// stay in the same chunks and no portal changes, the hundreds of thousands of chunks of a
/// large render distance are not looked at again.
#[derive(Default)]
pub struct LoadedAround {
    around: Vec<(PlayerId, ChunkReach)>,
    /// The chunks around the twins (`behind_portals`), listed once per reach.
    behind_portals: Option<Arc<[IVec3]>>,
    asked: usize,
    complete: bool,
}

/// The generator of the world being played, with the plans it has laid out.
#[derive(Resource)]
pub struct LoadedGenerator(pub Arc<WorldGenerator>);

pub fn handle_player_inputs_system(
    mut commands: Commands,
    mut events: MessageReader<PlayerInputsEvent>,
    mut world_map: ResMut<ServerWorldMap>,
    seed: Res<WorldSeed>,
    generation: Option<Res<WorldGeneration>>,
    mut loader: Local<Option<ChunkLoader>>,
    mut loaded_around: Local<LoadedAround>,
) {
    // The noises are built once per seed, not once per chunk; the generation of the world does
    // not change while the server runs
    if loader.as_ref().is_none_or(|l| l.generator.seed() != seed.0) {
        let generator = match generation {
            Some(generation) => generation.generator(seed.0),
            None => WorldGenerator::new(seed.0),
        };
        let made = ChunkLoader::with(generator);
        commands.insert_resource(LoadedGenerator(made.generator.clone()));
        *loader = Some(made);
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
    let portal_shots = &mut world_map.portal_shots;
    // The animals born with the chunks generated, and those hatched from spawn eggs
    let mut born = Vec::new();

    loader.insert_made(chunks, dirty_chunks, &mut born);
    let mut around: Vec<_> = players
        .values()
        .map(|player| {
            let distance = render_distances.get(player.id);
            (
                player.id,
                ChunkReach::of_player(player, distance, chunks.portals()),
            )
        })
        .collect();
    around.sort_by_key(|(id, _)| *id);
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
        let LoadedAround {
            around,
            behind_portals,
            asked,
            complete,
        } = &mut *loaded_around;
        let reaches: Vec<ChunkReach> = around.iter().map(|(_, reach)| reach.clone()).collect();
        let behind = behind_portals
            .get_or_insert_with(|| broadcast_world::behind_portals(&reaches))
            .clone();
        *complete = loader.ask(&reaches, behind, saved, chunks, asked);
    }

    for ev in events.read() {
        let Some(player) = players.get_mut(&ev.client_id) else {
            warn!("Ignoring inputs from unknown player {}", ev.client_id);
            continue;
        };

        let mob_world = MobWorld {
            mobs: &mut world_map.mobs,
            changed: &mut world_map.changed_mobs,
            items: dropped_items,
            chunks,
            recharges: &mut world_map.recharges,
            feed_waits: &mut world_map.feed_waits,
            trades: &mut world_map.trades,
        };
        let rest = combat::interact(player, &ev.input, mob_world, &mut rng);
        let input = rest.as_ref().unwrap_or(&ev.input);
        crate::mob::fishing::use_rod(
            &mut world_map.fishing,
            player,
            input,
            &mut world_map.mobs,
            &mut world_map.changed_mobs,
            dropped_items,
            &mut rng,
        );
        // The shots of the input are aimed before its move
        let eyes = player.position + Vec3::Y * EYE_HEIGHT;
        let outcome = simulate_player_actions(player, chunks, input, CallerType::Server);
        let home = &mut world_map.home;
        home.read
            .extend(outcome.signs.iter().map(|&pos| (ev.client_id, pos)));
        home.slept
            .extend(outcome.bedrolls.iter().map(|&pos| (ev.client_id, pos)));
        home.framed
            .extend(outcome.frames.iter().map(|&pos| (ev.client_id, pos)));
        home.drawn
            .extend(outcome.drawn_maps.iter().map(|&slot| (ev.client_id, slot)));
        // Compost grows a sapling at once one time in two
        for &pos in &outcome.fertilized {
            if rng.gen_bool(0.5) {
                if let Some(sapling) = chunks
                    .get_block(pos)
                    .map(|block| block.id)
                    .filter(|&id| shared::world::growth::is_sapling(id))
                {
                    super::ticks::grow_tree(chunks, pos, sapling, rng.gen());
                }
            }
        }
        if !outcome.bedrolls.is_empty() {
            player.earn(Achievement::Bed);
        }
        player.learn_blocks(outcome.broken.iter().map(|b| b.block), &outcome.placed);
        player.mined(outcome.broken.len());
        for block in &outcome.broken {
            dropped_items.drop_block_items(block, &mut rng);
        }
        for &(item, nb) in &outcome.smelted {
            let points = random_points(smelting_experience(item) * nb, &mut rng);
            dropped_items.drop_orb(points, player.position, &mut rng);
        }
        for stack in outcome.dropped {
            dropped_items.drop_from_player(stack, player);
        }
        for stack in outcome.thrown {
            dropped_items.throw_from_player(stack, player);
        }
        for mob in &outcome.spawned {
            let tier = DepthTier::of(chunks, mob.position.floor().as_ivec3())
                .filter(|_| mob.kind.is_hostile());
            born.push(ServerMob::new(mob.kind, mob.position).in_tier(tier));
        }
        portal_shots.extend(
            outcome
                .shots
                .iter()
                .filter(|shot| is_believable(shot, eyes))
                .map(|shot| (player.id, *shot)),
        );
        if let Some(shot) = outcome.arrow {
            born.push(arrow(
                player.id,
                eyes,
                input.camera.forward().as_vec3(),
                shot,
            ));
        }
        player.exhaust(outcome.exhaustion);
        if let Some(fallen) = outcome.fallen {
            player.hurt(fall_damage(fallen), DamageSource::Fall);
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
    player.hurt(player.health, DamageSource::Void);
    player.set_movement_state(shared::players::MovementState {
        position: spawn,
        is_flying: player.is_flying,
        ..Default::default()
    });
}

/// Counts down the invulnerability of each player after a hit, and its memory of the last mob
/// that hit it. Runs first in the tick, so a hit lands again `INVULNERABILITY_TICKS` later.
pub fn tick_damage_timers_system(mut world_map: ResMut<ServerWorldMap>) {
    for player in world_map.players.values_mut() {
        player.tick_damage_timers();
    }
}

/// Each player breathes for a tick: it loses air while its eyes are in a liquid, and drowns
/// with none left. Runs once the inputs of the tick moved the players.
pub fn breathe_system(mut world_map: ResMut<ServerWorldMap>) {
    let ServerWorldMap {
        chunks, players, ..
    } = &mut *world_map;
    for player in players.values_mut() {
        player.breathe(eye_in_liquid(chunks, player.eye_position()));
    }
}

/// Each player in lava is hurt by it and set burning, each player in a fire catches it, and a
/// burning player burns for a tick, unless water or the rain under the open sky puts it out,
/// which they do not while it is in lava. Runs once the inputs of the tick moved the players.
pub fn burn_system(mut world_map: ResMut<ServerWorldMap>, weather: Res<ServerWeather>) {
    let raining = weather.kind != WeatherKind::Clear;
    let ServerWorldMap {
        chunks, players, ..
    } = &mut *world_map;
    for player in players.values_mut() {
        let in_lava = body_in_lava(chunks, player);
        player.bathe_in_lava(in_lava);
        let in_fire = body_in_fire(chunks, player);
        let doused = !in_lava && is_doused(chunks, player, raining);
        player.burn(in_fire, doused);
    }
}

/// A tick of the status effects of each player (`Player::tick_effects`): poison, burn and
/// regeneration change its health, and its slowness is set for its next inputs. Runs once the
/// inputs of the tick moved the players.
pub fn status_effects_system(mut world_map: ResMut<ServerWorldMap>) {
    for player in world_map.players.values_mut() {
        player.tick_effects();
    }
}

/// Every `REGENERATION_TICKS`, each player's metabolism (`Player::metabolise`): a fed player
/// gets a point of health back, a starving one loses one.
pub fn metabolism_system(mut world_map: ResMut<ServerWorldMap>, time: Res<ServerTime>) {
    if !time.0.is_multiple_of(REGENERATION_TICKS) {
        return;
    }
    for player in world_map.players.values_mut() {
        player.metabolise();
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
                air: player.air,
                food: player.food,
                experience: player.experience,
                burning_ticks: player.burning_ticks,
                game_mode: player.game_mode,
                death_cause: player.death_cause,
                swinging: player.is_swinging,
                held: player.inventory.in_hand(player.hotbar_slot),
                worn: player.inventory.worn(),
                effects: player.effects.clone(),
                parries: player.parries,
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
    use shared::players::game_mode::{DeathCause, GameMode, INVULNERABILITY_TICKS, MAX_HEALTH};
    use shared::players::{air::MAX_AIR, PLAYER_HEIGHT};
    use shared::world::BlockId;
    use shared::world::MobKind;

    #[test]
    fn the_damage_timers_count_down_every_tick() {
        let mut app = App::new();
        let mut world_map = ServerWorldMap::default();
        let mut player = Player::new(1, "hit".into(), Vec3::ZERO, Transform::default());
        player.hurt(1, DamageSource::Mob(MobKind::Wolf));
        world_map.players.insert(1, player);
        app.insert_resource(world_map)
            .add_systems(Update, tick_damage_timers_system);
        let player = |app: &App| app.world().resource::<ServerWorldMap>().players[&1].clone();

        app.update();
        assert_eq!(player(&app).invulnerable_ticks, INVULNERABILITY_TICKS - 1);
        for _ in 1..INVULNERABILITY_TICKS {
            app.update();
        }
        assert_eq!(player(&app).invulnerable_ticks, 0);
        assert!(player(&app).last_mob_hit.is_some());
    }

    #[test]
    fn a_player_whose_head_is_under_water_loses_air_and_one_with_wet_feet_does_not() {
        let mut world_map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15)),
            ..Default::default()
        };
        // A pool two blocks deep at x = 1, one block deep at x = 5, on stone
        world_map
            .chunks
            .fill_box(IVec3::ZERO, IVec3::new(15, 0, 15), BlockId::Stone);
        world_map
            .chunks
            .fill_box(IVec3::new(1, 1, 1), IVec3::new(1, 2, 1), BlockId::Water);
        world_map
            .chunks
            .fill_box(IVec3::new(5, 1, 1), IVec3::new(5, 1, 1), BlockId::Water);
        let standing_in = |id, x: f32| Player {
            id,
            position: Vec3::new(x, 1.0 + PLAYER_HEIGHT / 2.0, 1.5),
            ..Default::default()
        };
        world_map.players.insert(1, standing_in(1, 1.5));
        world_map.players.insert(2, standing_in(2, 5.5));
        let mut app = App::new();
        app.insert_resource(world_map)
            .add_systems(Update, breathe_system);

        for _ in 0..10 {
            app.update();
        }
        let players = &app.world().resource::<ServerWorldMap>().players;
        assert_eq!(players[&1].air, MAX_AIR - 10);
        assert_eq!(players[&2].air, MAX_AIR);
    }

    #[test]
    fn a_player_standing_in_a_fire_burns_unless_it_rains_on_it() {
        for (kind, burns) in [(WeatherKind::Clear, true), (WeatherKind::Rain, false)] {
            let mut world_map = ServerWorldMap {
                chunks: ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15)),
                ..Default::default()
            };
            world_map
                .chunks
                .fill_box(IVec3::ZERO, IVec3::new(15, 0, 15), BlockId::Stone);
            world_map
                .chunks
                .fill_box(IVec3::new(1, 1, 1), IVec3::new(1, 1, 1), BlockId::Fire);
            let player = Player {
                id: 1,
                position: Vec3::new(1.5, 1.0 + PLAYER_HEIGHT / 2.0, 1.5),
                ..Default::default()
            };
            world_map.players.insert(1, player);
            let mut weather = ServerWeather::new(&mut rand::thread_rng());
            weather.kind = kind;
            let mut app = App::new();
            app.insert_resource(world_map)
                .insert_resource(weather)
                .add_systems(Update, (tick_damage_timers_system, burn_system).chain());

            for _ in 0..21 {
                app.update();
            }
            let player = &app.world().resource::<ServerWorldMap>().players[&1];
            assert_eq!(player.is_burning(), burns, "{kind:?}");
            let health = if burns { MAX_HEALTH - 2 } else { MAX_HEALTH };
            assert_eq!(player.health, health, "{kind:?}");
        }
    }

    #[test]
    fn a_player_in_lava_burns_even_in_the_rain_and_takes_four_every_ten_ticks() {
        let mut world_map = ServerWorldMap {
            chunks: ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15)),
            ..Default::default()
        };
        world_map
            .chunks
            .fill_box(IVec3::ZERO, IVec3::new(15, 0, 15), BlockId::Stone);
        world_map
            .chunks
            .fill_box(IVec3::new(1, 1, 1), IVec3::new(1, 1, 1), BlockId::Lava);
        let player = Player {
            id: 1,
            position: Vec3::new(1.5, 1.0 + PLAYER_HEIGHT / 2.0, 1.5),
            ..Default::default()
        };
        world_map.players.insert(1, player);
        let mut weather = ServerWeather::new(&mut rand::thread_rng());
        weather.kind = WeatherKind::Rain;
        let mut app = App::new();
        app.insert_resource(world_map)
            .insert_resource(weather)
            .add_systems(Update, (tick_damage_timers_system, burn_system).chain());

        for _ in 0..21 {
            app.update();
        }
        let player = &app.world().resource::<ServerWorldMap>().players[&1];
        assert!(player.is_burning());
        assert_eq!(player.health, MAX_HEALTH - 12);
    }

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
    fn fed_players_get_their_health_back_point_by_point_and_starving_ones_lose_it() {
        let mut app = App::new();
        let mut world_map = ServerWorldMap::default();
        let mut hurt = Player::new(1, "hurt".into(), Vec3::ZERO, Transform::default());
        hurt.health = MAX_HEALTH - 2;
        let mut dead = Player::new(2, "dead".into(), Vec3::ZERO, Transform::default());
        dead.health = 0;
        let mut starving = Player::new(3, "starving".into(), Vec3::ZERO, Transform::default());
        starving.food = 0;
        let mut hungry = Player::new(4, "hungry".into(), Vec3::ZERO, Transform::default());
        hungry.food = 17;
        hungry.health = MAX_HEALTH - 2;
        world_map.players.insert(1, hurt);
        world_map.players.insert(2, dead);
        world_map.players.insert(3, starving);
        world_map.players.insert(4, hungry);
        app.insert_resource(world_map)
            .insert_resource(ServerTime(1))
            .add_systems(Update, metabolism_system);
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
        assert_eq!(health(&app, 3), MAX_HEALTH - 3);
        assert_eq!(health(&app, 4), MAX_HEALTH - 2);
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
        let mut loader = ChunkLoader::with_most_in_flight(WorldGenerator::new(1), 7);
        let loaded = |chunks: &ChunkMap| chunks.chunk_positions().collect::<Vec<_>>();
        let mut asked = 0;

        assert!(!loader.ask_players(&players, |_| 2, &chunks, None, &mut asked));
        // The chunk of the player and its six face neighbours
        assert_eq!(asked, 7);
        assert!(!loader.ask_players(&players, |_| 2, &chunks, None, &mut asked));
        assert_eq!(asked, 7, "no room is left");
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        assert_eq!(loaded(&chunks).len(), 7);
        assert!(loaded(&chunks).iter().all(|c| c.abs().element_sum() <= 1));

        loader.ask_players(&players, |_| 2, &chunks, None, &mut asked);
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        assert_eq!(
            loaded(&chunks).len(),
            14,
            "the next ones are asked for where the last stopped"
        );
        assert!(loaded(&chunks).iter().all(|c| c.abs().max_element() <= 1));

        let within = active_chunks(&players, |_| 2, &Default::default()).count();
        for _ in 0..within {
            if loader.ask_players(&players, |_| 2, &chunks, None, &mut asked) {
                break;
            }
            insert_all_made(&mut loader, &mut chunks, &mut dirty);
        }
        assert_eq!(loaded(&chunks).len(), within);
    }

    /// Opens a pair of portals of the player 1: the blue one onto the chunk `blue`, the orange
    /// one onto `orange`.
    fn open_pair(chunks: &mut ChunkMap, blue: IVec3, orange: IVec3) {
        use shared::world::{FaceDirection, Portal, PortalColor};
        for (color, chunk) in [(PortalColor::Blue, blue), (PortalColor::Orange, orange)] {
            chunks.portals_mut().insert(Portal {
                owner: 1,
                color,
                anchor: chunk * shared::CHUNK_SIZE + IVec3::new(4, 4, 4),
                face: FaceDirection::PlusZ,
                up: FaceDirection::PlusY,
            });
        }
    }

    const FAR_TWIN: IVec3 = IVec3::new(40, 0, 0);

    #[test]
    fn the_chunks_around_a_twin_are_loaded_after_those_of_the_players() {
        use shared::world::{render_distance_offsets, within_render_distance, PORTAL_VIEW_RADIUS};
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        open_pair(&mut chunks, IVec3::X, FAR_TWIN);

        let active: Vec<IVec3> = active_chunks(&players, |_| 1, chunks.portals()).collect();
        let own = render_distance_offsets(1).len();
        assert!(active[..own]
            .iter()
            .all(|c| within_render_distance(*c, IVec3::ZERO, 1)));
        assert_eq!(active.len() - own, 9 * 9 * 9);
        assert!(active[own..].iter().all(|c| within_render_distance(
            *c,
            FAR_TWIN,
            PORTAL_VIEW_RADIUS
        )));

        let mut loader = ChunkLoader::new(1);
        let mut dirty = HashSet::new();
        let mut asked = 0;
        while !loader.ask_players(&players, |_| 1, &chunks, None, &mut asked) {
            insert_all_made(&mut loader, &mut chunks, &mut dirty);
        }
        assert!(active.iter().all(|c| chunks.is_chunk_loaded(*c)));
    }

    #[test]
    fn the_render_distance_of_an_approached_twin_is_loaded_after_the_players_own() {
        use shared::world::{render_distance_offsets, within_render_distance};
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        open_pair(&mut chunks, IVec3::X, FAR_TWIN);
        let distance = 6;

        let active: Vec<IVec3> = active_chunks(&players, |_| distance, chunks.portals()).collect();
        let own = render_distance_offsets(distance).len();
        assert_eq!(active.len(), 2 * own);
        assert!(active[own..]
            .iter()
            .all(|c| within_render_distance(*c, FAR_TWIN, distance)));
        assert_eq!(active[own], FAR_TWIN);
    }

    #[test]
    fn a_portal_opened_once_the_reach_is_loaded_has_the_chunks_around_its_twin_loaded() {
        let map = ServerWorldMap {
            players: player_at_the_origin(),
            ..default()
        };
        let mut app = App::new();
        app.insert_resource(map)
            .insert_resource(WorldSeed(1))
            .add_message::<PlayerInputsEvent>()
            .add_systems(Update, handle_player_inputs_system);
        let loaded = |app: &App, chunk: IVec3| {
            app.world()
                .resource::<ServerWorldMap>()
                .chunks
                .is_chunk_loaded(chunk)
        };
        let update_until = |app: &mut App, what: &str, done: &dyn Fn(&App) -> bool| {
            let start = std::time::Instant::now();
            while !done(app) {
                assert!(start.elapsed().as_secs() < 60, "never {what}");
                app.update();
                std::thread::yield_now();
            }
        };
        let corner = IVec3::splat(shared::MIN_RENDER_DISTANCE as i32);
        update_until(&mut app, "the reach", &|app| loaded(app, corner));
        assert!(!loaded(&app, FAR_TWIN));

        open_pair(
            &mut app.world_mut().resource_mut::<ServerWorldMap>().chunks,
            IVec3::X,
            FAR_TWIN,
        );
        let far_corner = FAR_TWIN + IVec3::splat(shared::world::PORTAL_VIEW_RADIUS);
        update_until(&mut app, "the twin's chunks", &|app| {
            loaded(app, FAR_TWIN) && loaded(app, far_corner)
        });
    }

    #[test]
    fn the_chunks_known_to_be_loaded_are_skipped_and_counted() {
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        let mut dirty = HashSet::new();
        let within = active_chunks(&players, |_| 2, &Default::default()).count();
        let mut loader = ChunkLoader::new(1);

        let mut asked = 0;
        assert!(!loader.ask_players(&players, |_| 2, &chunks, None, &mut asked));
        assert_eq!(asked, within);
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        // A chunk among the first, removed, is not looked for again
        chunks.remove_chunk(IVec3::ZERO);
        assert!(loader.ask_players(&players, |_| 2, &chunks, None, &mut asked));
        assert_eq!(asked, within);
        assert_eq!(chunks.chunk_positions().count(), within - 1);

        let mut asked = 0;
        assert!(!loader.ask_players(&players, |_| 2, &chunks, None, &mut asked));
        insert_all_made(&mut loader, &mut chunks, &mut dirty);
        assert!(chunks.is_chunk_loaded(IVec3::ZERO));
        assert!(loader.ask_players(&players, |_| 2, &chunks, None, &mut asked));
    }

    #[test]
    fn a_chunk_loaded_while_it_was_in_flight_is_kept() {
        let players = player_at_the_origin();
        let mut chunks = ChunkMap::default();
        let mut dirty = HashSet::new();
        let mut loader = ChunkLoader::new(1);

        loader.ask_players(&players, |_| 1, &chunks, None, &mut 0);
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
