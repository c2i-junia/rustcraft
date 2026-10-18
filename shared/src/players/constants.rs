// Vertical motion is in blocks per second and is integrated with the frame delta.
//
// These values reproduce the feel of the former per-frame integration at 60 FPS
// (dt = 1/60 s), where a per-frame velocity `v` is `V * dt` for a velocity `V` in blocks/s:
// - the jump set `v = 20 * dt` per frame, i.e. a jump velocity of 20 blocks/s;
// - gravity added `-0.5 * dt` to `v` each frame, i.e. `-0.5 * dt / dt` = -0.5 blocks/s per
//   frame, i.e. an acceleration of `-0.5 * 60` = -30 blocks/s^2;
// - the upward velocity was capped at 0.9 blocks/frame, i.e. `0.9 * 60` = 54 blocks/s.
pub const GRAVITY: f32 = -30.0;
pub const JUMP_VELOCITY: f32 = 20.0;
pub const MAX_UPWARD_VELOCITY: f32 = 54.0;
pub const FLY_SPEED_MULTIPLIER: f32 = 4.0;
pub const SPEED: f32 = 5.0;

// In water, the gravity is weaker and a drag brakes the vertical velocity: the velocity tends
// to `WATER_GRAVITY / WATER_DRAG` = -2 blocks/s, and a fall into water is soon slowed down.
pub const WATER_GRAVITY: f32 = -6.0;
/// Share of the vertical velocity lost per second in water.
pub const WATER_DRAG: f32 = 3.0;
/// Upward velocity of a player holding jump in water, in blocks/s.
pub const SWIM_UP_VELOCITY: f32 = 4.0;
/// Upward velocity of a swimmer pushing against a block while holding jump, enough to rise
/// 1.2 blocks out of the water (v² / 2g = 8.5² / 60) and climb onto the shore.
pub const WATER_EXIT_VELOCITY: f32 = 8.5;
pub const WATER_SPEED_MULTIPLIER: f32 = 0.6;
