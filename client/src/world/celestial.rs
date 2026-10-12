use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::world::materials::MaterialResource;
use crate::world::time::ClientTime;
use crate::{
    constants::{CELESTIAL_DISTANCE, CELESTIAL_SIZE, DAY_DURATION_IN_TICKS},
    world::GlobalMaterial,
};
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::f32::consts::PI;

/// The anchor of the sun and the moon: rotating it moves both across the sky.
#[derive(Component)]
pub struct CelestialRoot;

// Main light source : the sun
#[derive(Component)]
pub struct SunLight;

// Secondary main light source : the moon
#[derive(Component)]
pub struct MoonLight;

const SUN_ILLUMINANCE: f32 = 5000.;
const MOON_ILLUMINANCE: f32 = 500.;
const MOON_LIGHT_COLOR: Color = Color::srgb(0.788, 0.824, 0.871);

/// Spawns the sun and the moon, opposite each other around an anchor attached to the player:
/// rotating the anchor moves both across the sky.
pub fn setup_main_lighting(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    material_resource: Res<MaterialResource>,
    player: Query<Entity, With<CurrentPlayerMarker>>,
) {
    let mut body = |marker: GlobalMaterial, light: DirectionalLight, transform: Transform| {
        let material = material_resource.global_materials.get(&marker).cloned();
        let square = meshes.add(Rectangle::new(CELESTIAL_SIZE, CELESTIAL_SIZE));
        let mut body = commands.spawn((light, transform));
        // Without its material the body is not drawn, but it still lights the world
        if let Some(material) = material {
            body.with_child((
                Mesh3d(square),
                MeshMaterial3d(material),
                Transform::from_translation(Vec3::new(0., 0., CELESTIAL_DISTANCE)),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        } else {
            warn!("No material for {marker:?}: it will not be visible");
        }
        body.id()
    };

    let sun_light = body(
        GlobalMaterial::Sun,
        DirectionalLight {
            illuminance: SUN_ILLUMINANCE,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::IDENTITY,
    );
    let moon_light = body(
        GlobalMaterial::Moon,
        DirectionalLight {
            illuminance: MOON_ILLUMINANCE,
            color: MOON_LIGHT_COLOR,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_rotation_y(PI)),
    );
    commands.entity(sun_light).insert(SunLight);
    commands.entity(moon_light).insert(MoonLight);

    let celestial_root = commands
        .spawn((
            CelestialRoot,
            DespawnOnExit(GameState::Game),
            Transform::default(),
        ))
        .add_children(&[sun_light, moon_light])
        .id();

    // The sky follows the player, so that the sun and the moon stay at the same distance
    match player.single() {
        Ok(player) => {
            commands.entity(player).add_child(celestial_root);
        }
        Err(_) => warn!("No local player to attach the sky to: it stays at the origin"),
    }
}

pub fn update_celestial_bodies(
    mut query: Query<&mut Transform, With<CelestialRoot>>,
    time: Res<ClientTime>,
) {
    // Calculate the angle for the rotation (normalization between 0 and 1)
    let normalized_time = (time.0 % DAY_DURATION_IN_TICKS) as f32 / DAY_DURATION_IN_TICKS as f32;
    let angle = normalized_time * 2.0 * PI;

    // Apply the rotation to celestial bodies
    for mut tr in query.iter_mut() {
        tr.rotation = Quat::from_rotation_x(angle);
    }
}
