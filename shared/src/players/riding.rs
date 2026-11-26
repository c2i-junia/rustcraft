//! Riding a mount: a tamed and saddled horse, or a boat of planks. The rider's movement carries
//! the mount: the simulation of its inputs moves the mount's box, which the client predicts and
//! the server repeats as for a player on foot, and the rider sits on it. The server then puts
//! the mob where the rider's mount is (`server::mob::riding`). The rider gets off with the
//! sneak key.
//!
//! The horse walks at its own speed, gallops faster while the sprint key is held for as long
//! as its breath lasts, and jumps as high as the jump key was held long (`CHARGE_MS`). The boat
//! is fast on the water and crawls on land; the second seat of a boat is a passenger's, whom
//! the server carries.

use bevy::math::{bounding::Aabb3d, Vec3};
use serde::{Deserialize, Serialize};

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        constants::{GRAVITY, TERMINAL_VELOCITY},
        movement::MovementOutcome,
        Player, PLAYER_HEIGHT,
    },
    world::{BlockTransparency, ChunkMap, HorseTraits, MobId, MobKind},
};

/// How long the jump key is held for a horse's highest jump, in ms.
pub const CHARGE_MS: u32 = 1000;
/// A jump let go at once is this part of the highest.
const LEAST_CHARGE: f32 = 0.3;
/// How much faster a horse gallops than it walks.
pub const GALLOP: f32 = 1.6;
/// A winded horse gallops again once it has this part of its breath back.
const SECOND_WIND: f32 = 0.3;
/// How much slower a horse goes backwards, sideways and in water.
const BACKWARDS: f32 = 0.4;
const SIDEWAYS: f32 = 0.5;
const HORSE_SWIM: f32 = 0.35;
/// How fast a horse in water rises to the surface, in blocks/s.
const HORSE_RISE: f32 = 1.5;
/// How high a horse steps up without jumping: a whole block.
const HORSE_STEP: f32 = 1.0 + 2.0 * CONTACT_GAP;
/// Speed of a boat on the water and on land, in blocks/s.
pub const BOAT_WATER_SPEED: f32 = 8.0;
pub const BOAT_LAND_SPEED: f32 = 1.0;
/// How fast a boat's speed goes to the one wished, per second: it glides on the water.
const BOAT_GRIP: f32 = 2.0;
const BOAT_LAND_GRIP: f32 = 10.0;
/// How fast a sunk boat rises, in blocks/s.
const BOAT_RISE: f32 = 2.5;
/// How much a horse spares its rider of a fall, in blocks.
const HORSE_FALL_CUSHION: f32 = 3.0;
/// Room left between a mount and the block it lands on or bumps into.
const CONTACT_GAP: f32 = 1e-3;
/// Longest move tested at once, in blocks.
const MAX_STEP: f32 = 0.5;

/// What a rider knows of its mount. Part of the `MovementState`, which the server sends: the
/// client predicts the ride as it predicts a walk, and the others see the rider seated.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Riding {
    pub mob: MobId,
    pub kind: MobKind,
    /// 0 drives; a boat has a second seat, at its front, for a passenger.
    pub seat: u8,
    /// Where the mount faces, the angle about the vertical of its forward from +Z, as a mob's
    /// rotation: `Quat::from_rotation_y(heading)`.
    pub heading: f32,
    /// A horse's walk, in blocks/s, its highest jump, in blocks, and how long it gallops, in ms.
    pub speed: f32,
    pub jump: f32,
    pub endurance_ms: u32,
    /// The gallop it has left, in ms, and whether it ran out of it, until it gets its second
    /// wind (`SECOND_WIND`).
    pub breath_ms: u32,
    pub winded: bool,
    /// How long the jump key has been held on the ground, in ms, up to `CHARGE_MS`.
    pub charge_ms: u32,
}

impl Riding {
    /// The ride of the horse `mob`, born with `traits`, facing `heading`.
    pub fn horse(mob: MobId, traits: &HorseTraits, heading: f32) -> Self {
        let endurance_ms = (traits.endurance * 1000.0) as u32;
        Self {
            mob,
            kind: MobKind::Horse,
            seat: 0,
            heading,
            speed: traits.speed,
            jump: traits.jump,
            endurance_ms,
            breath_ms: endurance_ms,
            winded: false,
            charge_ms: 0,
        }
    }

    /// The ride of the boat `mob`, on `seat`, facing `heading`.
    pub fn boat(mob: MobId, seat: u8, heading: f32) -> Self {
        Self {
            mob,
            kind: MobKind::Boat,
            seat,
            heading,
            speed: BOAT_WATER_SPEED,
            jump: 0.0,
            endurance_ms: 0,
            breath_ms: 0,
            winded: false,
            charge_ms: 0,
        }
    }

    /// The part of its gallop the horse has left, from 0 to 1.
    pub fn breath(&self) -> f32 {
        if self.endurance_ms == 0 {
            return 0.0;
        }
        self.breath_ms as f32 / self.endurance_ms as f32
    }

    /// How charged the jump is, from 0 to 1.
    pub fn charge(&self) -> f32 {
        self.charge_ms as f32 / CHARGE_MS as f32
    }

    /// Where the rider's centre is against the mount's.
    pub fn seat_offset(&self) -> Vec3 {
        seat_offset(self.kind, self.seat, self.heading)
    }
}

/// The forward of a mount facing `heading`.
pub fn forward(heading: f32) -> Vec3 {
    Vec3::new(heading.sin(), 0.0, heading.cos())
}

/// Where the centre of the rider on `seat` of a mount of `kind` facing `heading` is against the
/// centre of the mount: its hips on the back of the horse, 1.31 above the ground, or on the
/// bench of the boat; the driver of a boat at its back, the passenger at its front. The legs of
/// a rider are bent forward, its hips 0.75 above its feet.
pub fn seat_offset(kind: MobKind, seat: u8, heading: f32) -> Vec3 {
    let hips = match kind {
        MobKind::Horse => 1.31,
        MobKind::Boat => 0.25,
        _ => kind.size().y,
    };
    let up = hips - kind.size().y / 2.0 - 0.75 + PLAYER_HEIGHT / 2.0;
    let along = match (kind, seat) {
        (MobKind::Boat, 0) => -0.4,
        (MobKind::Boat, _) => 0.45,
        _ => 0.0,
    };
    Vec3::Y * up + forward(heading) * along
}

/// How many riders a mount of `kind` carries.
pub fn seats(kind: MobKind) -> u8 {
    match kind {
        MobKind::Boat => 2,
        _ => 1,
    }
}

/// Whether the box centred on `centre` of half size `half` overlaps a block.
fn blocked(world: &ChunkMap, centre: Vec3, half: Vec3) -> bool {
    world.check_collision_box(&Aabb3d::new(centre, half))
}

/// Whether the cell of `point` holds a liquid.
fn liquid_at(world: &ChunkMap, point: Vec3) -> bool {
    world
        .get_block(point.floor().as_ivec3())
        .is_some_and(|block| block.id.get_visibility() == BlockTransparency::Liquid)
}

/// Takes `player` off its mount: it stands beside it where there is room, else on top of it.
pub fn dismount(player: &mut Player, world: &ChunkMap) {
    let Some(riding) = player.riding.take() else {
        return;
    };
    let half_mount = riding.kind.size() / 2.0;
    let centre = player.position - riding.seat_offset();
    let half_player = Vec3::new(player.width, player.height, player.width) / 2.0;
    let ground = centre.y - half_mount.y + half_player.y + CONTACT_GAP;
    let ahead = forward(riding.heading);
    let side = Vec3::new(ahead.z, 0.0, -ahead.x);
    let away = half_mount.x + half_player.x + 0.1;
    let spot = [side, -side, -ahead, ahead]
        .into_iter()
        .map(|direction| (centre + direction * away).with_y(ground))
        .find(|&spot| !blocked(world, spot, half_player))
        .unwrap_or(centre.with_y(centre.y + half_mount.y + half_player.y + CONTACT_GAP));
    player.position = spot;
    player.velocity = Vec3::ZERO;
    player.fall_distance = 0.0;
    player.on_ground = false;
}

/// One input of a rider: it gets off with the sneak key; the driver moves the mount, which
/// carries it, and the passenger waits for the server to carry it.
pub fn simulate_riding(
    player: &mut Player,
    world: &ChunkMap,
    action: &PlayerFrameInput,
    delta_ms: u64,
) -> MovementOutcome {
    player.camera_transform = action.camera;
    player.is_flying = false;
    player.is_sprinting = false;
    player.is_sneaking = false;
    player.is_swimming = false;
    if action.is_pressed(NetworkAction::SneakOrFlyDown) {
        dismount(player, world);
        return MovementOutcome::default();
    }
    let Some(mut riding) = player.riding else {
        return MovementOutcome::default();
    };
    if riding.seat != 0 {
        return MovementOutcome::default();
    }
    let delta = delta_ms as f32 / 1000.0;
    let half = riding.kind.size() / 2.0;
    let mut centre = player.position - riding.seat_offset();
    let mut velocity = player.velocity;
    let horse = riding.kind == MobKind::Horse;

    // Where the rider looks steers the mount
    let horizontal = |v: Vec3| v.with_y(0.0).normalize_or_zero();
    let look = horizontal(action.camera.forward().as_vec3());
    let right = horizontal(action.camera.right().as_vec3());
    let pressed = |a| action.is_pressed(a) as i32 as f32;
    let ahead = pressed(NetworkAction::MoveForward) - pressed(NetworkAction::MoveBackward);
    let aside = if horse {
        pressed(NetworkAction::MoveRight) - pressed(NetworkAction::MoveLeft)
    } else {
        0.0
    };
    if (ahead != 0.0 || aside != 0.0) && look != Vec3::ZERO {
        riding.heading = look.x.atan2(look.z);
    }

    let in_water = liquid_at(world, centre);
    let afloat = !in_water && liquid_at(world, centre - Vec3::Y * (half.y - 0.15));

    // The speed it goes at
    let galloping = horse
        && ahead > 0.0
        && action.is_pressed(NetworkAction::Sprint)
        && !riding.winded
        && riding.breath_ms > 0
        && !in_water;
    if horse {
        if galloping {
            riding.breath_ms = riding.breath_ms.saturating_sub(delta_ms as u32);
            riding.winded = riding.breath_ms == 0;
        } else {
            riding.breath_ms = (riding.breath_ms + delta_ms as u32 / 2).min(riding.endurance_ms);
            riding.winded &= riding.breath() < SECOND_WIND;
        }
    }
    let wished = if horse {
        let mut speed = riding.speed;
        if galloping {
            speed *= GALLOP;
        }
        if in_water || afloat {
            speed *= HORSE_SWIM;
        }
        let backwards = if ahead < 0.0 { BACKWARDS } else { 1.0 };
        (look * ahead * backwards + right * aside * SIDEWAYS) * speed
    } else {
        let on_water = in_water || afloat;
        let speed = if on_water {
            BOAT_WATER_SPEED
        } else {
            BOAT_LAND_SPEED
        };
        let backwards = if ahead < 0.0 { BACKWARDS } else { 1.0 };
        look * ahead * backwards * speed
    };
    if horse {
        // A horse in the air keeps its run
        if player.on_ground || in_water || afloat {
            velocity.x = wished.x;
            velocity.z = wished.z;
        }
    } else {
        let rate = if in_water || afloat {
            BOAT_GRIP
        } else {
            BOAT_LAND_GRIP
        };
        let grip = 1.0 - (-rate * delta).exp();
        velocity.x += (wished.x - velocity.x) * grip;
        velocity.z += (wished.z - velocity.z) * grip;
    }

    // Up and down: the water carries it, a horse jumps
    let jump_held = action.is_pressed(NetworkAction::JumpOrFlyUp);
    let mut jumped = false;
    if in_water {
        velocity.y = if horse { HORSE_RISE } else { BOAT_RISE };
        riding.charge_ms = 0;
    } else if afloat {
        velocity.y = velocity.y.max(0.0);
        if velocity.y == 0.0 || !horse {
            velocity.y = 0.0;
        }
        riding.charge_ms = 0;
    } else {
        if horse && player.on_ground {
            if jump_held {
                riding.charge_ms = (riding.charge_ms + delta_ms as u32).min(CHARGE_MS);
            } else if riding.charge_ms > 0 {
                let charge = LEAST_CHARGE + (1.0 - LEAST_CHARGE) * riding.charge();
                velocity.y = (-2.0 * GRAVITY * riding.jump * charge).sqrt();
                riding.charge_ms = 0;
                jumped = true;
            }
        }
        if !jumped {
            velocity.y += GRAVITY * delta;
        }
    }
    velocity.y = velocity.y.clamp(-TERMINAL_VELOCITY, TERMINAL_VELOCITY);

    // The vertical move, stopped by the blocks
    let was_on_ground = player.on_ground;
    let mut on_ground = false;
    let mut landed = None;
    let distance = velocity.y * delta;
    let steps = (distance.abs() / MAX_STEP).ceil().max(1.0);
    for _ in 0..steps as u32 {
        let moved = centre + Vec3::Y * (distance / steps);
        if !blocked(world, moved, half) {
            player.fall_distance += (centre.y - moved.y).max(0.0);
            centre = moved;
            continue;
        }
        let falling = distance < 0.0;
        let contact = if falling {
            (moved.y - half.y).floor() + 1.0 + half.y + CONTACT_GAP
        } else {
            (moved.y + half.y).floor() - half.y - CONTACT_GAP
        };
        let between = if falling {
            (moved.y..=centre.y).contains(&contact)
        } else {
            (centre.y..=moved.y).contains(&contact)
        };
        if between && !blocked(world, centre.with_y(contact), half) {
            player.fall_distance += (centre.y - contact).max(0.0);
            centre.y = contact;
        }
        velocity.y = 0.0;
        on_ground = falling;
        if falling && !was_on_ground {
            landed = Some(std::mem::take(&mut player.fall_distance));
        }
        break;
    }
    if on_ground || in_water || afloat {
        player.fall_distance = 0.0;
    }

    // The horizontal move, axis by axis; a horse steps up a block
    let step = velocity.with_y(0.0) * delta;
    for axis in [Vec3::X, Vec3::Z] {
        let along = step.dot(axis);
        if along == 0.0 {
            continue;
        }
        let steps = (along.abs() / MAX_STEP).ceil();
        for _ in 0..steps as u32 {
            let moved = centre + axis * (along / steps);
            if !blocked(world, moved, half) {
                centre = moved;
                continue;
            }
            let up = Vec3::Y * HORSE_STEP;
            if horse
                && (on_ground || was_on_ground)
                && !blocked(world, centre + up, half)
                && !blocked(world, moved + up, half)
            {
                centre = moved + up;
                continue;
            }
            velocity *= Vec3::ONE - axis;
            break;
        }
    }

    player.riding = Some(riding);
    player.on_ground = on_ground;
    player.velocity = velocity;
    player.position = centre + riding.seat_offset();
    let fallen = landed
        .filter(|_| horse)
        .map(|fallen| (fallen - HORSE_FALL_CUSHION).max(0.0));
    MovementOutcome {
        fallen,
        turned: None,
        exhaustion: 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockId, Coat};
    use bevy::math::IVec3;
    use bevy::prelude::Transform;

    fn traits() -> HorseTraits {
        HorseTraits {
            coat: Coat::Bay,
            speed: 8.0,
            jump: 2.0,
            endurance: 2.0,
        }
    }

    /// A world of stone under y = 0, water from `water` up to y = 0 if any.
    fn world(water: Option<(IVec3, IVec3)>) -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-32), IVec3::splat(31));
        world.fill_box(
            IVec3::new(-32, -8, -32),
            IVec3::new(31, -1, 31),
            BlockId::Stone,
        );
        if let Some((from, to)) = water {
            world.fill_box(from, to, BlockId::Water);
        }
        world
    }

    fn rider(riding: Riding, mount_centre: Vec3) -> Player {
        let mut player = Player::new(1, "rider".into(), Vec3::ZERO, Transform::default());
        player.riding = Some(riding);
        player.position = mount_centre + riding.seat_offset();
        player
    }

    fn input(actions: &[NetworkAction]) -> PlayerFrameInput {
        PlayerFrameInput {
            delta_ms: 50,
            inputs: actions.iter().cloned().collect(),
            // Looking north, -Z
            camera: Transform::default(),
            ..Default::default()
        }
    }

    fn mount_centre(player: &Player) -> Vec3 {
        player.position - player.riding.unwrap().seat_offset()
    }

    #[test]
    fn a_horse_gallops_faster_than_it_walks_until_it_is_out_of_breath() {
        let world = world(None);
        let start = Vec3::new(0.0, 0.85 + 0.01, 20.0);
        let mut walker = rider(Riding::horse(1, &traits(), 0.0), start);
        let mut galloper = walker.clone();
        for _ in 0..20 {
            simulate_riding(
                &mut walker,
                &world,
                &input(&[NetworkAction::MoveForward]),
                50,
            );
            simulate_riding(
                &mut galloper,
                &world,
                &input(&[NetworkAction::MoveForward, NetworkAction::Sprint]),
                50,
            );
        }
        let walked = start.z - mount_centre(&walker).z;
        let galloped = start.z - mount_centre(&galloper).z;
        assert!(walked > 7.0 && walked < 8.5, "{walked}");
        assert!(galloped > walked * 1.4, "{galloped} against {walked}");
        // Two seconds of endurance: winded after a second more
        for _ in 0..20 {
            simulate_riding(
                &mut galloper,
                &world,
                &input(&[NetworkAction::MoveForward, NetworkAction::Sprint]),
                50,
            );
        }
        assert!(galloper.riding.unwrap().winded);
    }

    #[test]
    fn a_horse_jumps_higher_the_longer_the_jump_key_was_held() {
        let world = world(None);
        let start = Vec3::new(0.0, 0.85 + 0.01, 0.0);
        let peak = |held: u32| {
            let mut player = rider(Riding::horse(1, &traits(), 0.0), start);
            simulate_riding(&mut player, &world, &input(&[]), 50);
            for _ in 0..held {
                simulate_riding(
                    &mut player,
                    &world,
                    &input(&[NetworkAction::JumpOrFlyUp]),
                    50,
                );
            }
            let mut top = f32::MIN;
            for _ in 0..40 {
                simulate_riding(&mut player, &world, &input(&[]), 50);
                top = top.max(mount_centre(&player).y);
            }
            assert!(player.on_ground);
            top - start.y
        };
        let short = peak(1);
        let long = peak(20);
        assert!(long > short + 0.8, "{short} then {long}");
        assert!(long < traits().jump + 0.5, "{long}");
    }

    #[test]
    fn a_boat_is_fast_on_the_water_and_slow_on_land_and_its_rider_gets_off() {
        let water = world(Some((IVec3::new(-32, -3, -32), IVec3::new(31, -1, 31))));
        let land = world(None);
        let start = Vec3::new(0.0, 0.31, 20.0);
        let forward = input(&[NetworkAction::MoveForward]);
        let mut sailor = rider(Riding::boat(1, 0, 0.0), start);
        let mut crawler = sailor.clone();
        for _ in 0..40 {
            simulate_riding(&mut sailor, &water, &forward, 50);
            simulate_riding(&mut crawler, &land, &forward, 50);
        }
        let sailed = start.z - mount_centre(&sailor).z;
        let crawled = start.z - mount_centre(&crawler).z;
        assert!(sailed > 8.0, "{sailed}");
        assert!(crawled < 2.5, "{crawled}");
        // Afloat, not sunk
        assert!(
            (mount_centre(&sailor).y - 0.3).abs() < 0.35,
            "{:?}",
            sailor.position
        );

        simulate_riding(
            &mut crawler,
            &land,
            &input(&[NetworkAction::SneakOrFlyDown]),
            50,
        );
        assert!(crawler.riding.is_none());
        assert!(!blocked(
            &land,
            crawler.position,
            Vec3::new(crawler.width, crawler.height, crawler.width) / 2.0
        ));
    }
}
