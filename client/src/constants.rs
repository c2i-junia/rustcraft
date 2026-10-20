pub const INTERACTION_DISTANCE: f32 = 7.;
pub const BASE_ROUGHNESS: f32 = 0.6;
pub const BASE_SPECULAR_HIGHLIGHT: f32 = 0.;

// increase render distance if we build the project in release mode
pub const DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS: u32 = if cfg!(debug_assertions) { 2 } else { 4 };
/// The largest render distance the player may choose, in chunks: the chunks within 64 are
/// 129² columns of 17 chunks (`shared::world::MAX_VERTICAL_RENDER_DISTANCE`), 282897 chunks. The
/// server of a world grants it.
pub const MAX_CHUNK_RENDER_DISTANCE_RADIUS: u32 = 64;

pub const CELESTIAL_SIZE: f32 = 10.;
pub const CELESTIAL_DISTANCE: f32 = 50.; // Low value for testing ; will be increased later

pub const MAX_HOTBAR_SLOTS: u32 = shared::HOTBAR_SLOTS;

pub const SAVE_PATH: &str = "saves/";
pub const SERVER_LIST_SAVE_NAME: &str = "servers.ron";
pub const BINDS_PATH: &str = "keybindings.ron";
pub const AUDIO_SETTINGS_PATH: &str = "audio.ron";
pub const INTERFACE_SETTINGS_PATH: &str = "interface.ron";
pub const VIDEO_SETTINGS_PATH: &str = "video.ron";
pub const PLAYER_NAME_PATH: &str = "player.ron";

pub const GRASS_COLOR: [f32; 4] = [0.1, 1.0, 0.3, 1.0];

pub const TEXTURE_PATH_BASE: &str = "graphics/base_textures/";
pub const TEXTURE_PATH_CUSTOM: &str = "graphics/custom_textures/";
