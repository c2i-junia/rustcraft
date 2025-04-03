use bevy::prelude::*;
use bevy_simple_text_input::TextInputInactive;

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

// Tag component used to mark which setting is currently selected
#[derive(Component)]
pub struct SelectedOption;

// All actions that can be triggered from a button click
#[derive(Component)]
pub enum MenuButtonAction {
    Solo,
    Multi,
    Settings,
    SettingsControls,
    BackToMainMenu,
    BackToSettings,
    Quit,
}

#[derive(Clone, Copy, Default, Eq, PartialEq, Debug, Hash, States)]
pub enum MenuState {
    Main,
    Solo,
    Multi,
    Settings,
    SettingsControls,
    #[default]
    Disabled,
}

pub fn button_system(
    mut interaction_query: Query<
        (
            &Interaction,
            &mut ImageNode,
            &mut BackgroundColor,
            Option<&SelectedOption>,
            &Node,
            &ComputedNode,
        ),
        With<Button>,
    >,
    input_click_query: Query<
        (Entity, &Interaction),
        (Changed<Interaction>, With<TextInputInactive>),
    >,
    mut text_input_query: Query<(Entity, &mut TextInputInactive)>,
    asset_server: Res<AssetServer>, // Needed to load images
) {
    for (interaction, mut ui_image, mut background_color, selected, style, node) in
        &mut interaction_query
    {
        // Get button real size
        let width = match style.width {
            Val::Px(width) => width,
            _ => node.size().x,
        };

        // Charger les images appropriées en fonction de la largeur
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

        match *interaction {
            Interaction::Pressed => {
                *background_color = PRESSED_BUTTON.into();
                *ui_image = normal_image.clone().into(); // Optional: Keep the "normal" image while pressed
            }
            Interaction::Hovered => {
                *background_color = HOVERED_BUTTON.into();
                *ui_image = dark_image.clone().into(); // Switch to the "dark" image on hover
            }
            Interaction::None => {
                *background_color = match selected {
                    Some(_) => PRESSED_BUTTON.into(), // Selected buttons stay pressed
                    None => NORMAL_BUTTON.into(),
                };
                *ui_image = normal_image.clone().into(); // Default back to the normal image
            }
        }
    }

    for (interaction_entity, interaction) in &input_click_query {
        if *interaction == Interaction::Pressed {
            for (entity, mut inactive) in &mut text_input_query {
                inactive.0 = entity != interaction_entity;
            }
        }
    }
}
