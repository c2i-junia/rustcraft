//! The lightning of the thunderstorms: the strikes the server sends, the bolt each draws within
//! the render distance (`bolt.rs`, `render.rs`), and the flash of its strokes, which lights the
//! world, the sky and the clouds. The thunder is `audio::weather`'s.

use bevy::prelude::*;

use crate::state::GameState;
use crate::world::{RenderDistance, ServerRenderDistanceLimit};
use bolt::{Bolt, Strokes, CLOUD_BASE};

pub mod bolt;

/// A lightning strike the server tells of: the block struck, and the seed of its shape.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Strike {
    pub at: IVec3,
    pub seed: u32,
}

/// Within this distance, a strike flashes at its brightest; farther off, its flash fades with
/// the distance, down to `FARTHEST_FLASH`.
const NEAR: f32 = 40.;
const FARTHEST_FLASH: f32 = 0.12;

/// How bright the even light of a flash at its brightest is on screen, on a white face open to
/// the sky: as the overcast at noon. The flash in the clouds and the bolt light more (the
/// shaders).
const FLASH_LIGHT: f32 = 0.5;

/// How much the eye opens to a flash at its brightest, in stops, with the physical sky.
const FLASH_EXPOSURE: f32 = 0.5;

/// The brightness to add to the ambient light for the even light of the flash `flash`, at the
/// exposure `ev100`: on screen, a light is its brightness times the exposure,
/// 1 / (1.2 * 2^ev100).
pub fn flash_ambient(flash: f32, ev100: f32) -> f32 {
    flash * FLASH_LIGHT * 1.2 * ev100.exp2()
}

/// How many stops brighter the exposure is during the flash `flash`.
pub fn flash_exposure(flash: f32) -> f32 {
    flash * FLASH_EXPOSURE
}

/// A strike being seen.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveStrike {
    pub at: IVec3,
    pub seed: u32,
    /// Seconds since it struck.
    pub age: f32,
    pub strokes: Strokes,
    /// The horizontal distance from the eye to the struck block, in blocks, when it struck.
    pub distance: f32,
    /// Its bolt, `None` beyond the render distance: only the clouds flash.
    pub bolt: Option<Bolt>,
}

impl LiveStrike {
    /// The strike `strike` seen from `eye`, with the chunks shown up to `reach` blocks away.
    pub fn new(strike: Strike, eye: Vec3, reach: f32) -> Self {
        let distance = strike.at.as_vec3().xz().distance(eye.xz());
        Self {
            at: strike.at,
            seed: strike.seed,
            age: 0.,
            strokes: Strokes::new(strike.seed),
            distance,
            bolt: (distance <= reach).then(|| Bolt::new(strike.at, strike.seed)),
        }
    }

    /// How bright its flash lights the world now, from 0 to 1.
    pub fn flash(&self) -> f32 {
        self.strokes.channel(self.age) * flash_strength(self.distance)
    }

    /// Where it flashes in the clouds: above the struck block, at the cloud base.
    pub fn cloud_point(&self) -> Vec3 {
        self.at.as_vec3().with_y(CLOUD_BASE) + Vec3::new(0.5, 0., 0.5)
    }

    fn over(&self) -> bool {
        self.age > self.strokes.life()
    }
}

/// How bright the flash of a strike `distance` blocks away is at its brightest, from 0 to 1.
pub fn flash_strength(distance: f32) -> f32 {
    (NEAR / distance.max(1.))
        .powf(0.75)
        .clamp(FARTHEST_FLASH, 1.)
}

/// The strikes being seen.
#[derive(Resource, Default, Debug)]
pub struct Lightning {
    pub strikes: Vec<LiveStrike>,
}

impl Lightning {
    /// How bright the flash of lightning is now, from 0: that of the brightest strike.
    pub fn flash(&self) -> f32 {
        self.strikes
            .iter()
            .map(LiveStrike::flash)
            .fold(0., f32::max)
    }

    /// The strike flashing the brightest now, if any flashes.
    pub fn brightest(&self) -> Option<&LiveStrike> {
        self.strikes
            .iter()
            .filter(|strike| strike.flash() > 0.)
            .max_by(|a, b| a.flash().total_cmp(&b.flash()))
    }

    /// Moves the strikes `secs` seconds on, and forgets those that are over.
    pub fn advance(&mut self, secs: f32) {
        for strike in &mut self.strikes {
            strike.age += secs;
        }
        self.strikes.retain(|strike| !strike.over());
    }
}

/// The lightning of the storms, while in game.
pub fn lightning_plugin(app: &mut App) {
    app.init_resource::<Lightning>()
        .add_message::<Strike>()
        .add_systems(OnExit(GameState::Game), forget_strikes)
        .add_systems(
            Update,
            (advance_strikes_system, receive_strikes_system)
                .chain()
                .run_if(in_state(GameState::Game)),
        );
}

/// How far the chunks are shown, in blocks: a bolt beyond it is not drawn.
pub fn shown_reach(
    render_distance: &RenderDistance,
    server_limit: Option<&ServerRenderDistanceLimit>,
) -> f32 {
    let chunks = render_distance
        .distance
        .min(server_limit.map_or(u32::MAX, |limit| limit.0));
    (chunks * shared::CHUNK_SIZE as u32) as f32
}

fn receive_strikes_system(
    mut strikes: MessageReader<Strike>,
    mut lightning: ResMut<Lightning>,
    cameras: Query<&GlobalTransform, With<Camera3d>>,
    render_distance: Res<RenderDistance>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
) {
    let eye = cameras
        .iter()
        .next()
        .map_or(Vec3::ZERO, GlobalTransform::translation);
    let reach = shown_reach(&render_distance, server_limit.as_deref());
    for &strike in strikes.read() {
        lightning.strikes.push(LiveStrike::new(strike, eye, reach));
    }
}

fn advance_strikes_system(time: Res<Time>, mut lightning: ResMut<Lightning>) {
    if !lightning.strikes.is_empty() {
        lightning.advance(time.delta_secs());
    }
}

fn forget_strikes(mut lightning: ResMut<Lightning>) {
    lightning.strikes.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strike(x: i32) -> Strike {
        Strike {
            at: IVec3::new(x, 70, 0),
            seed: 5,
        }
    }

    #[test]
    fn a_strike_within_the_render_distance_draws_its_bolt_and_one_beyond_only_flashes() {
        let near = LiveStrike::new(strike(50), Vec3::new(0., 75., 0.), 128.);
        assert_eq!(near.distance, 50.);
        assert_eq!(near.bolt, Some(Bolt::new(near.at, 5)));
        let far = LiveStrike::new(strike(150), Vec3::new(0., 75., 0.), 128.);
        assert_eq!(far.bolt, None);
        assert_eq!(far.cloud_point(), Vec3::new(150.5, CLOUD_BASE, 0.5));
    }

    #[test]
    fn a_flash_lights_the_world_as_brightly_on_screen_at_any_exposure() {
        assert_eq!(flash_ambient(0., 13.), 0.);
        assert_eq!(flash_exposure(0.), 0.);
        for ev100 in [9.4, 13.] {
            let on_screen = flash_ambient(1., ev100) / (1.2 * ev100.exp2());
            assert!((on_screen - FLASH_LIGHT).abs() < 1e-5);
        }
        assert!(flash_ambient(0.5, 13.) < flash_ambient(1., 13.));
        assert!(flash_exposure(1.) > 0.);
    }

    #[test]
    fn the_flash_is_brightest_close_by_and_fades_with_the_distance() {
        assert_eq!(flash_strength(16.), 1.);
        assert_eq!(flash_strength(40.), 1.);
        let mut before = 1.;
        for distance in [60., 100., 160., 300.] {
            let strength = flash_strength(distance);
            assert!(
                strength < before && strength >= FARTHEST_FLASH,
                "{distance}"
            );
            before = strength;
        }
        assert!((flash_strength(160.) - 0.35).abs() < 0.02);
        assert_eq!(flash_strength(5000.), FARTHEST_FLASH);
    }

    #[test]
    fn the_flash_follows_the_strokes_of_the_brightest_strike_and_ends_with_them() {
        let mut lightning = Lightning::default();
        assert_eq!(lightning.flash(), 0.);
        assert!(lightning.brightest().is_none());
        lightning
            .strikes
            .push(LiveStrike::new(strike(30), Vec3::ZERO, 128.));
        let peak = lightning.strikes[0].strokes.peaks().next().unwrap();
        lightning.advance(peak);
        assert_eq!(lightning.flash(), 1.);
        lightning
            .strikes
            .push(LiveStrike::new(strike(150), Vec3::ZERO, 128.));
        assert_eq!(lightning.brightest().unwrap().distance, 30.);
        lightning.advance(1.);
        assert!(lightning.strikes.is_empty());
        assert_eq!(lightning.flash(), 0.);
    }
}
