use crate::player::CurrentPlayerMarker;
use crate::state::GameState;
use crate::world::materials::MaterialResource;
use crate::world::time::ClientTime;
use crate::world::GraphicsSettings;
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

// The sun, drawn in the sky
#[derive(Component)]
pub struct SunLight;

// The moon, drawn in the sky opposite the sun
#[derive(Component)]
pub struct MoonLight;

/// The light of the sky: that of the sun by day and of the moon by night, from where it is.
/// One light, whose shadows are never turned off and on with the time of day: Bevy 0.19 no
/// longer draws the shadows of the meshes it had once drawn when they are turned on again.
#[derive(Component)]
pub struct SkyLight;

const SUN_ILLUMINANCE: f32 = 5000.;
const MOON_ILLUMINANCE: f32 = 500.;
const MOON_LIGHT_COLOR: Color = Color::srgb(0.788, 0.824, 0.871);
/// How high above the horizon the sun or the moon gives its full light, as the sine of its
/// height: lower, it fades out, to nothing on the horizon, where the other one takes over.
const HORIZON_FADE: f32 = 0.1;

/// Spawns the sun and the moon, opposite each other around an anchor attached to the player,
/// and the light of the sky: rotating the anchor moves both across the sky.
pub fn setup_main_lighting(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    material_resource: Res<MaterialResource>,
    player: Query<Entity, With<CurrentPlayerMarker>>,
    graphics: Option<Res<GraphicsSettings>>,
) {
    let mut body = |marker: GlobalMaterial, transform: Transform| {
        let material = material_resource.global_materials.get(&marker).cloned();
        let square = meshes.add(Rectangle::new(CELESTIAL_SIZE, CELESTIAL_SIZE));
        let mut body = commands.spawn((transform, Visibility::default()));
        // Without its material the body is not drawn, but the sky still lights the world
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

    let sun_light = body(GlobalMaterial::Sun, Transform::IDENTITY);
    let moon_light = body(
        GlobalMaterial::Moon,
        Transform::from_rotation(Quat::from_rotation_y(PI)),
    );
    commands.entity(sun_light).insert(SunLight);
    commands.entity(moon_light).insert(MoonLight);
    let sky_light = commands
        .spawn((
            SkyLight,
            DirectionalLight {
                illuminance: SUN_ILLUMINANCE,
                shadow_maps_enabled: graphics.is_none_or(|graphics| graphics.shadows),
                ..default()
            },
            Transform::IDENTITY,
        ))
        .id();

    let celestial_root = commands
        .spawn((
            CelestialRoot,
            DespawnOnExit(GameState::Game),
            Transform::default(),
            Visibility::default(),
        ))
        .add_children(&[sun_light, moon_light, sky_light])
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

/// Lights the world from the sun while it is above the horizon, and from the moon otherwise,
/// fading out as either nears the horizon: a light from under the ground would light the faces
/// turned down through the ground. The shadows are drawn if the settings say so.
pub fn celestial_light_system(
    roots: Query<(&Transform, &Children), With<CelestialRoot>>,
    mut lights: Query<
        (&mut Transform, &mut DirectionalLight),
        (With<SkyLight>, Without<CelestialRoot>),
    >,
    graphics: Option<Res<GraphicsSettings>>,
) {
    let shadows = graphics.is_none_or(|graphics| graphics.shadows);
    for (root, children) in &roots {
        for child in children {
            let Ok((mut transform, mut light)) = lights.get_mut(*child) else {
                continue;
            };
            // The sun is where the light it gives comes from, `Vec3::Z` of the anchor
            let sun_height = (root.rotation * Vec3::Z).y;
            let (rotation, illuminance, color) = if sun_height >= 0. {
                (Quat::IDENTITY, SUN_ILLUMINANCE, Color::WHITE)
            } else {
                (
                    Quat::from_rotation_y(PI),
                    MOON_ILLUMINANCE,
                    MOON_LIGHT_COLOR,
                )
            };
            let fade = (sun_height.abs() / HORIZON_FADE).clamp(0., 1.);
            transform.rotation = rotation;
            light.illuminance = illuminance * fade * fade * (3. - 2. * fade);
            light.color = color;
            if light.shadow_maps_enabled != shadows {
                light.shadow_maps_enabled = shadows;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use shared::{MIDNIGHT_IN_DAY, NOON_IN_DAY};

    /// The light of the sky at `tick`: where it shines toward, how bright it is and whether it
    /// casts shadows, with the shadows on in the settings if `shadows`.
    fn sky_light_at(tick: u64, shadows: bool) -> (Vec3, f32, bool) {
        let mut world = World::new();
        world.insert_resource(ClientTime(tick));
        world.insert_resource(GraphicsSettings {
            shadows,
            ..default()
        });
        let light = world
            .spawn((SkyLight, DirectionalLight::default(), Transform::IDENTITY))
            .id();
        let root = world
            .spawn((CelestialRoot, Transform::default()))
            .add_child(light)
            .id();
        world.run_system_once(update_celestial_bodies).unwrap();
        world.run_system_once(celestial_light_system).unwrap();
        let root = world.get::<Transform>(root).unwrap().rotation;
        let local = world.get::<Transform>(light).unwrap().rotation;
        let light = world.get::<DirectionalLight>(light).unwrap();
        (
            root * local * Vec3::NEG_Z,
            light.illuminance,
            light.shadow_maps_enabled,
        )
    }

    #[test]
    fn the_sky_shines_down_from_the_sun_by_day_and_the_moon_by_night() {
        let (toward, illuminance, shadows) = sky_light_at(NOON_IN_DAY, true);
        assert!(toward.y < -0.99, "{toward}");
        assert_eq!((illuminance, shadows), (SUN_ILLUMINANCE, true));
        let (toward, illuminance, shadows) = sky_light_at(MIDNIGHT_IN_DAY, true);
        assert!(toward.y < -0.99, "{toward}");
        assert_eq!((illuminance, shadows), (MOON_ILLUMINANCE, true));
        // Every hour, the light comes from above, if any
        for hour in 0..24 {
            let (toward, illuminance, _) = sky_light_at(DAY_DURATION_IN_TICKS * hour / 24, true);
            assert!(toward.y <= 0. || illuminance == 0., "hour {hour}: {toward}");
        }
        assert!(!sky_light_at(NOON_IN_DAY, false).2);
    }

    #[test]
    fn the_light_fades_out_at_the_horizon() {
        // At sunrise, the sun is on the horizon
        let sunrise = (0..DAY_DURATION_IN_TICKS)
            .step_by(10)
            .min_by_key(|tick| {
                let (toward, _, _) = sky_light_at(*tick, true);
                (toward.y.abs() * 1e6) as u64
            })
            .unwrap();
        assert!(sky_light_at(sunrise, true).1 < SUN_ILLUMINANCE * 0.01);
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
