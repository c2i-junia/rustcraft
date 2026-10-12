use bevy::prelude::*;

// Path to fonts
pub const FONT_PATH: &str = "./fonts/RustCraftRegular-Bmg3.otf";
pub const MONO_FONT_PATH: &str = "fonts/FiraMono-Medium.ttf";
pub const SANS_FONT_PATH: &str = "fonts/FiraSans-SemiBold.ttf";

// Path to icons
pub const BACKGROUND_IMAGE_PATH: &str = "./graphics/background.png";
pub const BUTTON_BACKGROUND_IMAGE_PATH: &str = "./graphics/button_background.png";
pub const BUTTON_BACKGROUND_LARGE_IMAGE_PATH: &str = "./graphics/button_background_large.png";
pub const DARK_BUTTON_BACKGROUND_IMAGE_PATH: &str = "./graphics/dark_button_background.png";
pub const DARK_BUTTON_BACKGROUND_LARGE_IMAGE_PATH: &str =
    "./graphics/dark_button_background_large.png";
pub const TITLE_IMAGE_PATH: &str = "./graphics/title.png";

// Function to load the font asset
pub fn load_font(asset_server: &Res<AssetServer>) -> Handle<Font> {
    asset_server.load(FONT_PATH)
}

pub fn load_background_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(BACKGROUND_IMAGE_PATH)
}

pub fn load_button_background_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(BUTTON_BACKGROUND_IMAGE_PATH)
}

pub fn load_button_background_large_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(BUTTON_BACKGROUND_LARGE_IMAGE_PATH)
}

pub fn load_dark_button_background_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(DARK_BUTTON_BACKGROUND_IMAGE_PATH)
}

pub fn load_dark_button_background_large_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(DARK_BUTTON_BACKGROUND_LARGE_IMAGE_PATH)
}

pub fn load_title_image(asset_server: &Res<AssetServer>) -> Handle<Image> {
    asset_server.load(TITLE_IMAGE_PATH)
}
