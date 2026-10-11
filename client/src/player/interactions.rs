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

// Function to handle block placement and breaking
pub fn handle_block_interactions(
    queries: (
        Query<&mut Player, With<CurrentPlayerMarker>>,
        Query<&mut Transform, With<CurrentPlayerMarker>>,
        Query<&Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
    ),
    resources: (
        ResMut<ClientWorldMap>,
        Res<ButtonInput<MouseButton>>,
        Res<UIMode>,
        Res<ViewMode>,
        ResMut<CurrentFrameInputs>,
    ),
    mut gizmos: Gizmos,
) {
    let (mut player_query, p_transform, camera_query) = queries;
    let (world_map, mouse_input, ui_mode, view_mode, mut frame_inputs) = resources;

    let mut player = player_query.single_mut().unwrap();

    if *ui_mode != UIMode::Closed {
        return;
    }

    let camera_transform = camera_query.single().unwrap();
    let player_transform = p_transform.single().unwrap();
    let player_translation = &player_transform.translation;

    let world_map = world_map.into_inner();

    let maybe_block = raycast::raycast(world_map, camera_transform, player_translation, *view_mode);

    if let Some(res) = maybe_block {
        // Draw gizmos for the bounding box
        let center = (res.bbox.max + res.bbox.min) / 2.0;
        let hsize = res.bbox.max - res.bbox.min;
        gizmos.cube(
            Transform::from_translation(center.into()).with_scale(hsize.into()),
            WHITE,
        );

        // Handle left-click for breaking blocks
        if mouse_input.pressed(MouseButton::Left) {
            frame_inputs.0.inputs.insert(NetworkAction::LeftClick);
        }

        // Handle right-click for placing blocks
        if mouse_input.pressed(MouseButton::Right) {
            frame_inputs.0.inputs.insert(NetworkAction::RightClick);
        }

        simulate_player_block_interactions(
            &mut player,
            world_map,
            &frame_inputs.0,
            CallerType::Client,
        );
    }
}
