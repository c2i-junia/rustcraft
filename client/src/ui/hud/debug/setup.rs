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
            (
                // give it a dark background for readability
                BackgroundColor(Color::BLACK.with_alpha(0.5)),
                // make it "always on top" by setting the Z index to maximum
                // we want it to be displayed over all other UI
                GlobalZIndex(i32::MAX),
                Node {
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
            ),
        ))
        .id();
    // create our text
    let text_fps = commands
        .spawn((
            Text::new("FPS :"),
            TextFont {
                font_size: 16.0,
                // if you want to use your game's font asset,
                // uncomment this and provide the handle:
                // font: my_font_handle
                ..default()
            },
            TextColor(Color::WHITE),
        ))
        .with_child((
            FpsText,
            TextSpan::new("N/A"),
            TextFont::from_font_size(16.0),
            TextColor(Color::WHITE),
        ))
        .id();
    // Displays selected block type
    let block_text = commands
        .spawn((
            BlockText,
            (
                (
                    Text::new("Selected block : "),
                    TextFont {
                        font_size: 16.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.2, 0.2, 0.2)),
                ),
                // (
                //     Text::new("<None>"),
                //     TextFont {
                //         font_size: 16.0,
                //         ..default()
                //     },
                //     TextColor(Color::srgb(0.2, 0.2, 0.2)),
                // ),
            ),
        ))
        .id();
    let default_text_bundle = || {
        (
            Text::new("..."),
            TextFont {
                font_size: 16.0,
                ..default()
            },
            TextColor(Color::WHITE),
        )
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
            Text::new("Time: N/A"),
            TextColor(Color::WHITE),
            TextFont {
                font_size: 16.0,
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_children(&[
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
