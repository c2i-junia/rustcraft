//! The shots of the portal gun, seen: a short glowing bolt of the portal's colour flies from
//! the shooter's eye to where the shot hits at `BOLT_SPEED`, a fading trail behind it, and
//! bursts into a few sparks there. The portal it placed is drawn when it lands
//! (`ShownPortals`): until then, the portals shown keep that one as it was drawn. Only what is
//! drawn waits: the client map, which the simulation reads, has the portal at once.
//!
//! The local player's shots come from its prediction, the others' from the server
//! (`ServerToClientMessage::PortalShot`), both as `ShotFired`.

use bevy::{
    asset::RenderAssetUsages,
    camera::Hdr,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use rand::{thread_rng, Rng};
use shared::messages::PlayerId;
use shared::players::blocks::PortalShot;
use shared::world::{PortalColor, Portals};
use std::collections::HashMap;

use super::particles::spawn_particle;
use super::{portal_color, DrawnPortals};
use crate::camera::CameraController;
use crate::state::GameState;
use crate::world::ClientWorldMap;

/// How fast a bolt flies, in blocks per second: a third of a second at the gun's reach.
pub const BOLT_SPEED: f32 = 100.0;
/// How far ahead of the eye of the local player its own bolts start, in blocks, so that a bolt
/// does not fill the screen on its first frame.
pub const SHOOTER_OFFSET: f32 = 0.6;
/// How long a point of the trail lasts behind the bolt, in seconds: the trail is as long as
/// the bolt flies in that time, and shrinks into the hit point once it landed.
const TRAIL_TIME: f32 = 0.06;
/// How many sparks a bolt bursts into.
const SPARKS: usize = 8;
/// How much brighter than its portal a bolt is: brighter than white, which the bloom of the
/// shaders makes glow, and which the tonemapping without them turns to a light, still saturated
/// colour.
const BOLT_GLOW: f32 = 1.5;
const TRAIL_GLOW: f32 = 1.0;
const SPARK_GLOW: f32 = 1.5;
/// How much brighter still a bolt is with the shaders, whose physical sky is far brighter than
/// white.
const HDR_BOOST: f32 = 4.0;

/// A shot of the portal gun to show: by the local player, as predicted, or by another one.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ShotFired {
    pub shooter: PlayerId,
    pub shot: PortalShot,
}

impl ShotFired {
    /// A shot of the local player, `shooter`, as its prediction made it: its bolt starts
    /// `SHOOTER_OFFSET` ahead of the eye, or on the hit point when that is nearer.
    pub fn predicted(shooter: PlayerId, shot: PortalShot) -> Self {
        let ahead = (shot.to - shot.from).clamp_length_max(SHOOTER_OFFSET);
        Self {
            shooter,
            shot: PortalShot {
                from: shot.from + ahead,
                ..shot
            },
        }
    }
}

/// How long the bolt of `shot` flies, in seconds.
pub fn flight_time(shot: &PortalShot) -> f32 {
    shot.from.distance(shot.to) / BOLT_SPEED
}

/// Where the bolt of `shot` is `age` seconds after it was fired: on the hit point once landed.
fn bolt_at(shot: &PortalShot, age: f32) -> Vec3 {
    let flight = flight_time(shot);
    if age >= flight {
        return shot.to;
    }
    shot.from.lerp(shot.to, age / flight)
}

/// The tail and the head of the trail of the bolt of `shot`, `age` seconds after it was fired.
fn trail_at(shot: &PortalShot, age: f32) -> (Vec3, Vec3) {
    (
        bolt_at(shot, (age - TRAIL_TIME).max(0.0)),
        bolt_at(shot, age),
    )
}

/// A bolt flying, or its trail shrinking after it burst.
#[derive(Component, Debug)]
pub struct Bolt {
    shooter: PlayerId,
    shot: PortalShot,
    /// Seconds since it was fired.
    pub age: f32,
    trail: Entity,
    /// Whether it landed and burst.
    burst: bool,
    spark: (Handle<Mesh>, Handle<StandardMaterial>),
}

impl Bolt {
    /// How long it flies, in seconds.
    pub fn flight(&self) -> f32 {
        flight_time(&self.shot)
    }

    /// Where it is.
    pub fn position(&self) -> Vec3 {
        bolt_at(&self.shot, self.age)
    }

    /// The portal that waits for this bolt to land before it is drawn: that of its colour of its
    /// shooter, if the shot placed it.
    fn holds(&self) -> Option<(PlayerId, PortalColor)> {
        (self.shot.placed && self.age < flight_time(&self.shot))
            .then_some((self.shooter, self.shot.color))
    }
}

/// The trail of a bolt.
#[derive(Component, Debug)]
pub struct Trail;

/// The portals drawn while a placed shot's bolt flies: the client map's, with the portal it
/// placed as it was drawn before (or none). `None` when no such bolt flies: the client map's
/// portals are drawn as they are.
#[derive(Resource, Default)]
pub struct ShownPortals(pub(super) Option<Portals>);

impl ShownPortals {
    /// The portals to draw, `map` being the client map's.
    pub fn get<'a>(&'a self, map: &'a Portals) -> &'a Portals {
        self.0.as_ref().unwrap_or(map)
    }
}

/// What the bolts of one colour are drawn with.
pub(super) struct BoltLook {
    bolt: Handle<StandardMaterial>,
    trail: Handle<StandardMaterial>,
    spark: Handle<StandardMaterial>,
}

/// The meshes and materials of the bolts, made with the first one.
pub(super) struct BoltAssets {
    bolt: Handle<Mesh>,
    trail: Handle<Mesh>,
    spark: Handle<Mesh>,
    blue: BoltLook,
    orange: BoltLook,
}

impl BoltAssets {
    /// The assets for a camera with HDR (`hdr`), where a bolt must be brighter to glow over the
    /// physical sky, or without.
    fn new(meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, hdr: bool) -> Self {
        let boost = if hdr { HDR_BOOST } else { 1.0 };
        let mut look = |color: PortalColor| {
            let linear = LinearRgba::from(portal_color(color));
            let mut glowing = |base: LinearRgba, glow: f32| {
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(LinearRgba {
                        alpha: 1.0,
                        ..base * (glow * boost)
                    }),
                    unlit: true,
                    fog_enabled: false,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    ..default()
                })
            };
            BoltLook {
                bolt: glowing(linear, BOLT_GLOW),
                trail: glowing(linear, TRAIL_GLOW),
                spark: glowing(linear, SPARK_GLOW),
            }
        };
        let (blue, orange) = (look(PortalColor::Blue), look(PortalColor::Orange));
        Self {
            bolt: meshes.add(Capsule3d::new(0.05, 0.4)),
            trail: meshes.add(trail_mesh()),
            spark: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
            blue,
            orange,
        }
    }

    fn look(&self, color: PortalColor) -> &BoltLook {
        match color {
            PortalColor::Blue => &self.blue,
            PortalColor::Orange => &self.orange,
        }
    }
}

/// The trail, a hexagonal frustum along +Y from its tail at 0, thin and transparent, to its
/// head at 1, as wide as the bolt and opaque. Scaled along Y to its length.
fn trail_mesh() -> Mesh {
    const SIDES: usize = 6;
    const WIDTH: f32 = 0.04;
    let mut positions = Vec::new();
    let mut colors = Vec::new();
    for side in 0..SIDES {
        let angle = side as f32 * std::f32::consts::TAU / SIDES as f32;
        let (sin, cos) = angle.sin_cos();
        positions.push([cos * WIDTH * 0.3, 0.0, sin * WIDTH * 0.3]);
        colors.push([1.0, 1.0, 1.0, 0.0]);
        positions.push([cos * WIDTH, 1.0, sin * WIDTH]);
        colors.push([1.0, 1.0, 1.0, 1.0]);
    }
    let mut indices = Vec::new();
    for side in 0..SIDES as u32 {
        let next = (side + 1) % SIDES as u32;
        let (tail, head, next_tail, next_head) = (2 * side, 2 * side + 1, 2 * next, 2 * next + 1);
        indices.extend([tail, head, next_head, tail, next_head, next_tail]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
    .with_inserted_indices(Indices::U32(indices))
}

/// The rotation that turns +Y, along which the bolt and its trail are made, toward `direction`.
fn along(direction: Vec3) -> Quat {
    Quat::from_rotation_arc(Vec3::Y, direction.try_normalize().unwrap_or(Vec3::Y))
}

/// Spawns a bolt and its trail for each shot fired.
pub(super) fn spawn_bolts(
    mut fired: MessageReader<ShotFired>,
    mut assets: Local<Option<(bool, BoltAssets)>>,
    camera: Query<Has<Hdr>, With<CameraController>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for ShotFired { shooter, shot } in fired.read() {
        let hdr = camera.single().unwrap_or(false);
        if assets.as_ref().is_none_or(|(made_for, _)| *made_for != hdr) {
            *assets = Some((hdr, BoltAssets::new(&mut meshes, &mut materials, hdr)));
        }
        let (_, assets) = assets.as_ref().unwrap();
        let look = assets.look(shot.color);
        let rotation = along(shot.to - shot.from);
        let trail = commands
            .spawn((
                Trail,
                Mesh3d(assets.trail.clone()),
                MeshMaterial3d(look.trail.clone()),
                Transform::from_translation(shot.from).with_scale(Vec3::ZERO),
                Visibility::Hidden,
                DespawnOnExit(GameState::Game),
            ))
            .id();
        commands.spawn((
            Bolt {
                shooter: *shooter,
                shot: *shot,
                age: 0.0,
                trail,
                burst: false,
                spark: (assets.spark.clone(), look.spark.clone()),
            },
            Mesh3d(assets.bolt.clone()),
            MeshMaterial3d(look.bolt.clone()),
            Transform::from_translation(shot.from).with_rotation(rotation),
            Visibility::Inherited,
            DespawnOnExit(GameState::Game),
        ));
    }
}

/// Moves the bolts and their trails, bursts those that land into sparks, and despawns them
/// once their trail is gone.
pub(super) fn fly_bolts(
    time: Res<Time>,
    mut bolts: Query<(Entity, &mut Bolt, &mut Transform, &mut Visibility), Without<Trail>>,
    mut trails: Query<(&mut Transform, &mut Visibility), With<Trail>>,
    mut commands: Commands,
) {
    for (entity, mut bolt, mut transform, mut visibility) in &mut bolts {
        bolt.age += time.delta_secs();
        let shot = bolt.shot;
        let flight = bolt.flight();
        // Even when a long frame takes it past the end of its trail at once
        if bolt.age >= flight && !bolt.burst {
            bolt.burst = true;
            *visibility = Visibility::Hidden;
            burst(&mut commands, &shot, &bolt.spark);
        }
        if bolt.age >= flight + TRAIL_TIME {
            commands.entity(bolt.trail).despawn();
            commands.entity(entity).despawn();
            continue;
        }
        transform.translation = bolt.position();
        let (tail, head) = trail_at(&shot, bolt.age);
        if let Ok((mut trail, mut shown)) = trails.get_mut(bolt.trail) {
            let length = tail.distance(head);
            *trail = Transform::from_translation(tail)
                .with_rotation(along(shot.to - shot.from))
                .with_scale(Vec3::new(1.0, length, 1.0));
            *shown = if length > 0.0 {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
        }
    }
}

/// The sparks of a bolt that landed, thrown out around the hit point, a little back toward the
/// shooter.
fn burst(
    commands: &mut Commands,
    shot: &PortalShot,
    (mesh, material): &(Handle<Mesh>, Handle<StandardMaterial>),
) {
    let back = (shot.from - shot.to).try_normalize().unwrap_or(Vec3::Y);
    let mut rng = thread_rng();
    for _ in 0..SPARKS {
        let aside = Vec3::new(
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
            rng.gen_range(-1.0..1.0),
        );
        // Mostly across the shot, so that the shooter sees them spread
        let across = (aside - back * aside.dot(back)).normalize_or_zero();
        let direction = (across + back * rng.gen_range(0.2..0.6)).normalize();
        commands.queue(spawn_particle(
            mesh.clone(),
            material.clone(),
            shot.to + back * 0.05,
            rng.gen_range(0.2..0.4),
            rng.gen_range(0.06..0.12),
            direction * rng.gen_range(3.0..6.0),
        ));
    }
}

/// Keeps `ShownPortals` while placed shots fly: the portals of the client map, but those whose
/// latest bolt has not landed, as they are drawn.
pub(super) fn hold_shot_portals(
    world_map: Res<ClientWorldMap>,
    drawn: Res<DrawnPortals>,
    bolts: Query<&Bolt>,
    mut shown: ResMut<ShownPortals>,
) {
    // Only the latest placed shot of a portal holds it: an earlier one, still flying, placed
    // where the portal no longer is
    let mut latest: HashMap<(PlayerId, PortalColor), &Bolt> = HashMap::new();
    for bolt in bolts.iter().filter(|bolt| bolt.shot.placed) {
        let key = (bolt.shooter, bolt.shot.color);
        if latest.get(&key).is_none_or(|other| bolt.age < other.age) {
            latest.insert(key, bolt);
        }
    }
    let held: Vec<_> = latest.values().filter_map(|bolt| bolt.holds()).collect();
    if held.is_empty() {
        if shown.0.is_some() {
            shown.0 = None;
        }
        return;
    }
    let mut portals = world_map.blocks.portals().clone();
    for (owner, color) in held {
        match drawn.0.get(&(owner, color)) {
            Some(drawn) => portals.insert(drawn.portal),
            None => {
                portals.remove(owner, color);
            }
        }
    }
    shown.0 = Some(portals);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot(from: Vec3, to: Vec3) -> PortalShot {
        PortalShot {
            from,
            to,
            color: PortalColor::Blue,
            placed: true,
        }
    }

    #[test]
    fn a_bolt_flies_a_hundred_blocks_a_second() {
        let far = shot(Vec3::ZERO, Vec3::new(0.0, 0.0, -32.0));
        assert!((flight_time(&far) - 0.32).abs() < 1e-6);
        let near = shot(Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 6.0, 3.0));
        assert!((flight_time(&near) - 0.05).abs() < 1e-6);
    }

    #[test]
    fn a_bolt_goes_from_the_eye_to_the_wall_its_trail_behind() {
        let shot = shot(Vec3::ZERO, Vec3::new(0.0, 0.0, -32.0));
        assert_eq!(bolt_at(&shot, 0.0), Vec3::ZERO);
        assert!(bolt_at(&shot, 0.16).distance(Vec3::new(0.0, 0.0, -16.0)) < 1e-4);
        // It stays where it landed
        assert_eq!(bolt_at(&shot, 1.0), shot.to);
        // The trail is as long as the bolt flew in `TRAIL_TIME`, shorter at first, and shrinks
        // into the wall once the bolt landed
        let trail = |age| trail_at(&shot, age);
        assert_eq!(trail(0.0), (Vec3::ZERO, Vec3::ZERO));
        let (tail, head) = trail(0.16);
        assert!((tail.distance(head) - TRAIL_TIME * BOLT_SPEED).abs() < 1e-3);
        let (tail, head) = trail(0.32 + TRAIL_TIME / 2.0);
        assert_eq!(head, shot.to);
        assert!((tail.distance(head) - TRAIL_TIME * BOLT_SPEED / 2.0).abs() < 1e-3);
        assert_eq!(trail(0.32 + TRAIL_TIME), (shot.to, shot.to));
    }

    #[test]
    fn the_local_players_bolt_starts_a_little_ahead_of_its_eye() {
        let fired = ShotFired::predicted(1, shot(Vec3::ZERO, Vec3::new(0.0, 0.0, -32.0)));
        assert_eq!(fired.shot.from, Vec3::new(0.0, 0.0, -SHOOTER_OFFSET));
        assert_eq!(fired.shot.to, Vec3::new(0.0, 0.0, -32.0));
        let close = ShotFired::predicted(1, shot(Vec3::ZERO, Vec3::new(0.0, 0.0, -0.2)));
        assert_eq!(close.shot.from, close.shot.to);
    }

    #[test]
    fn a_shot_at_its_own_eye_lands_at_once() {
        let here = shot(Vec3::ONE, Vec3::ONE);
        assert_eq!(flight_time(&here), 0.0);
        assert_eq!(bolt_at(&here, 0.0), Vec3::ONE);
    }
}
