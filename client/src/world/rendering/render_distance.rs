use crate::{
    constants::{
        DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS, MAX_CHUNK_RENDER_DISTANCE_RADIUS, VIDEO_SETTINGS_PATH,
    },
    input::{data::GameAction, keyboard::is_action_just_pressed},
    network::SendGameMessageExtension,
    state::KeyMap,
    ui::hud::UIMode,
    world::{clouds::mesh::CloudStyle, BiomeBlend, GraphicsSettings},
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
#[derive(Resource, Reflect, Clone, Copy, Debug, PartialEq)]
pub struct RenderDistance {
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

/// The video settings as `video.ron` holds them: each one missing from the file takes its
/// default.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub render_distance: u32,
    pub clouds: CloudStyle,
    /// `BiomeBlend::radius`
    pub biome_blend: u32,
    /// The fields of `GraphicsSettings`
    pub smooth_lighting: bool,
    pub brightness: f32,
    pub shadows: bool,
    pub effects: bool,
}

impl Default for VideoSettings {
    fn default() -> Self {
        let graphics = GraphicsSettings::default();
        Self {
            render_distance: DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
            clouds: CloudStyle::default(),
            biome_blend: BiomeBlend::default().radius,
            smooth_lighting: graphics.smooth_lighting,
            brightness: graphics.brightness,
            shadows: graphics.shadows,
            effects: graphics.effects,
        }
    }
}

/// The saved render distance, clouds, biome blend and graphics settings, or the default ones
/// when none were saved or the file cannot be read. A setting out of its bounds is brought back
/// within them.
pub fn load_video_settings(
    paths: &GameFolderPaths,
) -> (RenderDistance, CloudStyle, BiomeBlend, GraphicsSettings) {
    let path = paths.game_folder_path.join(VIDEO_SETTINGS_PATH);
    let saved: VideoSettings = fs::read_to_string(&path)
        .ok()
        .and_then(|content| from_str(&content).ok())
        .unwrap_or_default();
    let mut render_distance = RenderDistance::default();
    render_distance.set(saved.render_distance);
    let mut blend = BiomeBlend::default();
    blend.set(saved.biome_blend);
    let mut graphics = GraphicsSettings {
        smooth_lighting: saved.smooth_lighting,
        shadows: saved.shadows,
        effects: saved.effects,
        ..default()
    };
    graphics.set_brightness(saved.brightness);
    (render_distance, saved.clouds, blend, graphics)
}

/// Writes the render distance, the clouds, the biome blend and the graphics settings to
/// `video.ron` in the game folder: all of them, so that saving one keeps the others.
pub fn write_video_settings(
    render_distance: &RenderDistance,
    clouds: CloudStyle,
    blend: &BiomeBlend,
    graphics: &GraphicsSettings,
    paths: &GameFolderPaths,
) {
    let path = paths.game_folder_path.join(VIDEO_SETTINGS_PATH);
    let settings = VideoSettings {
        render_distance: render_distance.distance,
        clouds,
        biome_blend: blend.radius,
        smooth_lighting: graphics.smooth_lighting,
        brightness: graphics.brightness,
        shadows: graphics.shadows,
        effects: graphics.effects,
    };
    let serialized = match ron::ser::to_string_pretty(&settings, PrettyConfig::new()) {
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
    (clouds, blend, graphics): (Res<CloudStyle>, Res<BiomeBlend>, Res<GraphicsSettings>),
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
        write_video_settings(&render_distance, *clouds, &blend, &graphics, &paths);
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
    use crate::world::MAX_BIOME_BLEND;
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

    fn temp_paths(name: &str) -> GameFolderPaths {
        let dir = std::env::temp_dir().join(format!("rustcraft-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        GameFolderPaths {
            game_folder_path: dir.clone(),
            assets_folder_path: dir.join("assets"),
        }
    }

    #[test]
    fn the_render_distance_is_saved_and_loaded_back_within_the_bounds() {
        let paths = temp_paths("video");
        let file = paths.game_folder_path.join(VIDEO_SETTINGS_PATH);
        assert_eq!(
            load_video_settings(&paths),
            (
                RenderDistance::default(),
                CloudStyle::Fancy,
                BiomeBlend::default(),
                GraphicsSettings::default()
            )
        );

        let blend = BiomeBlend { radius: 6 };
        let graphics = GraphicsSettings {
            smooth_lighting: false,
            brightness: 0.8,
            shadows: false,
            effects: false,
        };
        write_video_settings(
            &RenderDistance { distance: 5 },
            CloudStyle::Fancy,
            &blend,
            &graphics,
            &paths,
        );
        assert_eq!(
            load_video_settings(&paths),
            (
                RenderDistance { distance: 5 },
                CloudStyle::Fancy,
                blend,
                graphics
            )
        );

        let default_blend = BiomeBlend::default().radius;
        for (content, distance, radius) in [
            (
                "garbage",
                DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
                default_blend,
            ),
            ("()", DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS, default_blend),
            ("(render_distance: 0)", MIN_RENDER_DISTANCE, default_blend),
            (
                "(render_distance: 500)",
                MAX_CHUNK_RENDER_DISTANCE_RADIUS,
                default_blend,
            ),
            ("(biome_blend: 0)", DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS, 0),
            (
                "(biome_blend: 50)",
                DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS,
                MAX_BIOME_BLEND,
            ),
        ] {
            fs::write(&file, content).unwrap();
            let (render_distance, _, blend, graphics) = load_video_settings(&paths);
            assert_eq!(render_distance.distance, distance, "{content}");
            assert_eq!(blend.radius, radius, "{content}");
            assert_eq!(graphics, GraphicsSettings::default(), "{content}");
        }
        for (content, brightness) in [("(brightness: 2.0)", 1.), ("(brightness: -1.0)", 0.)] {
            fs::write(&file, content).unwrap();
            assert_eq!(
                load_video_settings(&paths).3.brightness,
                brightness,
                "{content}"
            );
        }
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
    }

    #[test]
    fn a_file_saved_before_the_clouds_loads_with_fancy_clouds() {
        let paths = temp_paths("video-old");
        let file = paths.game_folder_path.join(VIDEO_SETTINGS_PATH);
        fs::write(&file, "(render_distance: 8)").unwrap();
        let loaded = load_video_settings(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(
            loaded,
            (
                RenderDistance { distance: 8 },
                CloudStyle::Fancy,
                BiomeBlend::default(),
                GraphicsSettings::default()
            )
        );
    }

    #[test]
    fn the_clouds_are_saved_and_loaded_back_with_the_render_distance() {
        let paths = temp_paths("video-clouds");
        for clouds in [CloudStyle::Fast, CloudStyle::Off, CloudStyle::Fancy] {
            let render_distance = RenderDistance { distance: 7 };
            let blend = BiomeBlend::default();
            let graphics = GraphicsSettings::default();
            write_video_settings(&render_distance, clouds, &blend, &graphics, &paths);
            assert_eq!(
                load_video_settings(&paths),
                (render_distance, clouds, blend, graphics)
            );
        }
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
    }

    #[test]
    fn the_render_distance_keys_keep_the_other_settings_saved() {
        let paths = temp_paths("video-keys");
        let mut keyboard = ButtonInput::<KeyCode>::default();
        let key_map = crate::input::keyboard::default_bindings();
        let plus = key_map.map[&GameAction::RenderDistancePlus][0];
        keyboard.press(plus);
        let mut app = App::new();
        app.insert_resource(keyboard)
            .insert_resource(key_map)
            .insert_resource(UIMode::Closed)
            .insert_resource(RenderDistance { distance: 4 })
            .insert_resource(CloudStyle::Off)
            .insert_resource(BiomeBlend { radius: 3 })
            .insert_resource(GraphicsSettings {
                shadows: false,
                ..default()
            })
            .insert_resource(paths.clone())
            .add_systems(Update, render_distance_update_system);
        app.update();
        let loaded = load_video_settings(&paths);
        fs::remove_dir_all(&paths.game_folder_path).unwrap();
        assert_eq!(
            loaded,
            (
                RenderDistance { distance: 5 },
                CloudStyle::Off,
                BiomeBlend { radius: 3 },
                GraphicsSettings {
                    shadows: false,
                    ..default()
                }
            )
        );
    }
}
