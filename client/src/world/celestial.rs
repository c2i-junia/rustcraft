use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::world::materials::MaterialResource;
use crate::world::time::ClientTime;
use crate::{
    constants::{CELESTIAL_DISTANCE, CELESTIAL_SIZE},
    world::GlobalMaterial,
};
use bevy::{
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};
use shared::DAY_DURATION_IN_TICKS;
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

/// The angle the sky has turned by around X at the world clock `tick`: the sun is at
/// `Quat::from_rotation_x(angle) * Vec3::Z` from the player.
pub fn sun_angle(tick: u64) -> f32 {
    // Calculate the angle for the rotation (normalization between 0 and 1)
    let normalized_time = (tick % DAY_DURATION_IN_TICKS) as f32 / DAY_DURATION_IN_TICKS as f32;
    normalized_time * 2.0 * PI
}

/// How high the sun is at the world clock `tick`: 1 overhead at noon, 0 on the horizon, −1 at
/// midnight.
pub fn sun_height(tick: u64) -> f32 {
    (Quat::from_rotation_x(sun_angle(tick)) * Vec3::Z).y
}

/// 0 up to `from`, 1 from `to`, and a smooth curve between.
pub fn smoothstep(from: f32, to: f32, x: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

pub fn update_celestial_bodies(
    mut query: Query<&mut Transform, With<CelestialRoot>>,
    time: Res<ClientTime>,
) {
    let angle = sun_angle(time.0);

    // Apply the rotation to celestial bodies
    for mut tr in query.iter_mut() {
        tr.rotation = Quat::from_rotation_x(angle);
    }
}

/// Only the body above the horizon, whose light shines down, casts shadows: the shadows of the
/// other one would be drawn every frame for nothing, as its light comes from under the ground.
pub fn celestial_shadows_system(
    roots: Query<(&Transform, &Children), With<CelestialRoot>>,
    mut lights: Query<(&Transform, &mut DirectionalLight), Without<CelestialRoot>>,
) {
    for (root, children) in &roots {
        for child in children {
            let Ok((transform, mut light)) = lights.get_mut(*child) else {
                continue;
            };
            let shines_down = (root.rotation * transform.rotation * Vec3::NEG_Z).y < 0.;
            if light.shadow_maps_enabled != shines_down {
                light.shadow_maps_enabled = shines_down;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use shared::{MIDNIGHT_IN_DAY, NOON_IN_DAY};

    /// Whether the sun and the moon cast shadows at `tick`, both casting before the update.
    fn casters_at(tick: u64) -> (bool, bool) {
        let mut world = World::new();
        world.insert_resource(ClientTime(tick));
        let light = || DirectionalLight {
            shadow_maps_enabled: true,
            ..default()
        };
        let sun = world.spawn((SunLight, light(), Transform::IDENTITY)).id();
        let moon = world
            .spawn((
                MoonLight,
                light(),
                Transform::from_rotation(Quat::from_rotation_y(PI)),
            ))
            .id();
        world
            .spawn((CelestialRoot, Transform::default()))
            .add_children(&[sun, moon]);
        world.run_system_once(update_celestial_bodies).unwrap();
        world.run_system_once(celestial_shadows_system).unwrap();
        let casts = |entity| {
            world
                .get::<DirectionalLight>(entity)
                .unwrap()
                .shadow_maps_enabled
        };
        (casts(sun), casts(moon))
    }

    #[test]
    fn only_the_body_above_the_horizon_casts_shadows() {
        assert_eq!(casters_at(NOON_IN_DAY), (true, false));
        assert_eq!(casters_at(MIDNIGHT_IN_DAY), (false, true));
    }

    /// Where the sun is drawn, relative to the player, at `tick`.
    fn sun_position_at(tick: u64) -> Vec3 {
        let mut app = App::new();
        app.insert_resource(ClientTime(tick))
            .add_systems(Update, update_celestial_bodies);
        let root = app
            .world_mut()
            .spawn((CelestialRoot, Transform::default()))
            .id();
        app.update();
        let rotation = app.world().get::<Transform>(root).unwrap().rotation;
        rotation * Vec3::new(0., 0., CELESTIAL_DISTANCE)
    }

    #[test]
    fn the_sun_is_at_its_highest_at_noon_and_lowest_at_midnight() {
        let noon = sun_position_at(5 * DAY_DURATION_IN_TICKS + NOON_IN_DAY);
        assert!(noon.y > CELESTIAL_DISTANCE * 0.99, "{noon}");
        let midnight = sun_position_at(5 * DAY_DURATION_IN_TICKS + MIDNIGHT_IN_DAY);
        assert!(midnight.y < -CELESTIAL_DISTANCE * 0.99, "{midnight}");
    }
}
