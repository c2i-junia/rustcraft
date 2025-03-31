use bevy::prelude::*;

use crate::world::raycast;
use crate::world::ClientWorldMap;

use super::DebugOptions;
use crate::player::{CurrentPlayerMarker, ViewMode};
use crate::world::raycast::FaceDirectionExt;

pub fn raycast_debug_update_system(
    mut gizmos: Gizmos,
    world_map: ResMut<ClientWorldMap>,
    p_transform: Query<&mut Transform, With<CurrentPlayerMarker>>,
    camera_query: Query<&Transform, (With<Camera>, Without<CurrentPlayerMarker>)>,
    view_mode: Res<ViewMode>,
    debug_options: Res<DebugOptions>,
) {
    if !debug_options.is_raycast_debug_mode_enabled {
        return;
    }

    let camera_transform = camera_query.single();
    let player_transform = p_transform.single();

    let maybe_block = raycast(&world_map, camera_transform, player_transform, *view_mode);

    if let Some(raycast_response) = maybe_block {
        let normal = raycast_response.face.to_ivec3().as_vec3();
        let start = raycast_response.position.as_vec3();
        let end = start + normal;

        gizmos.line(start, end, Color::srgb(1.0, 0.0, 0.0));
    }
}
