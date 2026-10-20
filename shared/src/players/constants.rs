// Vertical motion is in blocks per second and is integrated with the frame delta.
//
// A jump rises like in Minecraft: 1.25 blocks, the top reached in about 0.3 s. The jump
// velocity is the one that peaks there under `GRAVITY`: v = sqrt(2 * 30 * 1.25) = 8.66.
// The upward velocity is capped at 54 blocks/s.
pub const GRAVITY: f32 = -30.0;
pub const JUMP_VELOCITY: f32 = 8.66;
pub const MAX_UPWARD_VELOCITY: f32 = 54.0;
pub const FLY_SPEED_MULTIPLIER: f32 = 4.0;
pub const SPEED: f32 = 5.0;
/// Speed of a sprinting player on foot, against `SPEED`: 1.3 times, as in Minecraft.
pub const SPRINT_SPEED_MULTIPLIER: f32 = 1.3;
/// Speed of a sprinting player in flight, against the flying speed: twice, as in Minecraft.
pub const FLY_SPRINT_SPEED_MULTIPLIER: f32 = 2.0;

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
