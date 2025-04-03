use bevy::prelude::*;

// Path to fonts
pub const FONT_PATH: &str = "./fonts/RustCraftRegular-Bmg3.otf";

// Path to icons
// pub const PLAY_ICON_PATH: &str = "./graphics/play.png";
// pub const TRASH_ICON_PATH: &str = "./graphics/trash.png";
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

// Function to load common icons
// pub fn load_play_icon(asset_server: &Res<AssetServer>) -> Handle<Image> {
//     asset_server.load(PLAY_ICON_PATH)
// }

// pub fn load_trash_icon(asset_server: &Res<AssetServer>) -> Handle<Image> {
//     asset_server.load(TRASH_ICON_PATH)
// }

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
