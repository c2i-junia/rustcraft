//! `/locate biome <biome>`, `/locate village` and `/locate ruin|crypt|sanctuary`: the nearest
//! column of a biome, village or dungeon to a player. The search runs on the generation pool, since a biome far away takes
//! longer than a tick to find, and its answer is sent once it is done.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy_renet::renet::ClientId;
use bevy_renet::RenetServer;
use shared::messages::{FullChatMessage, ServerToClientMessage};
use shared::utils::unix_time_ms;
use shared::world::{BiomeId, TerrainNoises};

use super::generation::dungeon::{self, DungeonKind};
use super::generation::{village, WorldGeneration, WorldGenerator};
use super::pool::generation_pool;
use crate::init::ServerLobby;
use crate::network::commands::SERVER_AUTHOR;
use crate::network::extensions::SendGameMessageExtension;

/// The farthest a biome is searched, in blocks along each axis.
pub const LOCATE_RADIUS: i32 = 4096;

/// The farthest a dungeon is searched: a region of theirs costs more to read than one of the
/// villages, and dungeons are commoner.
pub const DUNGEON_RADIUS: i32 = 2048;

/// The columns read, one every this many blocks along each axis: narrower than any biome but
/// the thinnest borders, which the search may step over.
const LOCATE_STEP: i32 = 32;

/// The searches under way, whose answers come back with the player who asked.
#[derive(Resource)]
pub struct Locator {
    sender: Sender<(ClientId, String)>,
    receiver: Mutex<Receiver<(ClientId, String)>>,
}

impl Default for Locator {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver: Mutex::new(receiver),
        }
    }
}

impl Locator {
    /// Searches the nearest column of the biome `key` to `from`, in the world of `seed` made by
    /// `generation`, for the player `client`. Returns the answer to give at once: an error, or
    /// nothing while the search runs.
    pub fn start(
        &self,
        client: ClientId,
        (seed, generation): (u32, &WorldGeneration),
        key: &str,
        from: Vec3,
    ) -> String {
        let Some(biome) = generation.biomes.id(key) else {
            return format!("Unknown biome: {key}");
        };
        let sender = self.sender.clone();
        let generation = generation.clone();
        let from = from.floor().as_ivec3().xz();
        generation_pool()
            .spawn(async move {
                let terrain =
                    TerrainNoises::with(seed, &generation.settings, generation.biomes.clone());
                let name = &generation.biomes.get(biome).name;
                let answer = match nearest_biome(&terrain, biome, from, LOCATE_RADIUS) {
                    Some(at) => {
                        let distance = (at - from).as_vec2().length().round();
                        format!(
                            "The nearest {name} is at {} {}, {distance} blocks away",
                            at.x, at.y
                        )
                    }
                    None => format!("No {name} within {LOCATE_RADIUS} blocks"),
                };
                // the server may have stopped meanwhile
                let _ = sender.send((client, answer));
            })
            .detach();
        String::new()
    }

    /// Searches the nearest village to `from`, in the world of `seed` made by `generation`, for
    /// the player `client`, as [`Locator::start`] does.
    pub fn start_village(
        &self,
        client: ClientId,
        (seed, generation): (u32, &WorldGeneration),
        from: Vec3,
    ) -> String {
        if !generation.settings.villages {
            return "This world has no villages".to_string();
        }
        let sender = self.sender.clone();
        let generation = generation.clone();
        let from = from.floor().as_ivec3().xz();
        generation_pool()
            .spawn(async move {
                let terrain =
                    TerrainNoises::with(seed, &generation.settings, generation.biomes.clone());
                let answer = match nearest_village(seed, &terrain, from, LOCATE_RADIUS) {
                    Some(at) => {
                        let distance = (at - from).as_vec2().length().round();
                        format!(
                            "The nearest village is at {} {}, {distance} blocks away",
                            at.x, at.y
                        )
                    }
                    None => format!("No village within {LOCATE_RADIUS} blocks"),
                };
                let _ = sender.send((client, answer));
            })
            .detach();
        String::new()
    }

    /// Searches the nearest dungeon of `kind` to `from`, as [`Locator::start_village`] does.
    pub fn start_dungeon(
        &self,
        client: ClientId,
        (seed, generation): (u32, &WorldGeneration),
        kind: DungeonKind,
        from: Vec3,
    ) -> String {
        let name = kind.name();
        if !generation.settings.dungeons {
            return format!("This world has no {name}");
        }
        let sender = self.sender.clone();
        let generation = generation.clone();
        let from = from.floor().as_ivec3().xz();
        generation_pool()
            .spawn(async move {
                let generator = generation.generator(seed);
                let door_of = |region| generator.dungeon_site(kind, region).map(|site| site.door);
                let region = dungeon::region_of(from.x, from.y);
                let nearest = nearest_of(region, dungeon::REGION, from, DUNGEON_RADIUS, door_of);
                let answer = match nearest {
                    Some(at) => {
                        let distance = (at.xz() - from).as_vec2().length().round();
                        format!(
                            "The nearest {name} is at {} {} {}, {distance} blocks away",
                            at.x, at.y, at.z
                        )
                    }
                    None => format!("No {name} within {DUNGEON_RADIUS} blocks"),
                };
                let _ = sender.send((client, answer));
            })
            .detach();
        String::new()
    }

    /// Searches the nearest dungeon of any kind to `from` in the world of `generator`, and tells
    /// the player `client` its direction and its distance, as a cartographer sells them.
    pub fn start_directions(&self, client: ClientId, generator: Arc<WorldGenerator>, from: Vec3) {
        let sender = self.sender.clone();
        let from = from.floor().as_ivec3();
        generation_pool()
            .spawn(async move {
                let region = dungeon::region_of(from.x, from.z);
                let mut nearest: Option<(DungeonKind, IVec3)> = None;
                let mut radius = DUNGEON_RADIUS;
                for kind in DungeonKind::ALL {
                    let door_of =
                        |region| generator.dungeon_site(kind, region).map(|site| site.door);
                    let found = nearest_of(region, dungeon::REGION, from.xz(), radius, door_of);
                    let Some(at) = found else {
                        continue;
                    };
                    let distance = |at: IVec3| (at.xz() - from.xz()).as_i64vec2().length_squared();
                    if nearest.is_none_or(|(_, best)| distance(at) < distance(best)) {
                        nearest = Some((kind, at));
                        // The next kinds are not searched farther
                        radius = (at.xz() - from.xz()).as_vec2().length().ceil() as i32;
                    }
                }
                let answer = match nearest {
                    Some((kind, at)) => directions(kind, at - from),
                    None => "The cartographer knows of no dungeon around here".to_string(),
                };
                let _ = sender.send((client, answer));
            })
            .detach();
    }

    /// The answers of the searches done since the last call.
    fn finished(&self) -> Vec<(ClientId, String)> {
        let receiver = self.receiver.lock().unwrap_or_else(|err| err.into_inner());
        receiver.try_iter().collect()
    }
}

/// The column of `biome` nearest to `from`, within about `radius` blocks of it along each axis,
/// among those of a grid of `LOCATE_STEP` blocks: its columns are corners of the cells the
/// climate is computed on, which are the fastest to read. The squares of the grid around
/// `from` are read outward; once a column is found, those of the squares that may still hold a
/// nearer one are read too.
pub fn nearest_biome(
    terrain: &TerrainNoises,
    biome: BiomeId,
    from: IVec2,
    radius: i32,
) -> Option<IVec2> {
    let centre = from.div_euclid(IVec2::splat(LOCATE_STEP)) * LOCATE_STEP;
    let mut nearest: Option<(i64, IVec2)> = None;
    for ring in 0..=radius / LOCATE_STEP {
        // the nearest a column of this square may be to `from`, which is in the cell of `centre`
        let reach = i64::from((ring - 1).max(0) * LOCATE_STEP);
        if nearest.is_some_and(|(distance, _)| reach * reach > distance) {
            break;
        }
        for (dx, dz) in square(ring) {
            let at = centre + IVec2::new(dx, dz) * LOCATE_STEP;
            if terrain.column_shape(at.x, at.y).biome != biome {
                continue;
            }
            let offset = (at - from).as_i64vec2();
            let distance = offset.length_squared();
            if nearest.is_none_or(|(best, _)| distance < best) {
                nearest = Some((distance, at));
            }
        }
    }
    nearest.map(|(_, at)| at)
}

/// The centre of the village nearest to `from`, among those of the regions within about
/// `radius` blocks of it along each axis.
pub fn nearest_village(
    seed: u32,
    terrain: &TerrainNoises,
    from: IVec2,
    radius: i32,
) -> Option<IVec2> {
    let centre_of =
        |region| village::site(seed, terrain, region).map(|site| site.centre.extend(0).xzy());
    let region = village::region_of(from.x, from.y);
    nearest_of(region, village::REGION, from, radius, centre_of).map(|at| at.xz())
}

/// The place nearest to `from`, within about `radius` blocks along each axis, of those
/// `place_of` gives for the regions of side `side` around `region`, the one of `from`.
fn nearest_of(
    region: IVec2,
    side: i32,
    from: IVec2,
    radius: i32,
    place_of: impl Fn(IVec2) -> Option<IVec3>,
) -> Option<IVec3> {
    let mut nearest: Option<(i64, IVec3)> = None;
    for ring in 0..=radius / side + 1 {
        // the nearest a place of this square of regions may be to `from`
        let reach = i64::from((ring - 1).max(0) * side);
        if nearest.is_some_and(|(distance, _)| reach * reach > distance) {
            break;
        }
        for (dx, dz) in square(ring) {
            let Some(place) = place_of(region + IVec2::new(dx, dz)) else {
                continue;
            };
            let offset = place.xz() - from;
            let distance = offset.as_i64vec2().length_squared();
            let within = offset.abs().max_element() <= radius;
            if within && nearest.is_none_or(|(best, _)| distance < best) {
                nearest = Some((distance, place));
            }
        }
    }
    nearest.map(|(_, at)| at)
}

/// What a cartographer says of a dungeon of `kind` at `offset` from the player. North is -Z.
fn directions(kind: DungeonKind, offset: IVec3) -> String {
    const POINTS: [&str; 8] = [
        "north",
        "north-east",
        "east",
        "south-east",
        "south",
        "south-west",
        "west",
        "north-west",
    ];
    let flat = offset.xz().as_vec2();
    let eighths = flat.x.atan2(-flat.y) / std::f32::consts::FRAC_PI_4;
    let point = POINTS[(eighths.round() as i32).rem_euclid(8) as usize];
    let depth = match kind {
        DungeonKind::Ruin => String::new(),
        _ if offset.y < 0 => format!(", {} blocks down", -offset.y),
        _ => format!(", {} blocks up", offset.y),
    };
    format!(
        "The nearest dungeon is a {}, {} blocks to the {point}{depth}",
        kind.name(),
        flat.length().round()
    )
}

/// The cells of the square `ring` cells away from its centre, each once.
fn square(ring: i32) -> Vec<(i32, i32)> {
    if ring == 0 {
        return vec![(0, 0)];
    }
    let rows = (-ring..=ring).flat_map(|x| [(x, -ring), (x, ring)]);
    let columns = (-ring + 1..ring).flat_map(|z| [(-ring, z), (ring, z)]);
    rows.chain(columns).collect()
}

/// Sends the answers of the searches done to the players who asked, if they are still there.
pub fn answer_locates_system(
    locator: Res<Locator>,
    lobby: Res<ServerLobby>,
    mut server: ResMut<RenetServer>,
) {
    for (client, content) in locator.finished() {
        if !lobby.players.contains_key(&client) {
            continue;
        }
        let answer = FullChatMessage {
            author: SERVER_AUTHOR.to_string(),
            content,
            timestamp: unix_time_ms(),
        };
        server.send_game_message(client, ServerToClientMessage::ChatMessage(answer));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::vanilla;
    use std::collections::HashSet;

    #[test]
    fn a_cartographer_tells_the_point_of_the_compass() {
        let told = |x, y, z| directions(DungeonKind::Crypt, IVec3::new(x, y, z));
        assert_eq!(
            told(0, -40, -100),
            "The nearest dungeon is a crypt, 100 blocks to the north, 40 blocks down"
        );
        assert!(told(100, 0, -100).contains("141 blocks to the north-east"));
        assert!(told(100, 0, 3).contains(" east"));
        assert!(told(-100, 0, 100).contains("south-west"));
    }

    #[test]
    fn each_square_holds_its_cells_once() {
        assert_eq!(square(0), [(0, 0)]);
        for ring in 1..5 {
            let cells: HashSet<(i32, i32)> = square(ring).into_iter().collect();
            assert_eq!(cells.len(), square(ring).len());
            assert_eq!(cells.len() as i32, 8 * ring);
            assert!(cells.iter().all(|&(x, z)| x.abs().max(z.abs()) == ring));
        }
    }

    #[test]
    fn the_nearest_column_of_a_biome_is_found() {
        let terrain = TerrainNoises::new(42);
        // the desert pinned by `terrain_is_pinned_for_a_fixed_seed`
        let from = IVec2::new(0, 0);
        let found = nearest_biome(&terrain, vanilla::DESERT, from, LOCATE_RADIUS).unwrap();
        assert_eq!(
            terrain.column_shape(found.x, found.y).biome,
            vanilla::DESERT
        );
        // as near as that desert, but for the grid
        let pinned = IVec2::new(680, 80).as_vec2().length();
        assert!(
            found.as_vec2().length() <= pinned + 1.5 * LOCATE_STEP as f32,
            "{found}"
        );
        // no column of the grid nearer
        let reach = found.as_vec2().length();
        for x in (-1024..=1024).step_by(LOCATE_STEP as usize) {
            for z in (-1024..=1024).step_by(LOCATE_STEP as usize) {
                let at = IVec2::new(x, z);
                if at.as_vec2().length() < reach {
                    assert_ne!(terrain.column_shape(x, z).biome, vanilla::DESERT, "{at}");
                }
            }
        }
        // where the player stands, at once
        let here = nearest_biome(&terrain, vanilla::DESERT, IVec2::new(680, 80), 64).unwrap();
        assert!(
            (here - IVec2::new(680, 80)).abs().max_element() <= LOCATE_STEP,
            "{here}"
        );
        assert_eq!(here % 4, IVec2::ZERO);
    }

    #[test]
    fn a_biome_beyond_the_radius_is_not_found() {
        let settings = shared::world::WorldGenSettings {
            preset: shared::world::WorldPreset::SingleBiome("desert".into()),
            ..Default::default()
        };
        let terrain = TerrainNoises::with(42, &settings, shared::world::Biomes::vanilla());
        assert_eq!(
            nearest_biome(&terrain, vanilla::JUNGLE, IVec2::ZERO, 256),
            None
        );
    }

    #[test]
    fn the_answer_comes_back_for_the_player_who_asked() {
        let locator = Locator::default();
        let generation = WorldGeneration::default();
        let unknown = locator.start(7, (42, &generation), "nowhere", Vec3::ZERO);
        assert_eq!(unknown, "Unknown biome: nowhere");
        let started = locator.start(7, (42, &generation), "desert", Vec3::new(0.5, 70.0, 0.5));
        assert_eq!(started, "");
        let start = std::time::Instant::now();
        let answers = loop {
            let answers = locator.finished();
            if !answers.is_empty() || start.elapsed().as_secs() > 10 {
                break answers;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        let [(client, answer)] = &answers[..] else {
            panic!("{answers:?}");
        };
        assert_eq!(*client, 7);
        assert!(answer.starts_with("The nearest Desert is at "), "{answer}");
    }
}
