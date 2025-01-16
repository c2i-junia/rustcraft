use crate::player::CurrentPlayerMarker;
use crate::world::materials::MaterialResource;
use crate::world::time::ClientTime;
use crate::GameState;
use crate::{
    constants::{CELESTIAL_DISTANCE, CELESTIAL_SIZE, DAY_DURATION},
    world::GlobalMaterial,
};
use bevy::{
    pbr::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use std::f32::consts::PI;

//
#[derive(Component)]
pub struct CelestialRoot;

// Main light source : the sun
#[derive(Component)]
pub struct SunLight;

// Secondary main light source : the moon
#[derive(Component)]
pub struct MoonLight;

pub fn setup_main_lighting(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    material_resource: Res<MaterialResource>,
    player: Query<Entity, With<CurrentPlayerMarker>>,
) {
    // No fancy stuff ; Only acts as an anchor to move celestial bodies easily
    let celestial_root = commands
        .spawn((
            CelestialRoot,
            StateScoped(GameState::Game),
            Transform::default(),
        ))
        .id();

    let mut light_transform = Transform::from_translation(Vec3::new(0., 0., 0.));

    let sun_light = commands
        .spawn((
            SunLight,
            (
                DirectionalLight {
                    illuminance: 5000.,
                    shadows_enabled: true,
                    ..default()
                },
                light_transform,
            ),
        ))
        .with_children(|parent| {
            parent.spawn((
                (
                    Mesh3d(meshes.add(Rectangle::new(CELESTIAL_SIZE, CELESTIAL_SIZE))),
                    MeshMaterial3d(
                        material_resource
                            .global_materials
                            .get(&GlobalMaterial::Sun)
                            .expect("Sun material not found !")
                            .clone(),
                    ),
                    Transform {
                        translation: Vec3::new(0., 0., CELESTIAL_DISTANCE),
                        ..default()
                    },
                ),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        })
        .id();
    light_transform.rotate_y(PI);

    let moon_light = commands
        .spawn((
            MoonLight,
            (
                DirectionalLight {
                    illuminance: 500.,
                    color: Color::Srgba(Srgba::hex("c9d2de").unwrap()),
                    shadows_enabled: true,
                    ..default()
                },
                light_transform,
            ),
        ))
        .with_children(|parent| {
            parent.spawn((
                (
                    Mesh3d(meshes.add(Rectangle::new(CELESTIAL_SIZE, CELESTIAL_SIZE))),
                    MeshMaterial3d(
                        material_resource
                            .global_materials
                            .get(&GlobalMaterial::Moon)
                            .expect("Moon material not found !")
                            .clone(),
                    ),
                    Transform {
                        translation: Vec3::new(0., 0., CELESTIAL_DISTANCE),
                        ..Default::default()
                    },
                ),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        })
        .id();

    commands
        .entity(celestial_root)
        .add_children(&[sun_light, moon_light]);

    commands.entity(player.single()).add_child(celestial_root);
}

pub fn update_celestial_bodies(
    mut query: Query<&mut Transform, With<CelestialRoot>>,
    time: Res<Time>,
    client_time: Res<ClientTime>,
) {
    static mut LOCAL_TIME: f32 = 0.0;
    static mut LAST_SYNC: f32 = 0.0;

    unsafe {
        // Unsafe is used here because we are working with static mutable variables (`LOCAL_TIME` and `LAST_SYNC`).
        // Static mutable variables are not inherently thread-safe, and Rust enforces safety guarantees
        // to avoid potential data races. Since Bevy's systems run sequentially by default and this system
        // does not share these static variables across multiple threads, we are assuming it's safe to use `unsafe` here.
        // However, this approach should be avoided if this system might ever run in parallel or be accessed
        // from multiple threads simultaneously.

        // Update local time with delta_secs
        LOCAL_TIME += time.delta_secs();

        // Synchronize with the server time every second
        if LOCAL_TIME - LAST_SYNC >= 1.0 {
            LOCAL_TIME = client_time.0 as f32; // Reset local time based on the server
            LAST_SYNC = LOCAL_TIME;
        }

        // Calculate the angle for the rotation (normalization between 0 and 1)
        let normalized_time = (LOCAL_TIME % DAY_DURATION) / DAY_DURATION;
        let angle = normalized_time * 2.0 * PI;

        // Apply the rotation to celestial bodies
        for mut tr in query.iter_mut() {
            tr.rotation = Quat::from_rotation_x(angle);
        }
    }
}
