//! The model of a player: a body of six boxes (head, torso, two arms, two legs) cut from a skin,
//! and how it moves. A skin is a 64x64 image laid out as the skins of block games usually are
//! (`scripts/generate_skins.py` draws the ones shipped), each box with a second, slightly larger
//! layer over it (hat, jacket, sleeves, trousers) that is transparent where the skin leaves it
//! empty.
//!
//! The animation is computed, not played from clips: the legs and arms swing with the speed on
//! the ground, the head follows the look of the player and the body follows the head, the arm
//! strikes while the player swings, the torso leans while it sneaks, and a swimmer lies flat.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::RenderLayers,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use shared::players::{Player, ViewMode};

use crate::camera::{CameraController, EyeThroughPortal};
use crate::ui::hud::UIMode;

use super::hand::{Grip, ItemHolder};
use super::CurrentPlayerMarker;

/// Size of a skin, in pixels.
pub const SKIN_SIZE: u32 = 64;

/// Pixels of the skin's boxes from the soles to the top of the head.
pub(crate) const MODEL_PIXELS: f32 = 32.0;

/// By how many pixels the second layer stands out of the first, on each side.
pub(crate) const LAYER_INFLATE: f32 = 0.25;
/// The head's second layer (the hat) stands out more, so that hair and hats show.
const HAT_INFLATE: f32 = 0.5;

/// A part of the body.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyPart {
    Head,
    Torso,
    RightArm,
    LeftArm,
    RightLeg,
    LeftLeg,
}

impl BodyPart {
    pub const ALL: [BodyPart; 6] = [
        BodyPart::Torso,
        BodyPart::Head,
        BodyPart::RightArm,
        BodyPart::LeftArm,
        BodyPart::RightLeg,
        BodyPart::LeftLeg,
    ];

    /// The size of the part in pixels (width along X, height, depth along Z).
    pub fn size(self) -> Vec3 {
        match self {
            BodyPart::Head => Vec3::splat(8.0),
            BodyPart::Torso => Vec3::new(8.0, 12.0, 4.0),
            _ => Vec3::new(4.0, 12.0, 4.0),
        }
    }

    /// Where the faces of the part's box start in the skin, for the first and the second layer.
    pub fn skin_origins(self) -> (UVec2, UVec2) {
        let (first, second) = match self {
            BodyPart::Head => ((0, 0), (32, 0)),
            BodyPart::Torso => ((16, 16), (16, 32)),
            BodyPart::RightArm => ((40, 16), (40, 32)),
            BodyPart::LeftArm => ((32, 48), (48, 48)),
            BodyPart::RightLeg => ((0, 16), (0, 32)),
            BodyPart::LeftLeg => ((16, 48), (0, 48)),
        };
        (UVec2::new(first.0, first.1), UVec2::new(second.0, second.1))
    }

    /// The joint the part turns about, in pixels from the feet of a model facing -Z, its right
    /// toward +X. The head and the arms hang from the torso, whose joint is the hips, and their
    /// joints are given from it.
    fn joint(self) -> Vec3 {
        match self {
            BodyPart::Torso => Vec3::new(0.0, 12.0, 0.0),
            BodyPart::Head => Vec3::new(0.0, 12.0, 0.0),
            BodyPart::RightArm => Vec3::new(6.0, 10.0, 0.0),
            BodyPart::LeftArm => Vec3::new(-6.0, 10.0, 0.0),
            BodyPart::RightLeg => Vec3::new(2.0, 12.0, 0.0),
            BodyPart::LeftLeg => Vec3::new(-2.0, 12.0, 0.0),
        }
    }

    /// Where the centre of the part's box is from its joint, in pixels.
    pub fn offset(self) -> Vec3 {
        match self {
            BodyPart::Torso => Vec3::new(0.0, 6.0, 0.0),
            BodyPart::Head => Vec3::new(0.0, 4.0, 0.0),
            BodyPart::RightArm | BodyPart::LeftArm => Vec3::new(0.0, -4.0, 0.0),
            BodyPart::RightLeg | BodyPart::LeftLeg => Vec3::new(0.0, -6.0, 0.0),
        }
    }

    fn hangs_from_torso(self) -> bool {
        matches!(
            self,
            BodyPart::Head | BodyPart::RightArm | BodyPart::LeftArm
        )
    }
}

/// The faces of a box, with the corners of each as the skin shows it: top left, top right,
/// bottom right, bottom left. `x` is right of a model facing -Z.
const FACES: [(Vec3, [Vec3; 4]); 6] = {
    const fn v(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3::new(x, y, z)
    }
    [
        // Front, seen from -Z: its left is the model's right, +X
        (
            v(0., 0., -1.),
            [
                v(1., 1., -1.),
                v(-1., 1., -1.),
                v(-1., -1., -1.),
                v(1., -1., -1.),
            ],
        ),
        // Back
        (
            v(0., 0., 1.),
            [
                v(-1., 1., 1.),
                v(1., 1., 1.),
                v(1., -1., 1.),
                v(-1., -1., 1.),
            ],
        ),
        // Right, +X: from the back to the front
        (
            v(1., 0., 0.),
            [
                v(1., 1., 1.),
                v(1., 1., -1.),
                v(1., -1., -1.),
                v(1., -1., 1.),
            ],
        ),
        // Left, -X: from the front to the back
        (
            v(-1., 0., 0.),
            [
                v(-1., 1., -1.),
                v(-1., 1., 1.),
                v(-1., -1., 1.),
                v(-1., -1., -1.),
            ],
        ),
        // Top: the back at the top of the skin
        (
            v(0., 1., 0.),
            [
                v(1., 1., 1.),
                v(-1., 1., 1.),
                v(-1., 1., -1.),
                v(1., 1., -1.),
            ],
        ),
        // Bottom: the front at the top of the skin
        (
            v(0., -1., 0.),
            [
                v(1., -1., -1.),
                v(-1., -1., -1.),
                v(-1., -1., 1.),
                v(1., -1., 1.),
            ],
        ),
    ]
};

/// The rectangle of each face of a box of `size` pixels whose faces start at `origin` in the
/// skin, in the order of `FACES`: the usual cross, the top and the bottom above the front and
/// the left, the right, front, left and back side by side below.
pub fn face_rects(origin: UVec2, size: Vec3) -> [Rect; 6] {
    let (w, h, d) = (size.x, size.y, size.z);
    let (u, v) = (origin.x as f32, origin.y as f32);
    let rect = |x: f32, y: f32, fw: f32, fh: f32| Rect::new(x, y, x + fw, y + fh);
    [
        rect(u + d, v + d, w, h),
        rect(u + 2.0 * d + w, v + d, w, h),
        rect(u, v + d, d, h),
        rect(u + d + w, v + d, d, h),
        rect(u + d, v, w, d),
        rect(u + d + w, v, w, d),
    ]
}

/// The mesh of a box of `size` pixels, `inflate` more on each side, each pixel `scale` metres,
/// centred on its origin, its faces cut from the skin at `origin`.
pub fn box_mesh(origin: UVec2, size: Vec3, inflate: f32, scale: f32) -> Mesh {
    let half = (size / 2.0 + Vec3::splat(inflate)) * scale;
    let rects = face_rects(origin, size);
    let mut positions = Vec::with_capacity(24);
    let mut normals = Vec::with_capacity(24);
    let mut uvs = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    for ((normal, corners), rect) in FACES.iter().zip(rects) {
        let base = positions.len() as u32;
        let uv_corners = [
            Vec2::new(rect.min.x, rect.min.y),
            Vec2::new(rect.max.x, rect.min.y),
            Vec2::new(rect.max.x, rect.max.y),
            Vec2::new(rect.min.x, rect.max.y),
        ];
        for (corner, uv) in corners.iter().zip(uv_corners) {
            positions.push((*corner * half).to_array());
            normals.push(normal.to_array());
            uvs.push((uv / SKIN_SIZE as f32).to_array());
        }
        // Counter-clockwise seen from outside
        indices.extend([base, base + 3, base + 2, base, base + 2, base + 1]);
    }
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

/// The root of a player's model, a child of its `PlayerBody`, at the feet, turned to where the
/// body faces.
#[derive(Component)]
pub struct PlayerModelRoot;

/// What the animation of a player keeps from one frame to the next.
#[derive(Component, Debug, Default, Clone)]
pub struct PlayerAnimation {
    /// Where the legs are in their swing, in radians.
    pub walk_phase: f32,
    /// How wide the legs swing, from 0 to 1, eased toward the speed.
    pub walk_amount: f32,
    /// The way the body faces, as a yaw: it follows the head.
    pub body_yaw: f32,
    /// Where the arm is in its strike, from 0 to 1, while it strikes.
    pub swing: Option<f32>,
    /// From 0, standing, to 1, lying flat to swim.
    pub swim: f32,
    /// Where the player was the frame before, which its speed is read from.
    pub last_position: Option<Vec3>,
    /// The horizontal velocity, in blocks per second, smoothed: the other players move by the
    /// ticks of the server, not by the frames.
    pub velocity: Vec2,
}

/// How fast the measured velocity follows the moves, as an exponential rate.
const VELOCITY_SMOOTHING: f32 = 12.0;
/// A move longer than this in a frame is a jump through a portal or a teleport, not a walk.
const TELEPORT: f32 = 2.0;

/// The look of a player: where its head turns, in radians.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Look {
    /// About Y, 0 facing -Z, growing toward the left.
    pub yaw: f32,
    /// Positive looking down.
    pub pitch: f32,
}

impl Look {
    /// The look of a camera turned by `rotation`.
    pub fn of_rotation(rotation: Quat) -> Self {
        let forward = rotation * Vec3::NEG_Z;
        Self {
            yaw: f32::atan2(-forward.x, -forward.z),
            pitch: -forward.y.clamp(-1.0, 1.0).asin(),
        }
    }
}

/// What a player does, as its animation reads it.
#[derive(Debug, Clone, Copy, Default)]
pub struct Motion {
    pub look: Look,
    /// The horizontal velocity, in blocks per second.
    pub velocity: Vec2,
    pub on_ground: bool,
    pub is_sneaking: bool,
    pub is_swimming: bool,
    pub is_flying: bool,
    pub is_swinging: bool,
    /// Whether the player draws a bow.
    pub is_drawing: bool,
}

/// The angles of each joint, in radians, as Euler angles (X, Y, Z), and the lean of the whole
/// model.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Pose {
    pub head: Vec3,
    pub torso: Vec3,
    pub right_arm: Vec3,
    pub left_arm: Vec3,
    pub right_leg: Vec3,
    pub left_leg: Vec3,
    /// The yaw of the whole body.
    pub body_yaw: f32,
    /// The pitch of the whole body, lying forward while it swims.
    pub body_pitch: f32,
}

impl Pose {
    pub fn of(&self, part: BodyPart) -> Vec3 {
        match part {
            BodyPart::Head => self.head,
            BodyPart::Torso => self.torso,
            BodyPart::RightArm => self.right_arm,
            BodyPart::LeftArm => self.left_arm,
            BodyPart::RightLeg => self.right_leg,
            BodyPart::LeftLeg => self.left_leg,
        }
    }
}

/// Speed, in blocks per second, at which the limbs swing their widest.
const FULL_SWING_SPEED: f32 = 4.3;
/// Radians of swing per block walked.
const STRIDE: f32 = 2.0;
/// Widest angle of a leg's swing, in radians.
const LEG_SWING: f32 = 0.9;
/// How far the head turns from the body before the body follows it, in radians.
const MAX_HEAD_TURN: f32 = 0.87;
/// How fast the body turns to the head while the player moves, as an exponential rate.
const BODY_FOLLOW: f32 = 8.0;
/// Seconds of a strike of the arm.
const SWING_SECONDS: f32 = 0.3;
/// How fast the swim pose and the swing width ease, as an exponential rate.
const POSE_EASING: f32 = 10.0;
/// How far the torso leans forward while sneaking, in radians.
const SNEAK_LEAN: f32 = 0.5;

/// The angle from `a` to `b`, between -π and π.
fn angle_between(a: f32, b: f32) -> f32 {
    (b - a + PI).rem_euclid(TAU) - PI
}

impl PlayerAnimation {
    /// Takes the position of the player this frame, `delta` seconds after the last, and gives
    /// its horizontal velocity.
    pub fn measure(&mut self, position: Vec3, delta: f32) -> Vec2 {
        if let Some(last) = self.last_position.replace(position) {
            let moved = (position - last).xz();
            if delta > 0.0 && moved.length() < TELEPORT {
                let t = 1.0 - (-delta * VELOCITY_SMOOTHING).exp();
                self.velocity = self.velocity.lerp(moved / delta, t);
            }
        }
        self.velocity
    }

    /// Moves the animation `delta` seconds forward for a player doing `motion`, and gives its
    /// pose.
    pub fn advance(&mut self, motion: Motion, delta: f32, time: f32) -> Pose {
        let ease = 1.0 - (-delta * POSE_EASING).exp();
        let speed = motion.velocity.length();
        let walking = motion.on_ground || motion.is_swimming;
        let target = if walking {
            (speed / FULL_SWING_SPEED).min(1.0)
        } else {
            0.0
        };
        self.walk_amount += (target - self.walk_amount) * ease;
        if walking {
            self.walk_phase = (self.walk_phase + speed * delta * STRIDE).rem_euclid(TAU);
        }
        self.swim += (f32::from(motion.is_swimming) - self.swim) * ease;

        // The body follows the head: at once when it turns too far, and little by little while
        // the player moves
        let turn = angle_between(self.body_yaw, motion.look.yaw);
        if turn.abs() > MAX_HEAD_TURN {
            self.body_yaw += turn - MAX_HEAD_TURN * turn.signum();
        }
        if speed > 0.1 {
            let follow = 1.0 - (-delta * BODY_FOLLOW).exp();
            self.body_yaw += angle_between(self.body_yaw, motion.look.yaw) * follow;
        }
        self.body_yaw = self.body_yaw.rem_euclid(TAU);

        // A strike runs to its end, and another starts while the button is held
        self.swing = match self.swing {
            Some(t) if t + delta / SWING_SECONDS < 1.0 => Some(t + delta / SWING_SECONDS),
            _ if motion.is_swinging => Some(0.0),
            _ => None,
        };

        let swing = self.walk_phase.sin() * LEG_SWING * self.walk_amount;
        // The arms sway a little at rest
        let idle = (time * 1.2).sin() * 0.04;
        let mut pose = Pose {
            head: Vec3::new(
                -motion.look.pitch,
                angle_between(self.body_yaw, motion.look.yaw),
                0.0,
            ),
            right_leg: Vec3::new(swing, 0.0, 0.0),
            left_leg: Vec3::new(-swing, 0.0, 0.0),
            right_arm: Vec3::new(-swing * 0.8, 0.0, 0.05 + idle),
            left_arm: Vec3::new(swing * 0.8, 0.0, -0.05 - idle),
            torso: Vec3::ZERO,
            body_yaw: self.body_yaw,
            body_pitch: 0.0,
        };

        if motion.is_sneaking && !motion.is_flying {
            pose.torso.x = -SNEAK_LEAN;
            pose.head.x += SNEAK_LEAN;
            pose.right_arm.x += SNEAK_LEAN * 0.8;
            pose.left_arm.x += SNEAK_LEAN * 0.8;
        }

        if self.swim > 0.0 {
            // Flat, face down, the head raised to look ahead, the arms in a crawl
            pose.body_pitch = -FRAC_PI_2 * self.swim;
            pose.head.x += FRAC_PI_2 * 0.8 * self.swim;
            let stroke = self.walk_phase;
            pose.right_arm.x = pose.right_arm.x.lerp(-PI + stroke.sin() * 0.6, self.swim);
            pose.left_arm.x = pose.left_arm.x.lerp(-PI - stroke.sin() * 0.6, self.swim);
        }

        if motion.is_drawing {
            // The right arm holds the bow out where the player looks, the left one the string
            let aim = FRAC_PI_2 - motion.look.pitch - pose.torso.x;
            pose.right_arm = Vec3::new(aim, pose.head.y, 0.0);
            pose.left_arm = Vec3::new(aim, pose.head.y - 0.7, 0.0);
        }

        if let Some(t) = self.swing {
            // Raised forward and brought down across the body
            let arc = (t * PI).sin();
            pose.right_arm.x = -1.4 * arc - 0.3 * motion.look.pitch.min(0.0);
            pose.right_arm.y = 0.4 * arc;
            pose.torso.y += 0.25 * arc;
        }
        pose
    }
}

/// Spawns the model of a player with the skin of `material`, under its `PlayerBody`, for a player
/// `height` metres high, every part on `layers`: the game's, or the preview's of the skin screen.
pub fn spawn_player_model(
    body: &mut ChildSpawnerCommands,
    meshes: &mut Assets<Mesh>,
    material: Handle<StandardMaterial>,
    height: f32,
    layers: RenderLayers,
) {
    let scale = height / MODEL_PIXELS;
    body.spawn((
        PlayerModelRoot,
        Transform::from_translation(Vec3::Y * -height / 2.0),
        Visibility::default(),
        layers.clone(),
    ))
    .with_children(|root| {
        let mut part = |part| part_bundle(part, meshes, &material, scale, &layers);
        for leg in [BodyPart::RightLeg, BodyPart::LeftLeg] {
            root.spawn(part(leg));
        }
        root.spawn(part(BodyPart::Torso)).with_children(|torso| {
            for upper in BodyPart::ALL.into_iter().filter(|p| p.hangs_from_torso()) {
                let mut upper_part = torso.spawn(part(upper));
                if upper == BodyPart::RightArm {
                    // The item the player holds, in the hand, which follows the arm
                    upper_part.with_child((
                        ItemHolder::new(Grip::ThirdPerson),
                        Transform::from_translation(HAND * scale),
                        Visibility::default(),
                        layers.clone(),
                    ));
                }
            }
        });
    });
}

/// Where the right hand holds an item, in pixels from the joint of the arm: near the end of the
/// arm, at its front.
const HAND: Vec3 = Vec3::new(0.0, -10.0, -1.0);

/// A part of the model at its joint, with the boxes of its two layers.
fn part_bundle(
    part: BodyPart,
    meshes: &mut Assets<Mesh>,
    material: &Handle<StandardMaterial>,
    scale: f32,
    layers: &RenderLayers,
) -> impl Bundle {
    let (first, second) = part.skin_origins();
    let inflate = if part == BodyPart::Head {
        HAT_INFLATE
    } else {
        LAYER_INFLATE
    };
    let center = Transform::from_translation(part.offset() * scale);
    (
        part,
        Transform::from_translation(part.joint() * scale),
        Visibility::default(),
        layers.clone(),
        children![
            (
                Mesh3d(meshes.add(box_mesh(first, part.size(), 0.0, scale))),
                MeshMaterial3d(material.clone()),
                center,
                layers.clone(),
            ),
            (
                Mesh3d(meshes.add(box_mesh(second, part.size(), inflate, scale))),
                MeshMaterial3d(material.clone()),
                center,
                layers.clone(),
            )
        ],
    )
}

/// The material of a skin: drawn as its pixels, the transparent ones cut out.
pub fn skin_material(texture: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(texture),
        alpha_mode: AlphaMode::Mask(0.5),
        perceptual_roughness: 1.0,
        reflectance: 0.1,
        ..default()
    }
}

/// Poses each player's model from what it does: the local player from its camera and its
/// buttons, the others from the last update of the server.
pub fn animate_players_system(
    time: Res<Time>,
    mut players: Query<(
        &Player,
        &mut PlayerAnimation,
        &Children,
        Has<CurrentPlayerMarker>,
    )>,
    camera: Query<&CameraController>,
    mouse: Res<ButtonInput<MouseButton>>,
    ui_mode: Res<UIMode>,
    children: Query<&Children>,
    mut roots: Query<&mut Transform, (With<PlayerModelRoot>, Without<BodyPart>)>,
    mut parts: Query<(&BodyPart, &mut Transform), Without<PlayerModelRoot>>,
) {
    let delta = time.delta_secs();
    for (player, mut animation, player_children, local) in &mut players {
        let look = match (local, camera.single()) {
            (true, Ok(controller)) => Look {
                yaw: controller.angle_x,
                pitch: controller.angle_y,
            },
            _ => Look::of_rotation(player.camera_transform.rotation),
        };
        let is_swinging = if local {
            *ui_mode == UIMode::Closed && mouse.pressed(MouseButton::Left) && !player.is_dead()
        } else {
            player.is_swinging
        };
        let motion = Motion {
            look,
            velocity: animation.measure(player.position, delta),
            on_ground: player.on_ground,
            is_sneaking: player.is_sneaking,
            is_swimming: player.is_swimming,
            is_flying: player.is_flying,
            is_swinging,
            is_drawing: player.bow_draw_ms > 0 && !player.is_dead(),
        };
        let pose = animation.advance(motion, delta, time.elapsed_secs());

        for body in player_children {
            for descendant in children.iter_descendants(*body) {
                if let Ok(mut root) = roots.get_mut(descendant) {
                    root.rotation = Quat::from_rotation_y(pose.body_yaw)
                        * Quat::from_rotation_x(pose.body_pitch);
                    // Lying down, the swimmer turns about its middle
                    let lift = player.height / 2.0 * (pose.body_pitch.abs() / FRAC_PI_2);
                    root.translation.y = -player.height / 2.0 + lift * 0.5;
                    root.translation.z = 0.0;
                }
                if let Ok((part, mut transform)) = parts.get_mut(descendant) {
                    let angles = pose.of(*part);
                    transform.rotation =
                        Quat::from_euler(EulerRot::YXZ, angles.y, angles.x, angles.z);
                }
            }
        }
    }
}

/// Hides the local player's model in first person while its camera is drawn from the other side
/// of a portal (`EyeThroughPortal`): the eye is then out of the head, and would see the body
/// below it in the picture through the portal, where a player never sees itself.
pub fn hide_the_body_from_an_eye_through_a_portal(
    eye: Res<EyeThroughPortal>,
    view_mode: Res<ViewMode>,
    players: Query<&Children, With<CurrentPlayerMarker>>,
    children: Query<&Children>,
    mut roots: Query<&mut Visibility, With<PlayerModelRoot>>,
) {
    let visibility = if eye.is_through() && *view_mode == ViewMode::FirstPerson {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for player_children in &players {
        for body in player_children {
            for descendant in children.iter_descendants(*body) {
                if let Ok(mut root) = roots.get_mut(descendant) {
                    root.set_if_neq(visibility);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_faces_of_the_head_lie_where_the_skins_put_them() {
        let (first, second) = BodyPart::Head.skin_origins();
        let rects = face_rects(first, BodyPart::Head.size());
        // Front, back, right, left, top, bottom
        assert_eq!(rects[0], Rect::new(8.0, 8.0, 16.0, 16.0));
        assert_eq!(rects[1], Rect::new(24.0, 8.0, 32.0, 16.0));
        assert_eq!(rects[2], Rect::new(0.0, 8.0, 8.0, 16.0));
        assert_eq!(rects[3], Rect::new(16.0, 8.0, 24.0, 16.0));
        assert_eq!(rects[4], Rect::new(8.0, 0.0, 16.0, 8.0));
        assert_eq!(rects[5], Rect::new(16.0, 0.0, 24.0, 8.0));
        assert_eq!(
            face_rects(second, BodyPart::Head.size())[0].min,
            Vec2::new(40.0, 8.0)
        );
    }

    #[test]
    fn every_face_of_every_part_lies_inside_the_skin_and_none_overlap() {
        let mut taken = vec![false; (SKIN_SIZE * SKIN_SIZE) as usize];
        for part in BodyPart::ALL {
            let (first, second) = part.skin_origins();
            for origin in [first, second] {
                for rect in face_rects(origin, part.size()) {
                    assert!(rect.max.x <= SKIN_SIZE as f32 && rect.max.y <= SKIN_SIZE as f32);
                    for y in rect.min.y as u32..rect.max.y as u32 {
                        for x in rect.min.x as u32..rect.max.x as u32 {
                            let cell = &mut taken[(y * SKIN_SIZE + x) as usize];
                            assert!(!*cell, "{part:?} overlaps at {x}, {y}");
                            *cell = true;
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_box_faces_outward_and_is_as_large_as_its_pixels() {
        let mesh = box_mesh(UVec2::ZERO, Vec3::new(8.0, 12.0, 4.0), 0.0, 0.1);
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("no positions");
        };
        let max = positions
            .iter()
            .fold(Vec3::ZERO, |m, p| m.max(Vec3::from_array(*p)));
        assert!((max - Vec3::new(0.4, 0.6, 0.2)).length() < 1e-6);
        let Some(Indices::U32(indices)) = mesh.indices() else {
            panic!("no indices");
        };
        // Each triangle turns counter-clockwise about its outward normal
        for (face, tri) in indices.chunks(3).enumerate() {
            let [a, b, c] = [0, 1, 2].map(|i| Vec3::from_array(positions[tri[i] as usize]));
            let normal = (b - a).cross(c - a).normalize();
            assert!(
                (normal - FACES[face / 2].0).length() < 1e-5,
                "face {}: {normal}",
                face / 2
            );
        }
    }

    #[test]
    fn the_front_of_the_face_looks_toward_minus_z_with_its_left_eye_at_plus_x() {
        // The left of the front in the skin is the model's right, +X
        let (normal, corners) = FACES[0];
        assert_eq!(normal, Vec3::NEG_Z);
        assert!(corners[0].x > 0.0 && corners[1].x < 0.0);
    }

    #[test]
    fn a_look_is_read_back_from_a_camera_rotation() {
        let rotation = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-0.3);
        let look = Look::of_rotation(rotation);
        assert!((look.yaw - 0.7).abs() < 1e-5, "{look:?}");
        assert!((look.pitch - 0.3).abs() < 1e-5, "{look:?}");
    }

    fn walk(animation: &mut PlayerAnimation, motion: Motion, seconds: f32) -> Pose {
        let mut pose = Pose::default();
        for frame in 0..(seconds * 60.0) as u32 {
            pose = animation.advance(motion, 1.0 / 60.0, frame as f32 / 60.0);
        }
        pose
    }

    #[test]
    fn a_walker_swings_its_legs_and_arms_against_each_other() {
        let mut animation = PlayerAnimation::default();
        let motion = Motion {
            velocity: Vec2::new(0.0, -4.3),
            on_ground: true,
            ..default()
        };
        let mut widest: f32 = 0.0;
        for frame in 0..120 {
            let pose = animation.advance(motion, 1.0 / 60.0, frame as f32 / 60.0);
            assert!((pose.right_leg.x + pose.left_leg.x).abs() < 1e-5);
            assert!(pose.right_leg.x * pose.right_arm.x <= 1e-4);
            widest = widest.max(pose.right_leg.x.abs());
        }
        assert!(widest > 0.6, "{widest}");

        // Standing still, the legs come back together
        let pose = walk(
            &mut animation,
            Motion {
                on_ground: true,
                ..default()
            },
            1.0,
        );
        assert!(pose.right_leg.x.abs() < 0.05, "{pose:?}");
    }

    #[test]
    fn the_body_follows_a_head_that_turns_too_far() {
        let mut animation = PlayerAnimation::default();
        let look = Look {
            yaw: 0.5,
            pitch: 0.0,
        };
        let pose = walk(&mut animation, Motion { look, ..default() }, 0.5);
        assert_eq!(
            pose.body_yaw, 0.0,
            "a small turn of the head is the head's alone"
        );
        assert!((pose.head.y - 0.5).abs() < 1e-5);

        let look = Look {
            yaw: 2.0,
            pitch: 0.4,
        };
        let pose = walk(&mut animation, Motion { look, ..default() }, 0.1);
        assert!((pose.head.y - MAX_HEAD_TURN).abs() < 1e-4, "{pose:?}");
        assert!(
            (pose.head.x + 0.4).abs() < 1e-5,
            "looking down tips the head forward"
        );

        // Moving, the body turns to face where the player looks
        let moving = Motion {
            look,
            velocity: Vec2::new(1.0, 0.0),
            on_ground: true,
            ..default()
        };
        let pose = walk(&mut animation, moving, 2.0);
        assert!(pose.head.y.abs() < 0.01, "{pose:?}");
    }

    #[test]
    fn a_swinging_player_strikes_with_its_right_arm_until_the_strike_ends() {
        let mut animation = PlayerAnimation::default();
        let swinging = Motion {
            is_swinging: true,
            ..default()
        };
        let pose = walk(&mut animation, swinging, SWING_SECONDS / 2.0);
        assert!(pose.right_arm.x < -1.0, "{pose:?}");
        let pose = walk(&mut animation, Motion::default(), SWING_SECONDS);
        assert!(animation.swing.is_none());
        assert!(pose.right_arm.x.abs() < 0.1, "{pose:?}");
    }

    #[test]
    fn the_speed_is_read_from_the_moves_and_a_teleport_is_not_one() {
        let mut animation = PlayerAnimation::default();
        let mut position = Vec3::ZERO;
        for _ in 0..60 {
            position.z -= 4.0 / 60.0;
            animation.measure(position, 1.0 / 60.0);
        }
        assert!((animation.velocity - Vec2::new(0.0, -4.0)).length() < 0.01);
        let before = animation.velocity;
        animation.measure(position + Vec3::X * 50.0, 1.0 / 60.0);
        assert_eq!(animation.velocity, before);
    }

    #[test]
    fn a_swimmer_lies_flat_and_a_sneaker_leans() {
        let mut animation = PlayerAnimation::default();
        let swimming = Motion {
            is_swimming: true,
            ..default()
        };
        let pose = walk(&mut animation, swimming, 1.0);
        assert!((pose.body_pitch + FRAC_PI_2).abs() < 0.01, "{pose:?}");

        let mut animation = PlayerAnimation::default();
        let sneaking = Motion {
            is_sneaking: true,
            on_ground: true,
            ..default()
        };
        let pose = walk(&mut animation, sneaking, 0.2);
        assert_eq!(pose.torso.x, -SNEAK_LEAN);
        assert_eq!(pose.body_pitch, 0.0);
    }
}
