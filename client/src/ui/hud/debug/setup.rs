use super::loaded_stats::TimeText;
use super::loaded_stats::{BlocksNumberText, ChunksNumberText};
use super::targeted_block::BlockText;
use super::{CoordsText, FpsText};
use crate::input::data::GameAction;
use crate::input::keyboard::get_action_keys;
use crate::{GameState, KeyMap};
use bevy::prelude::*;

/// Marker to find the container entity so we can show/hide the FPS counter
#[derive(Component)]
pub struct HudRoot;

pub fn setup_hud(mut commands: Commands) {
    // create our UI root node
    // this is the wrapper/container for the text
    let root = commands
        .spawn((
            HudRoot,
            StateScoped(GameState::Game),
            NodeBundle {
                // give it a dark background for readability
                background_color: BackgroundColor(Color::BLACK.with_alpha(0.5)),
                // make it "always on top" by setting the Z index to maximum
                // we want it to be displayed over all other UI
                z_index: ZIndex::Global(i32::MAX),
                style: Style {
                    position_type: PositionType::Absolute,
                    // position it at the top-left corner
                    // 1% away from the top window edge
                    left: Val::Percent(1.),
                    top: Val::Percent(1.),
                    // set bottom/left to Auto, so it can be
                    // automatically sized depending on the text
                    bottom: Val::Auto,
                    right: Val::Auto,
                    // give it some padding for readability
                    padding: UiRect::all(Val::Px(4.0)),
                    flex_direction: FlexDirection::Column,
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    // create our text
    let text_fps = commands
        .spawn((
            FpsText,
            TextBundle {
                // use two sections, so it is easy to update just the number
                text: Text::from_sections([
                    TextSection {
                        value: "FPS: ".into(),
                        style: TextStyle {
                            font_size: 16.0,
                            color: Color::WHITE,
                            // if you want to use your game's font asset,
                            // uncomment this and provide the handle:
                            // font: my_font_handle
                            ..default()
                        },
                    },
                    TextSection {
                        value: " N/A".into(),
                        style: TextStyle {
                            font_size: 16.0,
                            color: Color::WHITE,
                            // if you want to use your game's font asset,
                            // uncomment this and provide the handle:
                            // font: my_font_handle
                            ..default()
                        },
                    },
                ]),
                ..Default::default()
            },
        ))
        .id();

    // Displays selected block type
    let block_text = commands
        .spawn((
            BlockText,
            TextBundle {
                // use two sections, so it is easy to update just the number
                text: Text::from_sections([
                    TextSection {
                        value: "Selected block : ".into(),
                        style: TextStyle {
                            font_size: 16.0,
                            ..default()
                        },
                    },
                    TextSection {
                        value: "<None>".into(),
                        style: TextStyle {
                            font_size: 16.0,
                            color: Color::srgb(0.2, 0.2, 0.2),
                            ..default()
                        },
                    },
                ]),
                ..Default::default()
            },
        ))
        .id();

    let default_text_bundle = || TextBundle {
        text: Text::from_sections([TextSection {
            value: "...".into(),
            style: TextStyle {
                font_size: 16.0,
                color: Color::WHITE,
                ..default()
            },
        }]),
        ..Default::default()
    };

    let coords_text = commands.spawn((CoordsText, default_text_bundle())).id();

    let blocks_number_text = commands
        .spawn((BlocksNumberText, default_text_bundle()))
        .id();
    let chunks_number_text = commands
        .spawn((ChunksNumberText, default_text_bundle()))
        .id();

    let time_text = commands
        .spawn((
            TimeText,
            TextBundle {
                text: Text::from_sections([TextSection {
                    value: "Time: N/A".into(),
                    style: TextStyle {
                        font_size: 16.0,
                        color: Color::WHITE,
                        ..default()
                    },
                }]),
                ..Default::default()
            },
        ))
        .id();

    commands.entity(root).push_children(&[
        text_fps,
        coords_text,
        blocks_number_text,
        chunks_number_text,
        block_text,
        time_text,
    ]);
}

/// Toggle the FPS counter when pressing F3
pub fn toggle_hud_system(
    mut q: Query<&mut Visibility, With<HudRoot>>,
    kbd: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
) {
    let keys = get_action_keys(GameAction::ToggleFps, &key_map);
    for key in keys {
        if kbd.just_pressed(key) {
            let mut vis = q.single_mut();
            *vis = match *vis {
                Visibility::Hidden => Visibility::Visible,
                _ => Visibility::Hidden,
            };
        }
    }
}
