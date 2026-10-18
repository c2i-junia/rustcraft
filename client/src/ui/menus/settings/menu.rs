use crate::menus::{MenuButtonAction, MenuState};
use crate::ui::assets::*;
use crate::ui::style::{big_button_style, fill_image_node, text_font, BUTTON_TEXT_COLOR};
use bevy::prelude::*;

pub fn settings_menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let background_image = load_background_image(&asset_server);
    let button_style = big_button_style();
    let button_font = text_font(load_font(&asset_server), 33.0);
    let button_color = TextColor(BUTTON_TEXT_COLOR);

    commands
        .spawn((
            (
                Node {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ),
            fill_image_node(background_image),
            DespawnOnExit(MenuState::Settings),
        ))
        .with_children(|parent| {
            parent
                .spawn((Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    ..default()
                },))
                .with_children(|parent| {
                    for (action, text) in [
                        (MenuButtonAction::SettingsControls, "Controls"),
                        (MenuButtonAction::SettingsAudio, "Audio"),
                        (MenuButtonAction::BackToMainMenu, "Back"),
                    ] {
                        parent
                            .spawn((
                                (Button, button_style.clone(), BackgroundColor(Color::NONE)),
                                action,
                            ))
                            .with_children(|parent| {
                                parent.spawn((Text::new(text), button_font.clone(), button_color));
                            });
                    }
                });
        });
}
