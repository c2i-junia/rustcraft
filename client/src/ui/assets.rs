use bevy::prelude::*;

// Path to fonts
pub const FONT_PATH: &str = "./fonts/RustCraftRegular-Bmg3.otf";
pub const MONO_FONT_PATH: &str = "fonts/FiraMono-Medium.ttf";

// Path to icons
pub const BACKGROUND_IMAGE_PATH: &str = "./graphics/background.png";
pub const TITLE_IMAGE_PATH: &str = "./graphics/title.png";

// Function to load the font asset
pub fn load_font(asset_server: &Res<AssetServer>) -> Handle<Font> {
    asset_server.load(FONT_PATH)
}

pub fn load_title_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(TITLE_IMAGE_PATH)
}
