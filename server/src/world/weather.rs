//! The weather of the world, held by the server: it counts down and changes on its own, and
//! `/weather` sets it. The clients only hear its kind.

use bevy::prelude::*;
use bevy_renet::RenetServer;
use rand::Rng;
use shared::messages::ServerToClientMessage;
use shared::world::{next_kind, spell_ticks, WeatherKind};

use crate::init::ServerLobby;
use crate::network::extensions::SendGameMessageExtension;
use crate::world::format::SavedWeather;

/// The weather and the ticks it has left before the next one.
#[derive(Resource, Debug, Clone, PartialEq)]
pub struct ServerWeather {
    pub kind: WeatherKind,
    pub ticks_left: u32,
}

impl ServerWeather {
    /// The weather of a new world: clear, for a clear spell.
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
}

/// Counts the weather down, and tells the players when it changes.
pub fn weather_system(
    mut weather: ResMut<ServerWeather>,
    mut server: ResMut<RenetServer>,
    lobby: Res<ServerLobby>,
) {
    let Some(kind) = weather.tick(&mut rand::thread_rng()) else {
        return;
    };
    info!("The weather turns to {}", kind.name());
    send_to_players(&mut server, &lobby, ServerToClientMessage::WeatherSet(kind));
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
    use shared::world::{CLEAR_TICKS, STORM_TICKS};

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
            })
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
}
