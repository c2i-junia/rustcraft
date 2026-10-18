//! The audio settings screen: one line per volume, with a button to lower it and one to
//! raise it.

use bevy::prelude::*;

use crate::audio::settings::{AudioSettings, Channel};
use crate::menus::{MenuButtonAction, MenuState};
use crate::ui::assets::*;
use crate::ui::style::{
    background_image_style, big_button_style, fill_image_node, text_font, BUTTON_TEXT_COLOR,
    NORMAL_BUTTON,
};

/// Moves the volume of `channel` by `steps` steps.
#[derive(Component)]
pub struct VolumeButton {
    channel: Channel,
    steps: i32,
}

/// The text showing the volume of a channel.
#[derive(Component)]
pub struct VolumeLabel(Channel);

fn volume_text(settings: &AudioSettings, channel: Channel) -> String {
    format!("{:.0}%", settings.get(channel) * 100.0)
}

pub fn audio_menu_setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    settings: Res<AudioSettings>,
) {
    let font = load_font(&asset_server);
    let button_image = load_button_background_image(&asset_server);
    let text = |size| (text_font(font.clone(), size), TextColor(BUTTON_TEXT_COLOR));
    let small_button = || {
        (
            Button,
            Node {
                width: Val::Px(60.0),
                height: Val::Px(60.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(NORMAL_BUTTON),
            fill_image_node(button_image.clone()),
        )
    };

    commands
        .spawn((
            background_image_style(),
            fill_image_node(load_background_image(&asset_server)),
            DespawnOnExit(MenuState::SettingsAudio),
        ))
        .with_children(|parent| {
            parent.spawn((Text::new("Audio"), text(36.0)));
            for channel in Channel::ALL {
                parent
                    .spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(20.0),
                        margin: UiRect::top(Val::Px(20.0)),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((
                            Text::new(channel.label()),
                            text(33.0),
                            Node {
                                width: Val::Px(240.0),
                                ..default()
                            },
                        ));
                        line.spawn((
                            small_button(),
                            VolumeButton { channel, steps: -1 },
                            Name::new(format!("Lower {}", channel.label())),
                        ))
                        .with_child((Text::new("-"), text(33.0)));
                        line.spawn((
                            Text::new(volume_text(&settings, channel)),
                            text(33.0),
                            TextLayout::justify(Justify::Center),
                            Node {
                                width: Val::Px(110.0),
                                ..default()
                            },
                            VolumeLabel(channel),
                        ));
                        line.spawn((
                            small_button(),
                            VolumeButton { channel, steps: 1 },
                            Name::new(format!("Raise {}", channel.label())),
                        ))
                        .with_child((Text::new("+"), text(33.0)));
                    });
            }
            parent
                .spawn((
                    Button,
                    big_button_style(),
                    BackgroundColor(NORMAL_BUTTON),
                    fill_image_node(button_image.clone()),
                    MenuButtonAction::BackToSettings,
                ))
                .with_child((Text::new("Back"), text(33.0)));
        });
}

pub fn audio_menu_update(
    buttons: Query<(&Interaction, &VolumeButton), Changed<Interaction>>,
    mut labels: Query<(&mut Text, &VolumeLabel)>,
    mut settings: ResMut<AudioSettings>,
) {
    for (interaction, button) in &buttons {
        if *interaction == Interaction::Pressed {
            settings.step(button.channel, button.steps);
        }
    }
    if settings.is_changed() {
        for (mut text, label) in &mut labels {
            text.0 = volume_text(&settings, label.0);
        }
    }
}
