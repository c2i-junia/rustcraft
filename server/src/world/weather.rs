//! The weather of the world, held by the server: it counts down and changes on its own, and
//! `/weather` sets it. The clients only hear its kind. During a thunderstorm the server also
//! draws the lightning strikes, so that every player sees the same ones.

use std::ops::RangeInclusive;

use bevy::prelude::*;
use bevy_renet::RenetServer;
use rand::seq::SliceRandom;
use rand::Rng;
use shared::messages::ServerToClientMessage;
use shared::world::{
    next_kind, spell_ticks, ChunkMap, WeatherKind, LIGHTNING_REACH, MAX_VERTICAL_RENDER_DISTANCE,
};
use shared::CHUNK_SIZE;

use crate::init::ServerLobby;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;
use crate::world::format::SavedWeather;

/// Ticks between two lightning strikes of a thunderstorm: 3 to 15 seconds.
pub const STRIKE_INTERVAL_TICKS: RangeInclusive<u32> = 60..=300;
/// How far from a player a strike lands, along the ground, in blocks.
const STRIKE_DISTANCE: RangeInclusive<f32> = 16.0..=160.0;
/// How far above and below its player the top of a struck column is looked for, in blocks: the
/// reach of a render distance.
const STRIKE_REACH: i32 = MAX_VERTICAL_RENDER_DISTANCE * CHUNK_SIZE;

/// The weather and the ticks it has left before the next one.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ServerWeather {
    pub kind: WeatherKind,
    pub ticks_left: u32,
    /// Ticks to the next lightning strike, counted during a thunderstorm only. Not saved: a
    /// storm loaded draws its own.
    pub strike_in: u32,
}

impl ServerWeather {
    /// The weather of a new world: clear, for a clear spell.
    #[cfg(test)]
    pub fn new(rng: &mut impl Rng) -> Self {
        Self::from_saved(None, rng)
    }

    /// The weather `saved` in a world, or that of a new world when it saved none.
    pub fn from_saved(saved: Option<SavedWeather>, rng: &mut impl Rng) -> Self {
        let saved = saved.unwrap_or_else(|| SavedWeather {
            kind: WeatherKind::Clear,
            ticks_left: spell_ticks(WeatherKind::Clear, rng),
        });
        Self {
            kind: saved.kind,
            ticks_left: saved.ticks_left,
            strike_in: rng.gen_range(STRIKE_INTERVAL_TICKS),
        }
    }

    /// What `world.ron` keeps of it.
    pub fn saved(&self) -> SavedWeather {
        SavedWeather {
            kind: self.kind,
            ticks_left: self.ticks_left,
        }
    }

    /// Puts `kind`, for a spell of the length it would naturally have.
    pub fn set(&mut self, kind: WeatherKind, rng: &mut impl Rng) {
        self.kind = kind;
        self.ticks_left = spell_ticks(kind, rng);
        self.strike_in = rng.gen_range(STRIKE_INTERVAL_TICKS);
    }

    /// One tick: the weather counts down, and changes once its time is out. The new kind, when
    /// it changed.
    pub fn tick(&mut self, rng: &mut impl Rng) -> Option<WeatherKind> {
        self.ticks_left = self.ticks_left.saturating_sub(1);
        if self.ticks_left > 0 {
            return None;
        }
        self.set(next_kind(self.kind, rng), rng);
        Some(self.kind)
    }

    /// One tick of lightning: during a thunderstorm, whether a strike falls on this tick.
    pub fn strike_due(&mut self, rng: &mut impl Rng) -> bool {
        if self.kind != WeatherKind::Thunder {
            return false;
        }
        self.strike_in = self.strike_in.saturating_sub(1);
        if self.strike_in > 0 {
            return false;
        }
        self.strike_in = rng.gen_range(STRIKE_INTERVAL_TICKS);
        true
    }
}

/// The highest block of the column at `x`, `z`, looked for within `STRIKE_REACH` of
/// `around_y`. `None` when it holds no block there, or its chunks are not loaded.
pub fn struck_block(chunks: &ChunkMap, x: i32, z: i32, around_y: i32) -> Option<IVec3> {
    (around_y - STRIKE_REACH..=around_y + STRIKE_REACH)
        .rev()
        .map(|y| IVec3::new(x, y, z))
        .find(|&pos| chunks.get_block(pos).is_some())
}

/// Where a strike lands: on the top block of a column drawn around one of the `players`, in
/// any direction and `STRIKE_DISTANCE` away. `None` without players, or when that column has
/// no block loaded: the strike is skipped.
pub fn draw_strike(players: &[Vec3], chunks: &ChunkMap, rng: &mut impl Rng) -> Option<IVec3> {
    let player = players.choose(rng)?;
    let angle = rng.gen_range(0.0..std::f32::consts::TAU);
    let column = player.xz() + Vec2::from_angle(angle) * rng.gen_range(STRIKE_DISTANCE);
    let column = column.floor().as_ivec2();
    struck_block(chunks, column.x, column.y, player.y.floor() as i32)
}

/// Counts the weather down and tells the players when it changes, then lets a lightning
/// strike fall when one is due, told to the players within `LIGHTNING_REACH` of it.
pub fn weather_system(
    mut weather: ResMut<ServerWeather>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
    world_map: Res<ServerWorldMap>,
) {
    let mut rng = rand::thread_rng();
    if let Some(kind) = weather.tick(&mut rng) {
        info!("The weather turns to {}", kind.name());
        send_to_players(&mut server, &lobby, ServerToClientMessage::WeatherSet(kind));
    }
    if !weather.strike_due(&mut rng) {
        return;
    }
    let players: Vec<Vec3> = world_map
        .players
        .values()
        .map(|player| player.position)
        .collect();
    let Some(at) = draw_strike(&players, &world_map.chunks, &mut rng) else {
        return;
    };
    let message = ServerToClientMessage::LightningStrike {
        at,
        seed: rng.gen(),
    };
    // Farther off, it would be neither heard nor seen
    let struck = at.xz().as_vec2() + 0.5;
    for &id in lobby.players.keys() {
        let near = world_map
            .players
            .get(&id)
            .is_some_and(|player| player.position.xz().distance(struck) <= LIGHTNING_REACH);
        if near {
            server.send_game_message(id, message.clone());
        }
    }
}

/// Sends `message` to every authenticated client: the others learn the weather when they are.
pub fn send_to_players(
    server: &mut RenetServer,
    lobby: &ServerLobby,
    message: ServerToClientMessage,
) {
    for &id in lobby.players.keys() {
        server.send_game_message(id, message.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init::LobbyPlayer;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use shared::players::Player;
    use shared::world::{BlockData, BlockDirection, BlockId, CLEAR_TICKS, STORM_TICKS};

    #[test]
    fn a_new_weather_is_clear_for_a_clear_spell() {
        let weather = ServerWeather::new(&mut StdRng::seed_from_u64(1));
        assert_eq!(weather.kind, WeatherKind::Clear);
        assert!(CLEAR_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn the_weather_counts_down_then_changes_for_a_spell_of_the_next_kind() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut weather = ServerWeather {
            kind: WeatherKind::Rain,
            ticks_left: 2,
            strike_in: 0,
        };
        assert_eq!(weather.tick(&mut rng), None);
        assert_eq!(weather.ticks_left, 1);
        assert_eq!(weather.tick(&mut rng), Some(WeatherKind::Clear));
        assert_eq!(weather.kind, WeatherKind::Clear);
        assert!(CLEAR_TICKS.contains(&weather.ticks_left));

        weather.ticks_left = 1;
        let next = weather.tick(&mut rng).unwrap();
        assert_ne!(next, WeatherKind::Clear);
        assert_eq!(weather.kind, next);
        assert!(STORM_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn a_weather_set_lasts_a_natural_spell_of_its_kind() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut weather = ServerWeather::new(&mut rng);
        weather.set(WeatherKind::Thunder, &mut rng);
        assert_eq!(weather.kind, WeatherKind::Thunder);
        assert!(STORM_TICKS.contains(&weather.ticks_left));
    }

    #[test]
    fn a_saved_weather_is_taken_back_and_a_world_without_one_starts_clear() {
        let mut rng = StdRng::seed_from_u64(1);
        let saved = SavedWeather {
            kind: WeatherKind::Rain,
            ticks_left: 345,
        };
        let weather = ServerWeather::from_saved(Some(saved), &mut rng);
        assert_eq!((weather.kind, weather.ticks_left), (WeatherKind::Rain, 345));
        assert_eq!(weather.saved(), saved);

        let weather = ServerWeather::from_saved(None, &mut rng);
        assert_eq!(weather.kind, WeatherKind::Clear);
        assert!(CLEAR_TICKS.contains(&weather.ticks_left));
    }

    /// The messages the server sent to the local client `client`, decoded.
    fn received(
        app: &mut App,
        id: u64,
        client: &mut bevy_renet::renet::RenetClient,
    ) -> Vec<ServerToClientMessage> {
        app.world_mut()
            .resource_mut::<RenetServer>()
            .process_local_client(id, client)
            .unwrap();
        let mut messages = Vec::new();
        while let Some(payload) = client.receive_message(shared::STC_STANDARD_CHANNEL) {
            messages.push(shared::payload_to_game_message(&payload).unwrap());
        }
        messages
    }

    #[test]
    fn a_change_of_weather_is_sent_to_every_authenticated_client() {
        let mut app = App::new();
        let mut server = RenetServer::new(shared::get_shared_renet_config());
        let (mut player, mut stranger) = (server.new_local_client(7), server.new_local_client(8));
        let mut lobby = ServerLobby::default();
        lobby.players.insert(7, LobbyPlayer::new("p".into()));
        app.insert_resource(server)
            .insert_resource(lobby)
            .insert_resource(ServerWeather {
                kind: WeatherKind::Rain,
                ticks_left: 2,
                strike_in: 0,
            })
            .init_resource::<ServerWorldMap>()
            .add_systems(Update, weather_system);

        app.update();
        assert!(received(&mut app, 7, &mut player).is_empty());

        app.update();
        assert!(matches!(
            received(&mut app, 7, &mut player)[..],
            [ServerToClientMessage::WeatherSet(WeatherKind::Clear)]
        ));
        assert!(received(&mut app, 8, &mut stranger).is_empty());
    }

    #[test]
    fn lightning_strikes_only_in_a_thunderstorm_every_3_to_15_seconds() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut weather = ServerWeather {
            kind: WeatherKind::Rain,
            ticks_left: 100_000,
            strike_in: 1,
        };
        assert!((0..1000).all(|_| !weather.strike_due(&mut rng)));

        weather.kind = WeatherKind::Thunder;
        weather.strike_in = 3;
        assert!(!weather.strike_due(&mut rng));
        assert!(!weather.strike_due(&mut rng));
        assert!(weather.strike_due(&mut rng));
        let mut since = 0;
        let mut intervals = Vec::new();
        for _ in 0..20_000 {
            since += 1;
            if weather.strike_due(&mut rng) {
                intervals.push(since);
                since = 0;
            }
        }
        assert!(intervals.len() > 50);
        assert!(intervals.iter().all(|ticks| (60..=300).contains(ticks)));
        assert!(intervals.iter().any(|&ticks| ticks < 90));
        assert!(intervals.iter().any(|&ticks| ticks > 270));
    }

    #[test]
    fn a_thunderstorm_set_or_saved_waits_3_to_15_seconds_for_its_first_strike() {
        let mut rng = StdRng::seed_from_u64(1);
        let mut weather = ServerWeather::new(&mut rng);
        weather.set(WeatherKind::Thunder, &mut rng);
        assert!(STRIKE_INTERVAL_TICKS.contains(&weather.strike_in));
        let saved = SavedWeather {
            kind: WeatherKind::Thunder,
            ticks_left: 500,
        };
        let weather = ServerWeather::from_saved(Some(saved), &mut rng);
        assert!(STRIKE_INTERVAL_TICKS.contains(&weather.strike_in));
    }

    fn stone() -> BlockData {
        BlockData::new(BlockId::Stone, BlockDirection::Front)
    }

    #[test]
    fn the_struck_block_is_the_highest_of_its_column_within_reach() {
        let mut chunks =
            ChunkMap::with_loaded_region(IVec3::new(0, -16, 0), IVec3::new(15, 255, 15));
        chunks.set_block(IVec3::new(3, 2, 4), stone());
        chunks.set_block(
            IVec3::new(3, 9, 4),
            BlockData::new(BlockId::OakLeaves, BlockDirection::Front),
        );
        assert_eq!(struck_block(&chunks, 3, 4, 70), Some(IVec3::new(3, 9, 4)));
        // out of reach above the player
        chunks.set_block(IVec3::new(3, 200, 4), stone());
        assert_eq!(struck_block(&chunks, 3, 4, 10), Some(IVec3::new(3, 9, 4)));
        assert_eq!(
            struck_block(&chunks, 3, 4, 100),
            Some(IVec3::new(3, 200, 4))
        );
        // an empty column, and one whose chunks are not loaded
        assert_eq!(struck_block(&chunks, 5, 5, 70), None);
        assert_eq!(struck_block(&chunks, 40, 4, 70), None);
    }

    /// Ground at y = 0 everywhere within 176 blocks of the origin.
    fn flat_ground() -> ChunkMap {
        let (min, max) = (IVec3::new(-176, 0, -176), IVec3::new(175, 15, 175));
        let mut chunks = ChunkMap::with_loaded_region(min, max);
        chunks.fill_box(min, max.with_y(0), BlockId::Stone);
        chunks
    }

    #[test]
    fn a_strike_lands_16_to_160_blocks_from_a_player_on_the_ground() {
        let mut rng = StdRng::seed_from_u64(1);
        let chunks = flat_ground();
        let player = Vec3::new(0.5, 1.0, 0.5);
        let mut distances = Vec::new();
        for _ in 0..500 {
            let at = draw_strike(&[player], &chunks, &mut rng).unwrap();
            assert_eq!(at.y, 0);
            let column = Vec2::new(at.x as f32 + 0.5, at.z as f32 + 0.5);
            distances.push(column.distance(player.xz()));
        }
        assert!(
            distances.iter().all(|d| (15.0..=161.5).contains(d)),
            "{distances:?}"
        );
        assert!(distances.iter().any(|&d| d < 30.0));
        assert!(distances.iter().any(|&d| d > 145.0));

        assert_eq!(draw_strike(&[], &chunks, &mut rng), None);
        let bare = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        assert_eq!(draw_strike(&[player], &bare, &mut rng), None);
    }

    #[test]
    fn a_strike_is_sent_to_the_authenticated_client_near_it() {
        let mut app = App::new();
        let mut server = RenetServer::new(shared::get_shared_renet_config());
        let mut client = server.new_local_client(7);
        let mut lobby = ServerLobby::default();
        lobby.players.insert(7, LobbyPlayer::new("p".into()));
        let mut world_map = ServerWorldMap {
            chunks: flat_ground(),
            ..default()
        };
        world_map.players.insert(
            7,
            Player::new(
                7,
                "p".into(),
                Vec3::new(0.5, 1.0, 0.5),
                Transform::default(),
            ),
        );
        app.insert_resource(server)
            .insert_resource(lobby)
            .insert_resource(world_map)
            .insert_resource(ServerWeather {
                kind: WeatherKind::Thunder,
                ticks_left: 100_000,
                strike_in: 1,
            })
            .add_systems(Update, weather_system);

        app.update();
        let messages = received(&mut app, 7, &mut client);
        let [ServerToClientMessage::LightningStrike { at, .. }] = messages[..] else {
            panic!("{messages:?}");
        };
        assert_eq!(at.y, 0);
        assert!(at.xz().as_vec2().length() >= 14.0, "{at}");
    }

    #[test]
    fn a_strike_is_sent_only_to_the_players_close_enough_to_see_it() {
        let mut app = App::new();
        let mut server = RenetServer::new(shared::get_shared_renet_config());
        let (mut near, mut far) = (server.new_local_client(7), server.new_local_client(8));
        let mut lobby = ServerLobby::default();
        let mut world_map = ServerWorldMap {
            chunks: flat_ground(),
            ..default()
        };
        for (id, at) in [
            (7, Vec3::new(0.5, 1., 0.5)),
            (8, Vec3::new(2_000., 1., 0.5)),
        ] {
            lobby.players.insert(id, LobbyPlayer::new("p".into()));
            world_map
                .players
                .insert(id, Player::new(id, "p".into(), at, Transform::default()));
        }
        app.insert_resource(server)
            .insert_resource(lobby)
            .insert_resource(world_map)
            .insert_resource(ServerWeather {
                kind: WeatherKind::Thunder,
                ticks_left: 100_000,
                strike_in: 1,
            })
            .add_systems(Update, weather_system);

        // the strikes fall around either player, but only the ground of the near one is loaded
        let mut struck = 0;
        for _ in 0..40 {
            app.world_mut().resource_mut::<ServerWeather>().strike_in = 1;
            app.update();
            struck += received(&mut app, 7, &mut near).len();
            assert!(received(&mut app, 8, &mut far).is_empty());
        }
        assert!(struck > 0);
        // the far one moves close enough
        let mut world_map = app.world_mut().resource_mut::<ServerWorldMap>();
        world_map.players.get_mut(&8).unwrap().position = Vec3::new(LIGHTNING_REACH - 20., 1., 0.5);
        let mut heard = 0;
        for _ in 0..40 {
            app.world_mut().resource_mut::<ServerWeather>().strike_in = 1;
            app.update();
            heard += received(&mut app, 8, &mut far).len();
        }
        assert!(heard > 0);
    }
}
