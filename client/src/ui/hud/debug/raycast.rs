use bevy::prelude::*;
use shared::players::ViewMode;
use shared::world::{raycast, FaceDirectionExt};

use crate::world::ClientWorldMap;

use super::DebugOptions;
use crate::player::CurrentPlayerMarker;

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

    let camera_transform = camera_query.single().unwrap();
    let player_transform = p_transform.single().unwrap();
    let player_translation = &player_transform.translation;

    let world_map = world_map.into_inner();

    let maybe_block = raycast::raycast(world_map, camera_transform, player_translation, *view_mode);

    if let Some(raycast_response) = maybe_block {
        let normal = raycast_response.face.to_ivec3().as_vec3();
        let start = raycast_response.position.as_vec3();
        let end = start + normal;

        gizmos.line(start, end, Color::srgb(1.0, 0.0, 0.0));
    }
}
