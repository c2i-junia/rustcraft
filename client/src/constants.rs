pub const CUBE_SIZE: f32 = 1.0;

pub const INTERACTION_DISTANCE: f32 = 7.;
pub const BASE_ROUGHNESS: f32 = 0.6;
pub const BASE_SPECULAR_HIGHLIGHT: f32 = 0.;

// increase render distance if we build the project in release mode
pub const DEFAULT_CHUNK_RENDER_DISTANCE_RADIUS: u32 = if cfg!(debug_assertions) { 2 } else { 4 };

pub const CELESTIAL_SIZE: f32 = 10.;
pub const CELESTIAL_DISTANCE: f32 = 50.; // Low value for testing ; will be increased later
pub const DAY_DURATION_IN_TICKS: u64 = 20 * 60; // 20 ticks per second * 60 seconds = 1 minute

pub const MAX_HOTBAR_SLOTS: u32 = 9;

pub const HOTBAR_CELL_SIZE: f32 = 50.;
pub const HOTBAR_PADDING: f32 = 5.;
pub const HOTBAR_BORDER: f32 = 5.;

pub const SAVE_PATH: &str = "saves/";
pub const SERVER_LIST_SAVE_NAME: &str = "servers.ron";
pub const BINDS_PATH: &str = "keybindings.ron";

pub const GRASS_COLOR: [f32; 4] = [0.1, 1.0, 0.3, 1.0];

pub const TEXTURE_PATH_BASE: &str = "graphics/base_textures/";
pub const TEXTURE_PATH_CUSTOM: &str = "graphics/custom_textures/";
