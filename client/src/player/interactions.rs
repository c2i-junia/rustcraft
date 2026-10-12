use crate::network::buffered_client::CurrentFrameInputs;
use crate::ui::hud::UIMode;
use crate::world::ClientWorldMap;
use bevy::color::palettes::css::WHITE;
use bevy::prelude::*;
use shared::messages::NetworkAction;
use shared::players::blocks::{simulate_player_block_interactions, CallerType};
use shared::players::{Player, ViewMode};
use shared::world::raycast;

use super::CurrentPlayerMarker;

/// Outlines the block the player aims at and, while a mouse button is held on it, predicts
/// breaking it or placing a block against it.
pub fn handle_block_interactions(
    mut player_query: Query<(&mut Player, &Transform), With<CurrentPlayerMarker>>,
    camera_query: Query<&Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
    world_map: ResMut<ClientWorldMap>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    ui_mode: Res<UIMode>,
    view_mode: Res<ViewMode>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut gizmos: Gizmos,
) {
    if *ui_mode != UIMode::Closed {
        return;
    }
    let (Ok((mut player, player_transform)), Ok(camera_transform)) =
        (player_query.single_mut(), camera_query.single())
    else {
        return;
    };
    let world_map = world_map.into_inner();

    let Some(target) = raycast::raycast(
        &world_map.blocks,
        camera_transform,
        &player_transform.translation,
        *view_mode,
    ) else {
        return;
    };

    let center = (target.bbox.max + target.bbox.min) / 2.0;
    let size = target.bbox.max - target.bbox.min;
    gizmos.cube(
        Transform::from_translation(center.into()).with_scale(size.into()),
        WHITE,
    );

    if mouse_input.pressed(MouseButton::Left) {
        frame_inputs.0.inputs.insert(NetworkAction::LeftClick);
    }
    if mouse_input.pressed(MouseButton::Right) {
        frame_inputs.0.inputs.insert(NetworkAction::RightClick);
    }

    simulate_player_block_interactions(
        &mut player,
        &mut world_map.blocks,
        &frame_inputs.0,
        CallerType::Client,
    );
}
