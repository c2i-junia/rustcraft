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
