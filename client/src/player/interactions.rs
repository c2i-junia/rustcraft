use crate::mob::{MobMarker, TargetedMob, TargetedMobData};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::ui::hud::UIMode;
use crate::world::ClientWorldMap;
use bevy::color::palettes::css::{GREEN, WHITE};
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
        Query<&MobMarker>,
    ),
    resources: (
        ResMut<ClientWorldMap>,
        Res<ButtonInput<MouseButton>>,
        Res<UIMode>,
        Res<ViewMode>,
        ResMut<TargetedMob>,
        ResMut<CurrentFrameInputs>,
    ),
    mut ray_cast: MeshRayCast,
    mut gizmos: Gizmos,
) {
    let (mut player_query, p_transform, camera_query, mob_query) = queries;
    let (world_map, mouse_input, ui_mode, view_mode, mut targeted_mob, mut frame_inputs) =
        resources;

    let mut player = player_query.single_mut().unwrap();

    if *ui_mode == UIMode::Opened {
        return;
    }

    let camera_transform = camera_query.single().unwrap();
    let player_transform = p_transform.single().unwrap();
    let player_translation = &player_transform.translation;

    let ray = Ray3d::new(camera_transform.translation, camera_transform.forward());

    let world_map = world_map.into_inner();

    let maybe_block = raycast::raycast(world_map, camera_transform, player_translation, *view_mode);

    bounce_ray(ray, &mut ray_cast);

    if let Some((entity, _)) = ray_cast
        .cast_ray(ray, &MeshRayCastSettings::default())
        .first()
    {
        let mob = mob_query.get(*entity);
        if let Ok(mob) = mob {
            targeted_mob.target = Some(TargetedMobData {
                // entity: *entity,
                id: mob.id,
                name: mob.name.clone(),
            });
        } else {
            targeted_mob.target = None;
        }
    } else {
        targeted_mob.target = None;
    }

    if mouse_input.just_pressed(MouseButton::Left) && targeted_mob.target.is_some() {
        // TODO: Attack the targeted

        targeted_mob.target = None;

        return;
    }

    if let Some(res) = maybe_block {
        // Draw gizmos for the bounding box
        let center = (res.bbox.max + res.bbox.min) / 2.0;
        let hsize = res.bbox.max - res.bbox.min;
        gizmos.cuboid(
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

const MAX_BOUNCES: usize = 1;

// Bounces a ray off of surfaces `MAX_BOUNCES` times.
fn bounce_ray(mut ray: Ray3d, ray_cast: &mut MeshRayCast) {
    let color = Color::from(GREEN);

    let mut intersections = Vec::with_capacity(MAX_BOUNCES + 1);
    intersections.push((ray.origin, Color::srgb(30.0, 0.0, 0.0)));

    for i in 0..MAX_BOUNCES {
        // Cast the ray and get the first hit
        let Some((_, hit)) = ray_cast
            .cast_ray(ray, &MeshRayCastSettings::default())
            .first()
        else {
            break;
        };

        // debug!("Hit: {:?} {:?}", entity, hit);

        // Draw the point of intersection and add it to the list
        let brightness = 1.0 + 10.0 * (1.0 - i as f32 / MAX_BOUNCES as f32);
        intersections.push((hit.point, Color::BLACK.mix(&color, brightness)));

        // Reflect the ray off of the surface
        ray.direction = Dir3::new(ray.direction.reflect(hit.normal)).unwrap();
        ray.origin = hit.point + ray.direction * 1e-6;
    }
}
