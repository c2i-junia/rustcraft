use bevy::prelude::*;
use shared::players::ViewMode;

use super::PLAYER_LABEL_FONT_SIZE;

#[derive(Component, Debug)]
pub struct PlayerLabel {
    pub entity: Entity,
    pub name: String,
}

pub fn player_labels_system(
    camera: Single<(&Camera, &GlobalTransform), With<Camera>>,
    mut labels: Query<(&mut Node, &mut Visibility, &PlayerLabel)>,
    labeled: Query<&GlobalTransform>,
    view: Res<ViewMode>,
    ui_scale: Res<UiScale>,
) {
    let (camera, camera_global_transform) = camera.into_inner();

    // The labels are written only when they change, as a write marks them changed for the frame
    for (mut node, mut vis, label) in &mut labels {
        if *view == ViewMode::FirstPerson {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        let Ok(entity) = labeled.get(label.entity) else {
            warn!("Entity not found: {:?}", label);
            continue;
        };
        let offset = Vec3::new(0.0, 1.2, 0.0);
        let world_position = entity.translation() + offset;

        // A player behind the camera has no place on screen
        let Ok(viewport_position) =
            camera.world_to_viewport(camera_global_transform, world_position)
        else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        vis.set_if_neq(Visibility::Visible);
        // The logic for centering the label is clearly not sound but kinda works, should be improved
        let name_px_size = PLAYER_LABEL_FONT_SIZE * label.name.len() as f32;
        // `Val::Px` is multiplied by the UI scale: the viewport position is divided by it
        let top = Val::Px(viewport_position.y / ui_scale.0);
        let left = Val::Px((viewport_position.x - (name_px_size / 4.0)) / ui_scale.0);
        if node.top != top || node.left != left {
            node.top = top;
            node.left = left;
        }
    }
}
