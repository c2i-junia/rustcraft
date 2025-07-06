use crate::ui::assets::*;
use crate::{ui::assets::load_background_image, TEXT_COLOR};
use bevy::prelude::ImageNode;
use bevy::prelude::*;
use bevy::text::{TextColor, TextFont};
use bevy::ui::BackgroundColor;
use bevy::utils::default;
use bevy::{
    asset::AssetServer,
    prelude::{Commands, Res, StateScoped},
    ui::{AlignItems, FlexDirection, JustifyContent, Node, UiRect, Val},
};

use crate::menus::{MenuButtonAction, MenuState};

pub fn settings_menu_setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    let background_image = load_background_image(&asset_server);
    let font = load_font(&asset_server);

    let button_style = Node {
        width: Val::Px(400.0),
        height: Val::Px(60.0),
        margin: UiRect::all(Val::Px(20.0)),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    };

    let button_font = TextFont {
        font: font.clone(),
        font_size: 33.0,
        ..default()
    };

    let button_color = TextColor(TEXT_COLOR);

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
            ImageNode::new(background_image),
            StateScoped(MenuState::Settings),
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
