use bevy::prelude::*;

use super::{
    assets::{
        load_button_background_image, load_button_background_large_image,
        load_dark_button_background_image, load_dark_button_background_large_image,
    },
    style::*,
};

#[derive(Component)]
pub struct ScrollingList {
    pub position: f32,
}

// All actions that can be triggered from a button click
#[derive(Component)]
pub enum MenuButtonAction {
    SettingsControls,
    SettingsAudio,
    BackToMainMenu,
    BackToSettings,
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum MenuState {
    Main,
    Solo,
    Multi,
    Settings,
    SettingsControls,
    SettingsAudio,
    #[default]
    Disabled,
}

pub fn button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut ImageNode,
            &mut BackgroundColor,
            &Node,
            &ComputedNode,
        ),
        With<Button>,
    >,
    asset_server: Res<AssetServer>, // Needed to load images
) {
    for (interaction, mut ui_image, mut background_color, style, node) in &mut interaction_query {
        // Get button real size
        let width = match style.width {
            Val::Px(width) => width,
            _ => node.size().x,
        };

        // Wide buttons have their own background images
        let (normal_image, dark_image) = if width > 400.0 {
            (
                load_button_background_large_image(&asset_server),
                load_dark_button_background_large_image(&asset_server),
            )
        } else {
            (
                load_button_background_image(&asset_server),
                load_dark_button_background_image(&asset_server),
            )
        };

        // The dark image shows only while the button is hovered
        let (color, image) = match *interaction {
            Interaction::Pressed => (PRESSED_BUTTON, normal_image),
            Interaction::Hovered => (HOVERED_BUTTON, dark_image),
            Interaction::None => (NORMAL_BUTTON, normal_image),
        };
        background_color.set_if_neq(color.into());
        if ui_image.image != image {
            *ui_image = fill_image_node(image);
        }
    }
}
