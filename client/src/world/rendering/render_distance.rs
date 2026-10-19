use crate::{
    constants::{
        DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS, MAX_CHUNK_RENDER_DISTANCE_RADIUS, VIDEO_SETTINGS_PATH,
    },
    input::{data::GameAction, keyboard::is_action_just_pressed},
    network::SendGameMessageExtension,
    state::KeyMap,
    ui::hud::UIMode,
};
use bevy::prelude::*;
use bevy_renet::RenetClient;
use ron::{from_str, ser::PrettyConfig};
use serde::{Deserialize, Serialize};
use shared::{messages::ClientToServerMessage, GameFolderPaths, MIN_RENDER_DISTANCE};
use std::fs;

/// The render distance the player asks for, in chunks, from `MIN_RENDER_DISTANCE` to
/// `MAX_CHUNK_RENDER_DISTANCE_RADIUS`: the server sends the chunks within it, up to its own
/// maximum. It also bounds how far away the mobs are drawn. Saved in `video.ron`.
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RenderDistance {
    #[serde(rename = "render_distance")]
    pub distance: u32,
}

impl Default for RenderDistance {
    fn default() -> Self {
        Self {
            distance: DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
        }
    }
}

impl RenderDistance {
    /// Sets the distance, kept within the bounds the player may choose.
    pub fn set(&mut self, distance: u32) {
        self.distance = distance.clamp(MIN_RENDER_DISTANCE, MAX_CHUNK_RENDER_DISTANCE_RADIUS);
    }

    fn decrease(&mut self) {
        self.set(self.distance.saturating_sub(1));
    }

    fn increase(&mut self) {
        self.set(self.distance.saturating_add(1));
    }
}

/// The largest render distance the server of the session grants, in chunks, from its answer
/// to the authentication.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ServerRenderDistanceLimit(pub u32);

/// The saved render distance, or the default one when none was saved or the file cannot be
/// read. A distance out of the bounds is brought back within them.
pub fn load_render_distance(paths: &GameFolderPaths) -> RenderDistance {
    let path = paths.game_folder_path.join(VIDEO_SETTINGS_PATH);
    let saved: RenderDistance = fs::read_to_string(&path)
        .ok()
        .and_then(|content| from_str(&content).ok())
        .unwrap_or_default();
    let mut render_distance = RenderDistance::default();
    render_distance.set(saved.distance);
    render_distance
}

/// Writes the render distance to `video.ron` in the game folder.
pub fn write_render_distance(render_distance: &RenderDistance, paths: &GameFolderPaths) {
    let path = paths.game_folder_path.join(VIDEO_SETTINGS_PATH);
    let serialized = match ron::ser::to_string_pretty(render_distance, PrettyConfig::new()) {
        Ok(serialized) => serialized,
        Err(e) => {
            error!("Failed to serialize the video settings: {e}");
            return;
        }
    };
    match fs::write(&path, serialized) {
        Ok(()) => info!("Video settings saved to {path:?}"),
        Err(e) => error!("Failed to save the video settings to {path:?}: {e}"),
    }
}

/// The keys that lower and raise the render distance, which is saved at once.
pub fn render_distance_update_system(
    mut render_distance: ResMut<RenderDistance>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    ui_mode: Res<UIMode>,
    paths: Res<GameFolderPaths>,
) {
    if *ui_mode != UIMode::Closed {
        return;
    }

    let before = *render_distance;
    if is_action_just_pressed(GameAction::RenderDistanceMinus, &keyboard_input, &key_map) {
        render_distance.decrease();
    }

    if is_action_just_pressed(GameAction::RenderDistancePlus, &keyboard_input, &key_map) {
        render_distance.increase();
    }

    if *render_distance != before {
        write_render_distance(&render_distance, &paths);
    }
}

/// Tells the server the render distance the player changed to. The authentication request
/// already carried the one it had, so the first run sends nothing.
pub fn send_render_distance_system(
    render_distance: Res<RenderDistance>,
    mut client: ResMut<RenetClient>,
    mut sent: Local<Option<u32>>,
) {
    let distance = render_distance.distance;
    let first_run = sent.is_none();
    if sent.replace(distance) != Some(distance) && !first_run {
        client.send_game_message(ClientToServerMessage::SetRenderDistance(distance));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_renet::RenetServer;
    use shared::{get_shared_renet_config, payload_to_game_message, CTS_STANDARD_CHANNEL};

    /// The render distances the client of `app` asked for since the last call.
    fn asked(app: &mut App, server: &mut RenetServer) -> Vec<u32> {
        let packets = app
            .world_mut()
            .resource_mut::<RenetClient>()
            .get_packets_to_send();
        for packet in packets {
            server.process_packet_from(&packet, 1).unwrap();
        }
        std::iter::from_fn(|| server.receive_message(1, CTS_STANDARD_CHANNEL))
            .map(|payload| match payload_to_game_message(&payload).unwrap() {
                ClientToServerMessage::SetRenderDistance(distance) => distance,
                other => panic!("unexpected {other:?}"),
            })
            .collect()
    }

    #[test]
    fn a_changed_render_distance_is_sent_once_and_the_first_one_is_not() {
        let mut server = RenetServer::new(get_shared_renet_config());
        server.add_connection(1);
        let mut app = App::new();
        app.insert_resource(RenetClient::new(get_shared_renet_config()))
            .insert_resource(RenderDistance { distance: 4 })
            .add_systems(Update, send_render_distance_system);

        app.update();
        assert!(asked(&mut app, &mut server).is_empty());

        app.world_mut().resource_mut::<RenderDistance>().distance = 6;
        app.update();
        app.update();
        assert_eq!(asked(&mut app, &mut server), vec![6]);

        // back and forth within one frame: nothing changed
        app.world_mut().resource_mut::<RenderDistance>().distance = 7;
        app.world_mut().resource_mut::<RenderDistance>().distance = 6;
        app.update();
        assert!(asked(&mut app, &mut server).is_empty());
    }

    #[test]
    fn the_distance_starts_at_the_default_and_stays_within_the_bounds() {
        let mut render_distance = RenderDistance::default();
        assert_eq!(
            render_distance.distance,
            DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS
        );

        for _ in 0..DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS + 3 {
            render_distance.decrease();
        }
        assert_eq!(render_distance.distance, MIN_RENDER_DISTANCE);

        render_distance.increase();
        assert_eq!(render_distance.distance, MIN_RENDER_DISTANCE + 1);

        for _ in 0..MAX_CHUNK_RENDER_DISTANCE_RADIUS + 3 {
            render_distance.increase();
        }
        assert_eq!(render_distance.distance, MAX_CHUNK_RENDER_DISTANCE_RADIUS);
    }

    #[test]
    fn the_render_distance_is_saved_and_loaded_back_within_the_bounds() {
        let dir = std::env::temp_dir().join(format!("rustcraft-video-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        };
        let file = dir.join(VIDEO_SETTINGS_PATH);
        assert_eq!(load_render_distance(&paths), RenderDistance::default());

        write_render_distance(&RenderDistance { distance: 5 }, &paths);
        assert_eq!(load_render_distance(&paths).distance, 5);

        for (content, expected) in [
            ("garbage", DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS),
            ("()", DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS),
            ("(render_distance: 0)", MIN_RENDER_DISTANCE),
            ("(render_distance: 50)", MAX_CHUNK_RENDER_DISTANCE_RADIUS),
        ] {
            fs::write(&file, content).unwrap();
            assert_eq!(load_render_distance(&paths).distance, expected, "{content}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }
}
