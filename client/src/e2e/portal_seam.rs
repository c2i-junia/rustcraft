//! The seam of a crossing, and the player's own view by a portal (`.superpowers/sdd/portals/
//! seam-diagnosis.md`): the view through a portal from an eye just in front of it, against the
//! view straight from the same eye carried through the portal (`Portals::through`), as the
//! player sees them on the last frame before it walks through and on the first after. Each test
//! saves its pictures, their differences and its measures into
//! `.superpowers/sdd/portals/seam-screens/`, and is ignored: each takes minutes.
//!
//! Two kinds. The gates check: a close twin, one 6 and one 21 chunks away are crossed without a
//! seam (`*_is_seamless`), the player's own view does not change with a portal (`own_view_*`),
//! the sky the chunks fade into is the sky at the horizon (`horizon_*`) and follows the hour
//! (`the_sky_follows_*`). The diagnoses (`seam_*` but `*_is_seamless`) measure, with some
//! suspects of the seam turned off one by one, and check little. Run them all, two at most at
//! once (with three, a server timed out), in about an hour, with
//! `E2E_LIFETIME_SECS=3000 cargo test -p client e2e::portal_seam -- --ignored --test-threads=2`,
//! or, before a merge, a gate of each kind, in about 8 minutes, with
//! `E2E_LIFETIME_SECS=3000 cargo test -p client -- --ignored --test-threads=2 --exact
//! e2e::portal_seam::seam_close_off_noon_is_seamless
//! e2e::portal_seam::seam_close_high_dusk_is_seamless
//! e2e::portal_seam::seam_far_high_noon_is_seamless
//! e2e::portal_seam::own_view_high_dusk_is_unchanged_by_an_approached_portal
//! e2e::portal_seam::own_view_high_dusk_is_unchanged_by_a_stacked_floor_and_ceiling_pair
//! e2e::portal_seam::own_view_dusk_is_unchanged_by_the_shadows_of_a_close_twin
//! e2e::portal_seam::horizon_high_dusk_portals_first`.
//!
//! The player's camera is pinned to the eye measured (`Pinned`), so that both pictures are taken
//! from exactly the same place, and some suspects of the seam can be turned off, one by one, to
//! see how much of it each makes.

use std::io::Write as _;
use std::path::PathBuf;

use bevy::anti_alias::taa::TemporalAntiAliasing;
use bevy::light::ShadowFilteringMethod;
use bevy::light::{NotShadowCaster, VolumetricLight};
use bevy::math::Affine3A;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::render::camera::TemporalJitter;
use bevy::transform::TransformSystems;
use image::{GrayImage, RgbImage};
use shared::world::{
    BlockData, BlockDirection, BlockId, ChunkMap, FaceDirection, FaceDirectionExt, ItemId, Portal,
    PortalColor, WeatherKind, PORTAL_HEIGHT, PORTAL_WIDTH,
};
use shared::{DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::{enter_world_with, hold_click, slot};
use super::harness::{Harness, Options, SERVER};
use super::locator::Locator as L;
use super::portals::{local_player, own_portal, FAR_MOD};
use super::seed::GameFolder;
use crate::camera::{CameraController, FOV_DEGREES};
use crate::portal::approach::{track_approach, Approach};
use crate::portal::PortalCamera;
use crate::state::GameSet;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::celestial::{sky_rotation, sun_height};
use crate::world::lightning::bolt::Strokes;
use crate::world::lightning::{advance_strikes_system, Lightning, LiveStrike, Strike};
use crate::world::rendering::render::CastsForTwinsOnly;
use crate::world::rendering::shaders::ShaderFeatures;
use crate::world::time::ClientTime;
use crate::world::weather::{ease_weather_system, ClientWeather};
use crate::world::{group_of, ClientWorldMap, GraphicsSettings, ShaderQuality};
use std::time::Duration;

const WIDTH: u32 = 960;
const HEIGHT: u32 = 540;
/// The player's render distance: its fog ends 128 blocks away, and so does that of a portal
/// camera through a close twin; through a far one, it ends where the chunks sent around the
/// twin end (`held_reach`).
const RENDER_DISTANCE: u32 = 8;
/// How far in front of the portal the eye is, on either side of the crossing.
const CROSSING: f32 = 0.05;
/// Frames for the exposure, TAA's history and the effects to settle after the camera moved.
const SETTLE: usize = 20;

/// What the test does to the player's camera and the portal cameras.
#[derive(Resource, Default)]
struct Pinned {
    /// Where the player's camera is, whatever the player does.
    eye: Option<Transform>,
    /// Whether the portal cameras get the fog of the player's camera, not their own shorter one.
    portal_fog_as_main: bool,
    /// Whether the portal cameras draw no fog at all, to see what lies in theirs.
    portal_fog_off: bool,
    /// The tick the clock stays at: it moves with the wall clock, so that otherwise the sun,
    /// and the shadows, moved by as much as lavapipe was slow between two pictures.
    tick: Option<(Hour, u64)>,
    /// The weather held, whatever the levels would ease to.
    weather: Option<ClientWeather>,
    /// The strikes of lightning held at their age, whatever the clock.
    strikes: Option<Vec<LiveStrike>>,
    /// The twins the player approaches, whatever the portals (`Approach::twins`).
    twins: Option<Vec<IVec3>>,
}

fn pin_approach(pinned: Res<Pinned>, mut approach: ResMut<Approach>) {
    if let Some(twins) = &pinned.twins {
        if approach.twins != *twins {
            approach.twins.clone_from(twins);
        }
    }
}

fn pin_strikes(pinned: Res<Pinned>, mut lightning: ResMut<Lightning>) {
    if let Some(strikes) = &pinned.strikes {
        lightning.strikes = strikes.clone();
    }
}

fn pin_weather(pinned: Res<Pinned>, mut weather: ResMut<ClientWeather>) {
    if let Some(held) = &pinned.weather {
        *weather = held.clone();
    }
}

fn pin_clock(pinned: Res<Pinned>, mut time: ResMut<ClientTime>) {
    if let Some((_, tick)) = pinned.tick {
        time.0 = tick;
    }
}

fn pin_eye(pinned: Res<Pinned>, mut cameras: Query<&mut Transform, With<CameraController>>) {
    if let Some(eye) = pinned.eye {
        for mut camera in &mut cameras {
            *camera = eye;
        }
    }
}

/// After `place_portal_cameras`, which runs before the propagation of the transforms.
fn portal_fog_as_main(
    pinned: Res<Pinned>,
    main: Query<&DistanceFog, (With<CameraController>, Without<PortalCamera>)>,
    mut portals: Query<&mut DistanceFog, (With<PortalCamera>, Without<CameraController>)>,
) {
    let Ok(main) = main.single() else {
        return;
    };
    for mut fog in &mut portals {
        if pinned.portal_fog_off {
            fog.falloff = FogFalloff::Linear {
                start: 1e4,
                end: 2e4,
            };
        } else if pinned.portal_fog_as_main {
            *fog = main.clone();
        }
    }
}

fn install(h: &mut Harness) {
    h.app.init_resource::<Pinned>();
    h.app.add_systems(Update, pin_eye.after(GameSet::Camera));
    h.app.add_systems(
        FixedPostUpdate,
        pin_clock.after(crate::world::time::time_update_system),
    );
    h.app.add_systems(
        PostUpdate,
        portal_fog_as_main.after(TransformSystems::Propagate),
    );
    h.app
        .add_systems(Update, pin_weather.after(ease_weather_system));
    h.app
        .add_systems(PreUpdate, pin_strikes.after(advance_strikes_system));
    h.app
        .add_systems(PreUpdate, pin_approach.after(track_approach));
}

fn pinned(h: &mut Harness) -> Mut<'_, Pinned> {
    h.world_mut().resource_mut::<Pinned>()
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Hour {
    Noon,
    Dusk,
    Night,
}

/// The tick of `hour` on the day after `now`.
fn tick_of(hour: Hour, now: u64) -> u64 {
    let noon = now - now % DAY_DURATION_IN_TICKS + NOON_IN_DAY + DAY_DURATION_IN_TICKS;
    let after_noon = |below: f32| {
        (noon..noon + DAY_DURATION_IN_TICKS)
            .step_by(5)
            .find(|tick| sun_height(*tick) < below)
            .unwrap()
    };
    match hour {
        Hour::Noon => noon,
        // The sun low, the haze at its thickest, the light golden
        Hour::Dusk => after_noon(0.06),
        Hour::Night => after_noon(-0.7),
    }
}

/// Sets the clock at `hour` and keeps it there, at the same tick every time for the same hour:
/// the moon, which changes from a night to the next, too.
fn set_hour(h: &mut Harness, hour: Hour) {
    let tick = match pinned(h).tick {
        Some((pinned_hour, tick)) if pinned_hour == hour => tick,
        _ => tick_of(hour, h.world().resource::<ClientTime>().0),
    };
    h.world_mut().insert_resource(ClientTime(tick));
    pinned(h).tick = Some((hour, tick));
}

fn set_quality(h: &mut Harness, quality: ShaderQuality) {
    h.world_mut().resource_mut::<GraphicsSettings>().shaders = quality;
    h.frames(SETTLE);
}

/// The eye `d` blocks in front of `portal` (behind it when negative), a tenth of a block above
/// the middle of the opening, as a player's eyes are, looking into it, `pitch` up.
fn eye_in_front(portal: &Portal, d: f32, pitch: f32) -> Transform {
    let eye = Vec3::from(portal.frame().translation) + portal.normal() * d + Vec3::Y * 0.1;
    let look = -portal.normal() * pitch.cos() + Vec3::Y * pitch.sin();
    Transform::from_translation(eye).looking_to(look, Vec3::Y)
}

/// Where the player's camera is at `d` blocks in front of `portal` on the way into it: in front of
/// it, or carried through it once past its plane.
fn walking_eye(portal: &Portal, through: Affine3A, d: f32, pitch: f32) -> Transform {
    let eye = eye_in_front(portal, d, pitch);
    if d > 0.0 {
        eye
    } else {
        carried(through, &eye)
    }
}

fn carried(through: Affine3A, eye: &Transform) -> Transform {
    Transform::from_matrix(Mat4::from(through * eye.compute_affine()))
}

/// The screen with the player's camera at `eye`, `settle` frames after it was put there.
fn shot_at(h: &mut Harness, eye: Transform, settle: usize) -> RgbImage {
    pinned(h).eye = Some(eye);
    h.frames(settle);
    h.screenshot()
}

/// The direction of the ray of the pixel `(x, y)` from a camera at `eye`.
fn ray(eye: &Transform, x: u32, y: u32) -> Vec3 {
    let tan = (FOV_DEGREES.to_radians() / 2.0).tan();
    let aspect = WIDTH as f32 / HEIGHT as f32;
    let ndc = Vec2::new(
        (x as f32 + 0.5) / WIDTH as f32 * 2.0 - 1.0,
        1.0 - (y as f32 + 0.5) / HEIGHT as f32 * 2.0,
    );
    (eye.rotation * Vec3::new(ndc.x * tan * aspect, ndc.y * tan, -1.0)).normalize()
}

/// How far from `from` along `direction` the first block is, within `max` blocks: a voxel walk.
fn hit_distance(map: &ChunkMap, from: Vec3, direction: Vec3, max: f32) -> Option<f32> {
    let mut cell = from.floor().as_ivec3();
    let step = direction.signum().as_ivec3();
    let delta = direction.recip().abs();
    let next = |axis: usize| {
        let edge = if direction[axis] > 0.0 {
            cell[axis] as f32 + 1.0
        } else {
            cell[axis] as f32
        };
        ((edge - from[axis]) / direction[axis]).abs()
    };
    let mut to_edge = Vec3::new(next(0), next(1), next(2));
    let mut travelled = 0.0;
    while travelled < max {
        if map.get_block(cell).is_some() {
            return Some(travelled);
        }
        let axis = if to_edge.x < to_edge.y && to_edge.x < to_edge.z {
            0
        } else if to_edge.y < to_edge.z {
            1
        } else {
            2
        };
        travelled = to_edge[axis];
        to_edge[axis] += delta[axis];
        cell[axis] += step[axis];
    }
    None
}

/// What the pixels measured show, by distance from the eye.
const CLASSES: [&str; 5] = ["0-32", "32-64", "64-96", "96+", "sky"];

/// The pixels measured, one in four, with what each shows from the eye `seen` (straight, after
/// the crossing): the class of its distance. When the eye `before` and its `portal` are given,
/// only the pixels of the opening of that portal from that eye. It runs a frame of the game
/// for each band of 32 rows, and reads the map of the chunks anew each band: take the pictures
/// before, or pin the eye, the clock and the strikes, so that every band sees the same world.
fn sample_pixels(
    h: &mut Harness,
    seen: &Transform,
    before: Option<(&Transform, &Portal)>,
) -> Vec<(u32, u32, usize)> {
    let mut pixels = Vec::new();
    // A band of rows a frame: a view far over the plains walks rays of up to 400 blocks for
    // long, longer than the connection to the server lasts without a frame
    for band in (0..HEIGHT).step_by(32) {
        let map = &h.world().resource::<ClientWorldMap>().blocks;
        for y in (band..(band + 32).min(HEIGHT)).step_by(2) {
            for x in (0..WIDTH).step_by(2) {
                if on_the_hud(x, y) {
                    continue;
                }
                if let Some((eye, portal)) = before {
                    let direction = ray(eye, x, y);
                    let along =
                        -portal.signed_distance(eye.translation) / direction.dot(portal.normal());
                    let local = portal.local(eye.translation + direction * along);
                    let half = Vec2::new(PORTAL_WIDTH as f32, PORTAL_HEIGHT as f32) / 2.0;
                    if along <= 0.0 || local.x.abs() > half.x || local.y.abs() > half.y {
                        continue;
                    }
                }
                let class = match hit_distance(map, seen.translation, ray(seen, x, y), 400.0) {
                    Some(d) if d < 32.0 => 0,
                    Some(d) if d < 64.0 => 1,
                    Some(d) if d < 96.0 => 2,
                    Some(_) => 3,
                    None => 4,
                };
                pixels.push((x, y, class));
            }
        }
        h.frames(1);
    }
    pixels
}

/// Whether the pixel `(x, y)` is under the minimap, the hotbar or the crosshair. The minimap's
/// panel widens with its coordinates: at z = 346 it starts at x = 755.
fn on_the_hud(x: u32, y: u32) -> bool {
    (x >= 720 && y <= 172)
        || ((234..=726).contains(&x) && (450..=512).contains(&y))
        || ((466..=494).contains(&x) && (256..=284).contains(&y))
}

fn luma(pixel: &image::Rgb<u8>) -> f32 {
    let [r, g, b] = pixel.0.map(f32::from);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The measures of a class of pixels: its name, how many pixels, the mean of the mean
/// difference of their channels, its 95th percentile and its maximum, from 0 to 255, how much
/// brighter the picture before is on average (luma), and the median difference.
type Row = (String, usize, f32, f32, f32, f32, f32);

/// The difference of `a` and `b` over `pixels`, by class and over all (`Row`).
fn measure(a: &RgbImage, b: &RgbImage, pixels: &[(u32, u32, usize)]) -> Vec<Row> {
    let mut by_class: Vec<Vec<(f32, f32)>> = vec![Vec::new(); CLASSES.len() + 1];
    for &(x, y, class) in pixels {
        let (pa, pb) = (a.get_pixel(x, y), b.get_pixel(x, y));
        let diff = (0..3)
            .map(|c| f32::from(pa.0[c].abs_diff(pb.0[c])))
            .sum::<f32>()
            / 3.0;
        let lighter = luma(pa) - luma(pb);
        by_class[class].push((diff, lighter));
        by_class[CLASSES.len()].push((diff, lighter));
    }
    by_class
        .into_iter()
        .enumerate()
        .map(|(class, mut values)| {
            let name = CLASSES.get(class).copied().unwrap_or("all").to_string();
            if values.is_empty() {
                return (name, 0, 0.0, 0.0, 0.0, 0.0, 0.0);
            }
            let n = values.len();
            let mean = values.iter().map(|v| v.0).sum::<f32>() / n as f32;
            let lighter = values.iter().map(|v| v.1).sum::<f32>() / n as f32;
            values.sort_by(|a, b| a.0.total_cmp(&b.0));
            (
                name,
                n,
                mean,
                values[n * 95 / 100].0,
                values[n - 1].0,
                lighter,
                values[n / 2].0,
            )
        })
        .collect()
}

/// The difference of `a` and `b`, four times brighter.
fn difference_image(a: &RgbImage, b: &RgbImage) -> GrayImage {
    GrayImage::from_fn(a.width(), a.height(), |x, y| {
        let (pa, pb) = (a.get_pixel(x, y), b.get_pixel(x, y));
        let diff = (0..3)
            .map(|c| u32::from(pa.0[c].abs_diff(pb.0[c])))
            .sum::<u32>()
            / 3;
        image::Luma([(diff * 4).min(255) as u8])
    })
}

fn out_dir() -> PathBuf {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.superpowers/sdd/portals/seam-screens");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A map of the classes of `pixels`, from dark (near) to light (far), the sky white.
fn class_image(pixels: &[(u32, u32, usize)]) -> GrayImage {
    let mut image = GrayImage::new(WIDTH, HEIGHT);
    for &(x, y, class) in pixels {
        let level = [40u8, 90, 140, 190, 255][class];
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            image.put_pixel(x + dx, y + dy, image::Luma([level]));
        }
    }
    image
}

/// Writes the measures of `a` against `b` into `results.tsv`, prints them and returns them.
fn record(
    case: &str,
    variant: &str,
    a: &RgbImage,
    b: &RgbImage,
    pixels: &[(u32, u32, usize)],
) -> Vec<Row> {
    let rows = measure(a, b, pixels);
    write_rows(case, variant, &rows);
    rows
}

/// Appends `rows` to `results.tsv`, and prints them.
fn write_rows(case: &str, variant: &str, rows: &[Row]) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir().join("results.tsv"))
        .unwrap();
    let mut text = String::new();
    for (class, n, mean, p95, max, lighter, median) in rows {
        text += &format!(
            "{case}\t{variant}\t{class}\t{n}\t{mean:.2}\t{p95:.1}\t{max:.1}\t{lighter:+.2}\t{median:.2}\n"
        );
    }
    file.write_all(text.as_bytes()).unwrap();
    print!("{text}");
}

/// How far over the noise of the reference window (`noise_floor`) the pictures of two windows
/// may differ and still show the same view: two windows taken apart also differ by what changes
/// from one to the other run to run, which no pair within a window shows.
const NOISE_MARGIN: f32 = 1.5;
/// The most two windows may differ in a class and still show the same view, whatever the noise:
/// a class as noisy as the mid-distance terrain (64-96) would otherwise let a change of a level
/// or two through.
const MOST_UNCHANGED: f32 = 1.5;
/// How many times noisier than the reference window the window under test may be: more, and the
/// view under test flickers, which its mean picture would hide.
const FLICKER: f32 = 2.0;
/// The noise under which a window under test never counts as flickering, whatever the reference.
const FLICKER_FLOOR: f32 = 0.5;
/// Whether `unchanged_view` checks that the window under test does not flicker (`FLICKER`): not
/// after the shaders were switched off and on, when the first window still varies more from
/// shot to shot one run in two (1.4 against 0.55 in the mid-distance class), though it no longer
/// drifts.
#[derive(Clone, Copy, PartialEq)]
enum Flicker {
    Checked,
    Unchecked,
}
/// The change every "unchanged view" gate must see in a run: its mid-distance terrain lighter by
/// so many levels (`unchanged_view`).
const RESOLUTION: u8 = 2;

/// Whether the window `tested` shows the view of the window `reference`, held still: the classes
/// in which they differ, with the reasons, empty if none. The noise is the reference window's
/// only (`noise_floor`): should the view under test flicker, its own noise would hide it. A class
/// changed if the mean pictures of the windows differ by more than `NOISE_MARGIN` times that
/// noise, at least `floor`, at most `MOST_UNCHANGED`; or, if `flicker` is checked, if the window
/// under test is `FLICKER` times noisier than the reference. And the gate must see in this run a
/// change of `RESOLUTION` levels of its mid-distance terrain: it panics if it would not.
fn unchanged_view(
    case: &str,
    variant: &str,
    (reference, tested): (&[RgbImage], &[RgbImage]),
    pixels: &[(u32, u32, usize)],
    floor: f32,
    flicker: Flicker,
) -> Vec<String> {
    let noise = noise_floor(case, &format!("{variant}_noise"), &[reference], pixels);
    let tested_noise = noise_floor(case, &format!("{variant}_tested_noise"), &[tested], pixels);
    let (a, b) = (mean_picture(reference), mean_picture(tested));
    save(case, variant, &a, &b, true);
    let rows = record(case, variant, &a, &b, pixels);
    let threshold = |noise: f32| (noise * NOISE_MARGIN).max(floor).min(MOST_UNCHANGED);
    let changed = |rows: &[Row]| -> Vec<String> {
        rows.iter()
            .zip(&noise)
            .zip(&tested_noise)
            .filter_map(|(((class, count, mean, ..), noise), tested)| {
                let (noise, tested) = (noise.2, tested.2);
                if *count == 0 {
                    None
                } else if *mean > threshold(noise) {
                    Some(format!("{class} by {mean:.2} (noise {noise:.2})"))
                } else if flicker == Flicker::Checked
                    && tested > (noise * FLICKER).max(FLICKER_FLOOR)
                {
                    Some(format!(
                        "{class} flickers by {tested:.2} (noise {noise:.2})"
                    ))
                } else {
                    None
                }
            })
            .collect()
    };
    // The same view, its mid-distance terrain a little lighter: the gate sees it
    let mid = CLASSES.iter().position(|class| *class == "64-96").unwrap();
    if pixels.iter().any(|&(.., class)| class == mid) {
        let sees = |levels: u8| {
            let mut lighter = b.clone();
            for &(x, y, class) in pixels {
                if class == mid {
                    let pixel = lighter.get_pixel_mut(x, y);
                    pixel.0 = pixel.0.map(|c| c.saturating_add(levels));
                }
            }
            let seen = changed(&measure(&a, &lighter, pixels));
            seen.iter().any(|reason| reason.starts_with("64-96 by"))
        };
        println!(
            "{case} {variant}: the mid-distance terrain a level lighter seen: {}",
            sees(1)
        );
        assert!(
            sees(RESOLUTION),
            "{case} {variant}: too noisy to see the mid-distance terrain {RESOLUTION} levels \
             lighter"
        );
    }
    changed(&rows)
}

/// The noise of a view held still, by class (`Row`): of every pair of shots of each of
/// `windows`, the one whose pictures differ most. Recorded as `variant`.
fn noise_floor(
    case: &str,
    variant: &str,
    windows: &[&[RgbImage]],
    pixels: &[(u32, u32, usize)],
) -> Vec<Row> {
    let pairs: Vec<Vec<Row>> = windows
        .iter()
        .flat_map(|shots| {
            (0..shots.len()).flat_map(move |i| (i + 1..shots.len()).map(move |j| (i, j, shots)))
        })
        .map(|(i, j, shots)| measure(&shots[i], &shots[j], pixels))
        .collect();
    let rows: Vec<Row> = (0..pairs[0].len())
        .map(|class| {
            pairs
                .iter()
                .map(|rows| rows[class].clone())
                .max_by(|a, b| a.2.total_cmp(&b.2))
                .unwrap()
        })
        .collect();
    write_rows(case, variant, &rows);
    rows
}

fn save(case: &str, variant: &str, a: &RgbImage, b: &RgbImage, pictures: bool) {
    let dir = out_dir();
    if pictures {
        a.save(dir.join(format!("{case}_{variant}_a.png"))).unwrap();
        b.save(dir.join(format!("{case}_{variant}_b.png"))).unwrap();
    }
    difference_image(a, b)
        .save(dir.join(format!("{case}_{variant}_diff.png")))
        .unwrap();
}

/// How many screenshots, how many frames apart, make a picture of the regression tests.
const STEADY_SHOTS: usize = 3;
const STEADY_GAP: usize = 5;

/// The mean of `pictures`, pixel by pixel.
fn mean_picture(pictures: &[RgbImage]) -> RgbImage {
    let (width, height) = pictures[0].dimensions();
    RgbImage::from_fn(width, height, |x, y| {
        image::Rgb(std::array::from_fn(|c| {
            let sum: u32 = pictures
                .iter()
                .map(|p| u32::from(p.get_pixel(x, y).0[c]))
                .sum();
            ((sum as f32 / pictures.len() as f32).round()) as u8
        }))
    })
}

/// `STEADY_SHOTS` screenshots `STEADY_GAP` frames apart with the player's camera at `eye`, at
/// `hour`, once the frame settled.
fn steady_shots(h: &mut Harness, eye: Transform, hour: Hour) -> Vec<RgbImage> {
    set_hour(h, hour);
    pinned(h).eye = Some(eye);
    h.frames(SETTLE);
    (0..STEADY_SHOTS)
        .map(|shot| {
            if shot > 0 {
                h.frames(STEADY_GAP);
            }
            h.screenshot()
        })
        .collect()
}

/// How much lighter or darker on average a picture may still get between two shots
/// `STEADY_GAP` frames apart for the view to count as settled, in levels of luma (0 to 255).
const SETTLED_DRIFT: f32 = 0.1;
/// The most shots taken while waiting for a view to settle.
const MAX_SETTLE_SHOTS: usize = 40;

fn mean_luma(picture: &RgbImage) -> f32 {
    picture.pixels().map(luma).sum::<f32>() / (picture.width() * picture.height()) as f32
}

/// `steady_shots`, once the view stopped drifting. After the shaders are switched, or the world
/// changed around the eye, some effects converge for longer than `SETTLE` frames, the whole
/// picture getting lighter or darker from shot to shot: compared with a picture of the same view
/// taken later, settled, that drift would read as a change of the view. Shots are taken
/// `STEADY_GAP` frames apart until each of the last `STEADY_SHOTS` is within `SETTLED_DRIFT` of
/// the one before in mean luma.
fn settled_shots(h: &mut Harness, eye: Transform, hour: Hour) -> Vec<RgbImage> {
    let mut shots = steady_shots(h, eye, hour);
    let drift = |shots: &[RgbImage]| {
        shots
            .windows(2)
            .map(|pair| (mean_luma(&pair[0]) - mean_luma(&pair[1])).abs())
            .fold(0.0, f32::max)
    };
    for taken in 0..MAX_SETTLE_SHOTS {
        if drift(&shots) < SETTLED_DRIFT {
            println!("settled after {taken} more shots");
            return shots;
        }
        shots.remove(0);
        h.frames(STEADY_GAP);
        shots.push(h.screenshot());
    }
    println!(
        "not settled after {MAX_SETTLE_SHOTS} more shots: drift {}",
        drift(&shots)
    );
    shots
}

/// The two eyes of a crossing and the pixels measured.
struct Crossing {
    case: String,
    /// In front of the first portal, looking through it.
    before: Transform,
    /// The same eye carried through, in front of the twin, looking straight.
    after: Transform,
    /// The hour of both pictures, set again before each: at dusk, the sky changes fast.
    hour: Hour,
    pixels: Vec<(u32, u32, usize)>,
}

impl Crossing {
    /// Measures the picture before against the picture after, as `variant`, and how much two
    /// pictures from the same eye differ (the noise) for the baseline, and returns the measures.
    fn measure(&self, h: &mut Harness, variant: &str, pictures: bool) -> Vec<Row> {
        set_hour(h, self.hour);
        let a = shot_at(h, self.before, SETTLE);
        if variant == "baseline" {
            describe_cameras(h);
        }
        set_hour(h, self.hour);
        let b = shot_at(h, self.after, SETTLE);
        if variant == "baseline" {
            h.frames(5);
            let b2 = h.screenshot();
            record(&self.case, "noise", &b, &b2, &self.pixels);
        }
        save(&self.case, variant, &a, &b, pictures);
        record(&self.case, variant, &a, &b, &self.pixels)
    }

    /// Measures the seam as `measure` does, each picture the mean of `STEADY_SHOTS` screenshots
    /// `STEADY_GAP` frames apart, which smooths the noise of the frame (TAA, the rays of light),
    /// up to 3.8 in the 64-96 class between two frames. Also records the noise of two single
    /// screenshots.
    fn measure_steady(&self, h: &mut Harness) -> Vec<Row> {
        let a = steady_shots(h, self.before, self.hour);
        let b = steady_shots(h, self.after, self.hour);
        record(&self.case, "noise", &b[0], &b[1], &self.pixels);
        let (a, b) = (mean_picture(&a), mean_picture(&b));
        save(&self.case, "baseline", &a, &b, true);
        record(&self.case, "baseline", &a, &b, &self.pixels)
    }

    /// `measure_steady`, recorded as `variant`.
    fn measure_steady_as(&self, h: &mut Harness, variant: &str) -> Vec<Row> {
        let a = steady_shots(h, self.before, self.hour);
        let b = steady_shots(h, self.after, self.hour);
        record(
            &self.case,
            &format!("{variant}_noise"),
            &b[0],
            &b[1],
            &self.pixels,
        );
        let (a, b) = (mean_picture(&a), mean_picture(&b));
        save(&self.case, variant, &a, &b, true);
        record(&self.case, variant, &a, &b, &self.pixels)
    }

    /// Turns off each suspect alone, measuring the seam each time.
    fn toggles_alone(&self, h: &mut Harness, quality: ShaderQuality) {
        pinned(h).portal_fog_as_main = true;
        self.measure(h, "fog_as_main", true);
        pinned(h).portal_fog_as_main = false;
        if quality == ShaderQuality::Off {
            return;
        }
        // Each taken away, then given back as it was
        // From the light: Bevy 0.19 leaves the rays of a camera drawn once it loses its
        // `VolumetricFog`, as long as some light has a `VolumetricLight`
        let taken = take::<VolumetricLight>(h, false);
        self.measure(h, "no_godrays", true);
        give_back(h, taken);
        let taken = take::<ScreenSpaceAmbientOcclusion>(h, false);
        self.measure(h, "no_ssao", false);
        give_back(h, taken);
        let taa = take::<TemporalAntiAliasing>(h, true);
        let jitter = take::<TemporalJitter>(h, true);
        self.measure(h, "no_taa", false);
        give_back(h, taa);
        give_back(h, jitter);
        let taken = take::<Bloom>(h, true);
        self.measure(h, "no_bloom", false);
        give_back(h, taken);
    }

    /// Turns off the suspects one after the other, together, measuring the seam each time.
    fn toggles_together(&self, h: &mut Harness, _quality: ShaderQuality) {
        pinned(h).portal_fog_as_main = true;
        take::<VolumetricLight>(h, false);
        self.measure(h, "fog_godrays", true);
        remove_from_all::<ScreenSpaceAmbientOcclusion>(h);
        self.measure(h, "fog_godrays_ssao", false);
        remove_from_main::<TemporalAntiAliasing>(h);
        remove_from_main::<TemporalJitter>(h);
        self.measure(h, "fog_godrays_ssao_taa", false);
        remove_from_main::<Bloom>(h);
        self.measure(h, "fog_godrays_ssao_taa_bloom", true);
    }
}

/// Prints what the player's camera and the portal cameras that draw have that the seam may come
/// from: their exposure, their environment map and its intensity, their fog.
fn describe_cameras(h: &mut Harness) {
    let world = h.world_mut();
    let mut cameras = world.query::<(
        &Camera,
        Has<CameraController>,
        Option<&PortalCamera>,
        Option<&bevy::camera::Exposure>,
        Option<&EnvironmentMapLight>,
        Option<&DistanceFog>,
    )>();
    for (camera, main, portal, exposure, map, fog) in cameras.iter(world) {
        if !main && !camera.is_active {
            continue;
        }
        println!(
            "camera {}: exposure {:?}, environment map {:?}, fog {:?}",
            if main {
                "of the player".to_string()
            } else {
                format!("{:?}", portal.map(|p| (p.color, p.level)))
            },
            exposure.map(|e| e.ev100),
            map.map(|m| (m.specular_map.id(), m.intensity)),
            fog.map(|f| (f.color, f.falloff.clone())),
        );
    }
}

/// Shows or hides the horizon skirt (`shaders.rs`).
fn show_skirt(h: &mut Harness, shown: bool) {
    let world = h.world_mut();
    let skirts: Vec<Entity> = world
        .query_filtered::<Entity, With<MeshMaterial3d<crate::world::rendering::shaders::HorizonMaterial>>>()
        .iter(world)
        .collect();
    for skirt in skirts {
        world.entity_mut(skirt).insert(if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
}

/// Takes `T` away from the player's camera (`main`) or from every camera, and returns it.
fn take<T: Component + Clone>(h: &mut Harness, main: bool) -> Vec<(Entity, T)> {
    let world = h.world_mut();
    let taken: Vec<(Entity, T)> = world
        .query::<(Entity, &T, Has<CameraController>)>()
        .iter(world)
        .filter(|(_, _, is_main)| *is_main || !main)
        .map(|(entity, component, _)| (entity, component.clone()))
        .collect();
    for (entity, _) in &taken {
        world.entity_mut(*entity).remove::<T>();
    }
    taken
}

fn give_back<T: Component>(h: &mut Harness, taken: Vec<(Entity, T)>) {
    let world = h.world_mut();
    for (entity, component) in taken {
        world.entity_mut(entity).insert(component);
    }
}

fn remove_from_main<T: Component>(h: &mut Harness) {
    let world = h.world_mut();
    let main = world
        .query_filtered::<Entity, With<CameraController>>()
        .single(world)
        .unwrap();
    world.entity_mut(main).remove::<T>();
}

fn remove_from_all<T: Component>(h: &mut Harness) {
    let world = h.world_mut();
    let cameras: Vec<Entity> = world
        .query_filtered::<Entity, With<Camera3d>>()
        .iter(world)
        .collect();
    for camera in cameras {
        world.entity_mut(camera).remove::<T>();
    }
}

/// The eye walks into the portal, 0.15 block a frame as a sneaking player does, from a block in
/// front of it to half a block past it; the screen is taken on every step, and how much it
/// changes from a step to the next is recorded.
fn walk(h: &mut Harness, case: &str, portal: &Portal, through: Affine3A, pitch: f32) {
    let dir = out_dir();
    let mut previous: Option<RgbImage> = None;
    let mut changes = Vec::new();
    for step in 0..11 {
        let d = 0.825 - 0.15 * step as f32;
        let shot = shot_at(h, walking_eye(portal, through, d, pitch), 1);
        shot.save(dir.join(format!("{case}_walk_{step:02}.png")))
            .unwrap();
        if let Some(previous) = &previous {
            let change = shot
                .as_raw()
                .iter()
                .zip(previous.as_raw())
                .map(|(a, b)| f32::from(a.abs_diff(*b)))
                .sum::<f32>()
                / shot.as_raw().len() as f32;
            changes.push(format!("{d:+.3}:{change:.2}"));
        }
        previous = Some(shot);
    }
    let line = format!("{case}\twalk\t{}\n", changes.join(" "));
    print!("{line}");
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir().join("walks.tsv"))
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
}

fn quality_name(quality: ShaderQuality) -> String {
    format!("{quality:?}").to_lowercase()
}

fn folder(world: &str, at: Vec3, clouds: bool, with_gun: bool) -> GameFolder {
    folder_at(world, at, clouds, with_gun, RENDER_DISTANCE)
}

/// `folder`, with the render distance `render_distance`.
fn folder_at(
    world: &str,
    at: Vec3,
    clouds: bool,
    with_gun: bool,
    render_distance: u32,
) -> GameFolder {
    let folder = GameFolder::new()
        .with_world(world)
        .with_player(world, "Tester", |player| {
            player.position = at;
            player.game_mode = shared::players::game_mode::GameMode::Creative;
            player.is_flying = true;
            if with_gun {
                player.inventory.inner.insert(
                    0,
                    shared::world::ItemStack {
                        item_id: ItemId::PortalGun,
                        item_type: ItemId::PortalGun.get_default_type(),
                        nb: 1,
                    },
                );
            }
        })
        .with_file(
            crate::constants::VIDEO_SETTINGS_PATH,
            &format!(
                "(render_distance: {render_distance}, clouds: {})",
                if clouds { "Fancy" } else { "Off" }
            ),
        );
    folder.with_mod("far", FAR_MOD)
}

fn options() -> Options {
    Options {
        width: WIDTH,
        height: HEIGHT,
        // The others are set once the world is loaded: lavapipe draws them slowly
        shaders: Some(ShaderQuality::Off),
        ..default()
    }
}

/// Waits for every chunk within the render distance of the chunk at `center` to have its
/// columns, and for them to be meshed.
fn wait_for_terrain(h: &mut Harness, center: Vec3) {
    let reach = RENDER_DISTANCE as i32 - 1;
    h.wait_until("the terrain", SERVER, |w| {
        let map = &w.resource::<ClientWorldMap>().blocks;
        (-reach..=reach).all(|cx| {
            (-reach..=reach).all(|cz| {
                map.get_height_ground(center + Vec3::new(cx as f32, 0.0, cz as f32) * 16.0) > 0
            })
        })
    });
    h.frames(90);
}

/// Builds a stone wall for `portal`, on a wall, down to the ground, and clears the two cells in
/// front of its opening, then puts the portal into the client's map: décor the server knows
/// nothing of.
fn build_portal(h: &mut Harness, portal: Portal) {
    let blocks = &mut h.world_mut().resource_mut::<ClientWorldMap>().blocks;
    let (normal, right) = (portal.face.to_ivec3(), portal.right().as_ivec3());
    for across in -3..=3 {
        let column = portal.anchor + right * across;
        let ground = blocks.get_height_ground(column.as_vec3());
        for y in ground.min(portal.anchor.y - 1)..=portal.anchor.y + 3 {
            blocks.set_block(
                column.with_y(y),
                BlockData::new(BlockId::Stone, BlockDirection::Front),
            );
        }
        for depth in 1..=2 {
            for y in portal.anchor.y..=portal.anchor.y + 3 {
                blocks.remove_block(column.with_y(y) + normal * depth);
            }
        }
    }
    blocks.portals_mut().insert(portal);
}

/// The ground under the cells of the column `x` from `z0` to `z1`, at its highest.
fn ground(h: &Harness, x: i32, z0: i32, z1: i32) -> i32 {
    let blocks = &h.world().resource::<ClientWorldMap>().blocks;
    (z0..=z1)
        .map(|z| blocks.get_height_ground(Vec3::new(x as f32 + 0.5, 0.0, z as f32 + 0.5)))
        .max()
        .unwrap()
}

/// Two portals on the ground of snowy plains, 24 blocks apart, in the client's map: the blue one
/// facing east, the orange one, north of it, facing west. Through the blue one, the player sees
/// west from the orange one, over the plains, to the end of its render distance. The player's
/// camera is pinned in front of the blue portal and through it, at `hour`, with the shaders
/// `quality`.
fn close_seam(quality: ShaderQuality, hour: Hour, test: Test, clouds: bool) {
    let player = Vec3::new(-60.5, 140.9, 4.5);
    let mut h = enter_world_with(folder("Seam", player, clouds, false), "Seam", options());
    install(&mut h);
    wait_for_terrain(&mut h, player.with_y(0.0));
    let blue_y = ground(&h, -20, 12, 19) + 1;
    let blue = Portal {
        owner: 1,
        color: PortalColor::Blue,
        anchor: IVec3::new(-20, blue_y, 16),
        face: FaceDirection::PlusX,
        up: FaceDirection::PlusY,
    };
    // Under the top of the layer of haze, on a pillar, for the eye before the blue one to be
    // carried above it
    let orange_y = if test == Test::FromAbove {
        crate::world::rendering::shaders::HAZE_TOP as i32 - 6
    } else {
        ground(&h, -19, -12, -5) + 1
    };
    let orange = Portal {
        owner: 1,
        color: PortalColor::Orange,
        anchor: IVec3::new(-19, orange_y, -9),
        face: FaceDirection::MinusX,
        up: FaceDirection::PlusY,
    };
    if test == Test::LatePortals {
        // The portals opened once the shaders are on, as in a game started with them
        set_quality(&mut h, quality);
    }
    build_portal(&mut h, blue);
    build_portal(&mut h, orange);
    let through = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .through(&blue)
        .unwrap();
    // A translation only
    assert!(through.transform_vector3(Vec3::X).distance(Vec3::X) < 1e-4);
    println!("blue {blue:?}, orange {orange:?}, through {through:?}");

    let pitch = -5f32.to_radians();
    let before = eye_in_front(&blue, CROSSING, pitch);
    let after = carried(through, &eye_in_front(&blue, -CROSSING, pitch));
    pinned(&mut h).eye = Some(after);
    h.frames(60);
    set_quality(&mut h, quality);
    let pixels = sample_pixels(&mut h, &after, Some((&before, &blue)));
    class_image(&pixels)
        .save(out_dir().join("close_classes.png"))
        .unwrap();
    let suffix = if clouds { "_clouds" } else { "" };
    set_hour(&mut h, hour);
    let mut crossing = Crossing {
        case: format!(
            "close_{}_{}{suffix}",
            quality_name(quality),
            format!("{hour:?}").to_lowercase()
        ),
        before,
        after,
        hour,
        pixels,
    };
    match test {
        Test::Baseline => {
            crossing.measure(&mut h, "baseline", true);
            if !clouds {
                set_hour(&mut h, hour);
                walk(&mut h, &crossing.case, &blue, through, pitch);
            }
        }
        Test::Alone => crossing.toggles_alone(&mut h, quality),
        Test::Together => crossing.toggles_together(&mut h, quality),
        Test::LatePortals => {
            crossing.case += "_late_portals";
            crossing.measure(&mut h, "baseline", true);
        }
        Test::Weather(kind) => {
            crossing.case += &format!("_{}", format!("{kind:?}").to_lowercase());
            pinned(&mut h).weather = Some(ClientWeather::settled(kind));
            h.frames(SETTLE);
            crossing.measure(&mut h, "baseline", true);
            // And the portal in the view, from a few steps back
            shot_at(&mut h, eye_in_front(&blue, 6.0, pitch), SETTLE)
                .save(out_dir().join(format!("{}_from_6.png", crossing.case)))
                .unwrap();
        }
        Test::RainVeil => {
            // The contrast of the drops against the sky seen through the opening, through the
            // portal and straight, the eye still, then strafing back and forth 0.03 blocks a
            // frame: TAA, the player's, blends the pictures of the frames along the motion of
            // what it sees, the portal's surface's, not that of the drops behind it
            crossing.case += "_rain_veil";
            pinned(&mut h).weather = Some(ClientWeather::settled(WeatherKind::Rain));
            h.frames(SETTLE);
            let sky: Vec<(u32, u32, usize)> = crossing
                .pixels
                .iter()
                .copied()
                .filter(|(.., class)| *class == 4)
                .collect();
            // The drops, thin streaks, by the luma's difference to the pixels two to the left
            // and two up, on the sky's smooth gradient
            let contrast = |shot: &RgbImage| {
                let near = |x: u32, y: u32| {
                    let l = luma(shot.get_pixel(x, y));
                    (l - luma(shot.get_pixel(x.saturating_sub(2), y))).abs()
                        + (l - luma(shot.get_pixel(x, y.saturating_sub(2)))).abs()
                };
                sky.iter().map(|&(x, y, _)| near(x, y)).sum::<f32>() / sky.len() as f32
            };
            let right = blue.right();
            let mut lines = Vec::new();
            for moving in [false, true] {
                for (view, eye) in [("portal", crossing.before), ("straight", crossing.after)] {
                    let mut contrasts = Vec::new();
                    for _ in 0..6 {
                        for frame in 0..20 {
                            let along = if moving {
                                (frame as f32 - 10.0) * 0.03
                            } else {
                                0.0
                            };
                            pinned(&mut h).eye =
                                Some(eye.with_translation(eye.translation + right * along));
                            h.frames(1);
                        }
                        contrasts.push(contrast(&h.screenshot()));
                    }
                    let mean = contrasts.iter().sum::<f32>() / contrasts.len() as f32;
                    let line = format!(
                        "{}\t{}\t{view}\t{} sky pixels\tcontrast {mean:.2} ({})",
                        crossing.case,
                        if moving { "strafing" } else { "still" },
                        sky.len(),
                        contrasts
                            .iter()
                            .map(|d| format!("{d:.2}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    );
                    println!("{line}");
                    lines.push(line);
                }
            }
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(out_dir().join("rain_veil.tsv"))
                .unwrap();
            for line in lines {
                writeln!(file, "{line}").unwrap();
            }
        }
        Test::Shadows => {
            crossing.case += "_shadows";
            // Each filter of the shadows on every camera, or as built: the portal cameras'
            // Gaussian, the player's temporal with TAA. A diagnosis, with no threshold: at Medium
            // and High noon, every filter, and no shadows at all, measured the same, within the
            // noise (lot 12 report)
            let filters = |h: &mut Harness, filter: Option<ShadowFilteringMethod>| {
                let world = h.world_mut();
                let cameras: Vec<(Entity, bool)> = world
                    .query_filtered::<(Entity, Has<PortalCamera>), With<Camera3d>>()
                    .iter(world)
                    .collect();
                for (camera, portal) in cameras {
                    let method = filter.unwrap_or(if portal {
                        ShadowFilteringMethod::Gaussian
                    } else {
                        ShadowFilteringMethod::Temporal
                    });
                    world.entity_mut(camera).insert(method);
                }
            };
            // The crossing, and from the portal camera's own eye, so that only how the two
            // cameras draw differs, not where they stand (the pixels classed as seen after)
            let same_eye = Crossing {
                case: crossing.case.clone(),
                after: carried(through, &before),
                pixels: crossing.pixels.clone(),
                ..crossing
            };
            // The first measure of a run differs from the next ones by more than the filters
            // do (lot 12 report): it is left out, and the filters are measured in turn
            crossing.measure_steady_as(&mut h, "warm_up");
            for round in 0..3 {
                for (name, filter) in [
                    ("as_built", None),
                    ("temporal", Some(ShadowFilteringMethod::Temporal)),
                    ("gaussian", Some(ShadowFilteringMethod::Gaussian)),
                    ("hardware", Some(ShadowFilteringMethod::Hardware2x2)),
                ] {
                    filters(&mut h, filter);
                    crossing.measure_steady_as(&mut h, &format!("{name}_{round}"));
                    same_eye.measure_steady_as(&mut h, &format!("same_eye_{name}_{round}"));
                }
            }
            filters(&mut h, None);
            let world = h.world_mut();
            let mut lights = world.query::<&mut DirectionalLight>();
            for mut light in lights.iter_mut(world) {
                light.shadow_maps_enabled = false;
            }
            crossing.measure_steady_as(&mut h, "no_shadows");
            same_eye.measure_steady_as(&mut h, "same_eye_no_shadows");
        }
        Test::Lightning => {
            // A strike 40 blocks out of the twin, at the brightest of its first stroke, under a
            // storm, the view raised to its bolt: through the portal, straight, and the portal
            // in the view from a few steps back
            crossing.case += "_lightning";
            pinned(&mut h).weather = Some(ClientWeather::settled(WeatherKind::Thunder));
            let (x, z) = (orange.anchor.x - 40, orange.anchor.z);
            let at = IVec3::new(x, ground(&h, x, z, z), z);
            let seed = 20_261_021;
            let eye = h
                .world_mut()
                .query_filtered::<&GlobalTransform, With<CameraController>>()
                .single(h.world())
                .unwrap()
                .translation();
            let mut strike = LiveStrike::seen(Strike { at, seed }, eye, &[], 128.0);
            strike.age = Strokes::new(seed).peaks().next().unwrap();
            assert!(strike.bolt.is_some(), "the bolt is drawn");
            pinned(&mut h).strikes = Some(vec![strike]);
            let up = 20f32.to_radians();
            crossing.before = eye_in_front(&blue, CROSSING, up);
            crossing.after = carried(through, &eye_in_front(&blue, -CROSSING, up));
            crossing.pixels =
                sample_pixels(&mut h, &crossing.after, Some((&crossing.before, &blue)));
            h.frames(SETTLE);
            crossing.measure(&mut h, "baseline", true);
            shot_at(&mut h, eye_in_front(&blue, 6.0, up), SETTLE)
                .save(out_dir().join(format!("{}_from_6.png", crossing.case)))
                .unwrap();
        }
        Test::FromAbove => {
            // The twin under the top of the layer of haze, the portal seen from 8 blocks above
            // its middle, at 45°: the portal camera stands above the layer, its twin in it. And
            // from 1 block above, the portal camera in the layer. The view straight from where
            // the portal camera stands is the picture after
            let mut measured = Vec::new();
            for (name, d, rise) in [("above", 8.0, 8.0), ("within", 4.0, 1.0)] {
                let centre = Vec3::from(blue.frame().translation);
                let eye = centre + blue.normal() * d + Vec3::Y * rise;
                let before = Transform::from_translation(eye).looking_at(centre, Vec3::Y);
                let after = carried(through, &before);
                assert_eq!(
                    after.translation.y > crate::world::rendering::shaders::HAZE_TOP,
                    name == "above"
                );
                let case = format!("{}_from_{name}", crossing.case);
                let a = steady_shots(&mut h, before, hour);
                // Straight from where the portal camera stands, the wall of the twin from its
                // opening up, which the portal camera does not draw, in front of its near plane,
                // taken away, and the portals with it
                let wall: Vec<IVec3> = (-3..=3)
                    .flat_map(|across| {
                        (0..=3).map(move |up| {
                            orange.anchor
                                + orange.right().as_ivec3() * across
                                + orange.up.to_ivec3() * up
                        })
                    })
                    .collect();
                let set_twin = |h: &mut Harness, open: bool| {
                    let blocks = &mut h.world_mut().resource_mut::<ClientWorldMap>().blocks;
                    for &cell in &wall {
                        if open {
                            blocks.remove_block(cell);
                        } else {
                            blocks.set_block(
                                cell,
                                BlockData::new(BlockId::Stone, BlockDirection::Front),
                            );
                        }
                    }
                    if open {
                        blocks.portals_mut().remove(orange.owner, orange.color);
                    } else {
                        blocks.portals_mut().insert(orange);
                    }
                };
                set_twin(&mut h, true);
                let pixels = sample_pixels(&mut h, &after, Some((&before, &blue)));
                let b = steady_shots(&mut h, after, hour);
                set_twin(&mut h, false);
                record(&case, "baseline_noise", &b[0], &b[1], &pixels);
                let (a, b) = (mean_picture(&a), mean_picture(&b));
                save(&case, "baseline", &a, &b, true);
                measured.push((case.clone(), record(&case, "baseline", &a, &b, &pixels)));
            }
            let screen = match hour {
                Hour::Noon => FROM_ABOVE_SEAMLESS_NOON,
                _ => FROM_ABOVE_SEAMLESS_DUSK,
            };
            for (case, rows) in measured {
                assert_seamless(&case, quality, &rows, screen);
            }
        }
        Test::Regression => {
            crossing.case += "_regression";
            let rows = crossing.measure_steady(&mut h);
            assert_seamless(&crossing.case, quality, &rows, SCREEN_SEAMLESS);
        }
    }
}

/// Checks that the measures `rows` of the crossing `case` at `quality` are within the gates:
/// `SEAMLESS` on the median of the 32–64 and 64–96 blocks classes, and with the haze of the
/// 96+ class, `SKY_SEAMLESS` and `SCREEN_SEAMLESS` on the mean of the sky and of all.
/// `screen`: how much the whole screen may differ on average, with the haze (`SCREEN_SEAMLESS`).
fn assert_seamless(case: &str, quality: ShaderQuality, rows: &[Row], screen: f32) {
    // With the haze, what it veils counts too: the far terrain, the sky, the screen
    let hazed = ShaderFeatures::of(quality).god_rays.is_some();
    let mut failed = Vec::new();
    for (class, _, mean, .., median) in rows {
        // The terrain by its median, which a shadow edge a pixel off does not move; the sky,
        // which has none, and the whole screen, by their mean
        let (what, value, threshold) = match class.as_str() {
            "32-64" | "64-96" => ("median", median, SEAMLESS),
            "96+" if hazed => ("median", median, SEAMLESS),
            "sky" if hazed => ("mean", mean, SKY_SEAMLESS),
            "all" if hazed => ("mean", mean, screen),
            _ => continue,
        };
        if *value >= threshold {
            failed.push(format!("the {class} class differs by {value:.2} ({what})"));
        }
    }
    assert!(
        failed.is_empty(),
        "{case}: {} across the portal",
        failed.join(", ")
    );
}

/// How much the median pixel of the 32–64 and 64–96 blocks classes, and of the 96+ class with
/// the haze, may differ across a crossing (out of 255, `measure`). The median, not the mean: at
/// Medium the shadow edges seen through the portal lie a pixel off those seen straight, which
/// puts the mean of the 64–96 class at 9–10.5 in most runs while its median stays at 4.33 at
/// most (lot 10 report: a difference of the shadows, which these gates do not test). What they
/// are for moves the median well past 6: with the portal camera's fog shortened to 64 blocks
/// (lot 9), 41–48 in the 64–96 class; without the haze through the portal at High (lot 10), 7
/// in the 64–96 class at noon and 10–20 past 96 blocks. Since, 4.33 at most.
const SEAMLESS: f32 = 6.0;

/// How much, on average, the sky and the whole screen may differ across a crossing at High and
/// Ultra, which draw the god-ray haze, and the 96+ blocks class there (`SEAMLESS`). Before the
/// portal cameras drew the haze, the sky differed by 6 at noon and 55 at dusk, the whole screen
/// by 5.2–5.8 and 23, the 96+ class by 16–17 and 10; after, with no SSAO on the portal cameras,
/// by 0.06–0.6, 0.89–1.43 and 0.87–2.3, against a noise of 0.03–2.7, 0.3–2.4 and 0.6–3.1
/// between two pictures from the same eye (lot 10 report): the screen has about 1.1 of margin.
const SKY_SEAMLESS: f32 = 3.0;
const SCREEN_SEAMLESS: f32 = 2.5;

/// How much, on average, the whole screen may differ between the view through a portal whose
/// twin is just under the top of the layer of haze, seen from above, and the view straight from
/// where the portal camera stands (`Test::FromAbove`), at High. With the portal camera out of
/// its layer, Bevy drew the layer from outside, its faces clipped by the oblique near plane on
/// the twin: no haze through the portal, 29.7 apart at dusk and 3.1 at noon. Since, 2.6 and
/// 0.8, against a noise of 0.05: the eye straight sees the haze between the top of the layer and
/// the twin's plane, which the portal camera does not draw, and the eye before the haze between
/// it and the blue portal (lot 12 report). Each hour has its own gate, between the two, so that
/// each fails without the fix: the haze at noon is thin.
const FROM_ABOVE_SEAMLESS_NOON: f32 = 1.5;
const FROM_ABOVE_SEAMLESS_DUSK: f32 = 6.0;

/// What a test of the seam measures.
#[derive(Clone, Copy, PartialEq)]
enum Test {
    /// The seam, and the walk through the portal.
    Baseline,
    /// The seam with each suspect turned off alone.
    Alone,
    /// The seam with the suspects turned off one after the other, together.
    Together,
    /// The seam, the portals opened after the shaders were turned on.
    LatePortals,
    /// The seam under the weather held at this one: the drops differ from a picture to the
    /// next, so it is for the eye, and for the sky, the fog and the light of the weather.
    Weather(WeatherKind),
    /// A bolt of lightning out of the twin, through the portal and straight.
    Lightning,
    /// The contrast of the drops through the portal and straight, the eye still and strafing.
    RainVeil,
    /// The view through a portal whose twin is just under the top of the layer of haze, from
    /// above, the portal camera above the layer, and from within it, against the view straight
    /// from where the portal camera stands.
    FromAbove,
    /// The seam from the portal camera's own eye, under each filter of the shadows, and without
    /// shadows.
    Shadows,
    /// The seam, whose median must stay under `SEAMLESS` from 32 to 96 blocks, and with the haze
    /// past 96 blocks too, and whose mean, with the haze, under `SKY_SEAMLESS` in the sky and
    /// `SCREEN_SEAMLESS` over all.
    Regression,
}

macro_rules! close_seam {
    ($($name:ident: $quality:ident, $hour:ident, $test:ident, $clouds:expr;)*) => {$(
        #[test]
        #[ignore = "a diagnosis: run it by hand"]
        fn $name() {
            close_seam(ShaderQuality::$quality, Hour::$hour, Test::$test, $clouds);
        }
    )*};
}

close_seam! {
    seam_close_off_noon: Off, Noon, Baseline, false;
    seam_close_off_dusk: Off, Dusk, Baseline, false;
    seam_close_off_night: Off, Night, Baseline, false;
    seam_close_low_noon: Low, Noon, Baseline, false;
    seam_close_low_dusk: Low, Dusk, Baseline, false;
    seam_close_medium_noon: Medium, Noon, Baseline, false;
    seam_close_medium_dusk: Medium, Dusk, Baseline, false;
    seam_close_high_noon: High, Noon, Baseline, false;
    seam_close_high_dusk: High, Dusk, Baseline, false;
    seam_close_high_night: High, Night, Baseline, false;
    seam_close_ultra_noon: Ultra, Noon, Baseline, false;
    seam_close_ultra_dusk: Ultra, Dusk, Baseline, false;
    seam_close_off_noon_alone: Off, Noon, Alone, false;
    seam_close_high_noon_alone: High, Noon, Alone, false;
    seam_close_high_noon_together: High, Noon, Together, false;
    seam_close_high_dusk_alone: High, Dusk, Alone, false;
    seam_close_high_dusk_together: High, Dusk, Together, false;
    seam_close_high_noon_clouds: High, Noon, Baseline, true;
    seam_close_high_noon_late_portals: High, Noon, LatePortals, false;
    seam_close_medium_noon_shadows: Medium, Noon, Shadows, false;
    seam_close_high_dusk_from_above: High, Dusk, FromAbove, false;
    seam_close_high_noon_lightning: High, Noon, Lightning, false;
    seam_close_off_night_lightning: Off, Night, Lightning, false;
    seam_close_high_noon_from_above: High, Noon, FromAbove, false;
    seam_close_high_noon_shadows: High, Noon, Shadows, false;
    seam_close_high_noon_rain_veil: High, Noon, RainVeil, false;
    seam_close_medium_noon_rain_veil: Medium, Noon, RainVeil, false;
}

macro_rules! close_seam_weather {
    ($($name:ident: $quality:ident, $hour:ident, $kind:ident;)*) => {$(
        #[test]
        #[ignore = "a diagnosis: run it by hand"]
        fn $name() {
            close_seam(
                ShaderQuality::$quality,
                Hour::$hour,
                Test::Weather(WeatherKind::$kind),
                false,
            );
        }
    )*};
}

close_seam_weather! {
    seam_close_off_noon_rain: Off, Noon, Rain;
    seam_close_high_noon_rain: High, Noon, Rain;
    seam_close_high_noon_thunder: High, Noon, Thunder;
}

macro_rules! close_seam_regression {
    ($($name:ident: $quality:ident, $hour:ident;)*) => {$(
        #[test]
        #[ignore = "a regression test of 2.5 minutes on lavapipe: run it by hand with --ignored"]
        fn $name() {
            close_seam(ShaderQuality::$quality, Hour::$hour, Test::Regression, false);
        }
    )*};
}

// Through a close twin, crossing is seamless at every level: with the haze at noon and at dusk,
// when it is thickest
close_seam_regression! {
    seam_close_off_noon_is_seamless: Off, Noon;
    seam_close_low_noon_is_seamless: Low, Noon;
    seam_close_medium_noon_is_seamless: Medium, Noon;
    seam_close_high_noon_is_seamless: High, Noon;
    seam_close_high_dusk_is_seamless: High, Dusk;
}

/// How far south the player flies from the orange portal to place the blue one: 16 chunks, beyond
/// its own reach (its render distance and the margin) and that of the chunks sent around the
/// twin (`PORTAL_VIEW_RADIUS`).
const FAR_FLIGHT: f32 = 256.0;

/// How many chunks the client holds.
fn chunks(h: &Harness) -> usize {
    h.world()
        .resource::<ClientWorldMap>()
        .blocks
        .chunk_positions()
        .count()
}

/// The far twin of lot 4, with the portal gun, as the server knows the portals: in the air, the
/// orange portal on a wall at the start, the blue one 16 chunks south, both facing south.
/// Through the blue one, the player sees south from the orange one, where the server has sent
/// it only the chunks within `PORTAL_VIEW_RADIUS` of it. The picture through the portal is taken
/// from an eye just in front of the blue portal; then the player flies through it for real, and
/// the picture straight is taken from the same eye carried through, on the first frames after
/// the crossing and as the chunks around come.
fn far_seam(quality: ShaderQuality, hour: Hour) {
    let start = Vec3::new(-60.5, 110.9, 0.5);
    let mut h = enter_world_with(folder("Far", start, false, true), "Far", options());
    install(&mut h);
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    let in_the_air = |w: &World| {
        local_player(w).is_some_and(|player| {
            let map = &w.resource::<ClientWorldMap>().blocks;
            player.is_flying && shared::players::movement::is_terrain_loaded(player.position, map)
        })
    };
    h.wait_until("the player in the air", SERVER, in_the_air);
    let look = |h: &mut Harness, yaw: f32| {
        let world = h.world_mut();
        let mut controller = world
            .query::<&mut CameraController>()
            .single_mut(world)
            .unwrap();
        controller.angle_x = yaw;
        controller.angle_y = 0.0;
    };
    let eyes = |h: &mut Harness| {
        let world = h.world_mut();
        world
            .query_filtered::<&Transform, With<CameraController>>()
            .single(world)
            .unwrap()
            .translation
    };
    let place = |h: &mut Harness, button: MouseButton, color: PortalColor| {
        let here = local_player(h.world()).unwrap().position.floor().as_ivec3();
        let wall = IVec3::new(here.x, eyes(h).y.floor() as i32, here.z - 8);
        h.command("/wall");
        h.wait_until("the wall", SERVER, |w| {
            w.resource::<ClientWorldMap>()
                .blocks
                .get_block(wall)
                .is_some()
        });
        h.aim_at(wall.as_vec3() + Vec3::new(0.5, 0.5, 1.0), wall);
        hold_click(h, &L::marker::<Hotbar>(), button, 3);
        h.wait_until("the portal", SERVER, |w| own_portal(w, color).is_some());
    };
    look(&mut h, 0.0);
    place(&mut h, MouseButton::Right, PortalColor::Orange);
    look(&mut h, 0.0);
    let z0 = local_player(h.world()).unwrap().position.z;
    h.key_down(KeyCode::KeyS);
    h.wait_until("the far place", Duration::from_secs(600), |w| {
        local_player(w).is_some_and(|player| player.position.z > z0 + FAR_FLIGHT)
    });
    h.key_up(KeyCode::KeyS);
    h.frames(20);
    h.wait_until("the player in the air", SERVER, in_the_air);
    place(&mut h, MouseButton::Left, PortalColor::Blue);
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    let orange = own_portal(h.world(), PortalColor::Orange).unwrap();
    h.wait_until("the chunks around the orange portal", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .is_chunk_loaded(orange.chunk())
    });
    let through = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .through(&blue)
        .unwrap();
    println!("blue {blue:?}, orange {orange:?}");
    h.frames(120);
    set_quality(&mut h, quality);

    let case = format!(
        "far_{}_{}",
        quality_name(quality),
        format!("{hour:?}").to_lowercase()
    );
    let pitch = -25f32.to_radians();
    let before = eye_in_front(&blue, CROSSING, pitch);
    let after = carried(through, &eye_in_front(&blue, -CROSSING, pitch));
    let at_hour = |h: &mut Harness| {
        set_hour(h, hour);
        h.frames(3);
    };
    pinned(&mut h).eye = Some(before);
    at_hour(&mut h);
    let a = shot_at(&mut h, before, SETTLE);
    describe_cameras(&mut h);
    pinned(&mut h).portal_fog_as_main = true;
    at_hour(&mut h);
    let a_fog = shot_at(&mut h, before, SETTLE);
    pinned(&mut h).portal_fog_as_main = false;
    // What lies in the fog of the portal camera, and what without the horizon skirt
    pinned(&mut h).portal_fog_off = true;
    at_hour(&mut h);
    let a_unfogged = shot_at(&mut h, before, SETTLE);
    show_skirt(&mut h, false);
    at_hour(&mut h);
    let a_unfogged_no_skirt = shot_at(&mut h, before, SETTLE);
    show_skirt(&mut h, true);
    pinned(&mut h).portal_fog_off = false;
    show_skirt(&mut h, false);
    at_hour(&mut h);
    let a_no_skirt = shot_at(&mut h, before, SETTLE);
    show_skirt(&mut h, true);
    for (name, shot) in [
        ("unfogged", &a_unfogged),
        ("unfogged_no_skirt", &a_unfogged_no_skirt),
        ("no_skirt", &a_no_skirt),
    ] {
        shot.save(out_dir().join(format!("{case}_{name}_a.png")))
            .unwrap();
    }
    // The chunks around the twin cast shadows, as those around the player do
    let world = h.world_mut();
    let shadowless: Vec<Entity> = world
        .query_filtered::<Entity, With<NotShadowCaster>>()
        .iter(world)
        .collect();
    println!("{} meshes cast no shadow", shadowless.len());
    for entity in &shadowless {
        world.entity_mut(*entity).remove::<NotShadowCaster>();
    }
    at_hour(&mut h);
    let a_shadows = shot_at(&mut h, before, SETTLE);
    let chunks_before = chunks(&h);

    // Through, for real: toward the middle of the blue portal, level
    pinned(&mut h).eye = None;
    let player = local_player(h.world()).unwrap().position;
    let target = Vec3::from(blue.frame().translation);
    look(
        &mut h,
        (-(target.x - player.x)).atan2(-(target.z - player.z)),
    );
    h.frames(5);
    let yaw = |h: &mut Harness| {
        let world = h.world_mut();
        world
            .query::<&CameraController>()
            .single(world)
            .unwrap()
            .angle_x
    };
    let facing = yaw(&mut h);
    h.key_down(KeyCode::KeyW);
    let mut frames = 0;
    while (yaw(&mut h) - facing).abs() < 1.0 {
        h.frames(1);
        frames += 1;
        assert!(
            frames < 600,
            "never went through: {:?}",
            local_player(h.world()).map(|p| p.position)
        );
    }
    h.key_up(KeyCode::KeyW);
    // The first picture straight, on the frames right after the crossing
    pinned(&mut h).eye = Some(after);
    set_hour(&mut h, hour);
    let first = h.screenshot();
    let mut later = Vec::new();
    let mut elapsed = 0;
    for wait in [10, 20, 30, 60, 120, 240] {
        h.frames(wait);
        elapsed += wait;
        at_hour(&mut h);
        elapsed += 3;
        later.push((elapsed, chunks(&h), h.screenshot()));
    }
    let (_, _, last) = later.last().unwrap();
    let pixels = sample_pixels(&mut h, &after, Some((&before, &blue)));
    class_image(&pixels)
        .save(out_dir().join("far_classes.png"))
        .unwrap();
    save(&case, "baseline", &a, last, true);
    record(&case, "baseline", &a, last, &pixels);
    save(&case, "first_frame", &a, &first, true);
    record(&case, "first_frame", &a, &first, &pixels);
    save(&case, "fog_as_main", &a_fog, last, true);
    record(&case, "fog_as_main", &a_fog, last, &pixels);
    save(&case, "shadows", &a_shadows, last, true);
    record(&case, "shadows", &a_shadows, last, &pixels);
    record(&case, "popin_0", &first, last, &pixels);
    for (frames, count, shot) in &later {
        record(
            &case,
            &format!("popin_{frames}_{}chunks", count - chunks_before),
            shot,
            last,
            &pixels,
        );
    }
}

macro_rules! far_seam {
    ($($name:ident: $quality:ident, $hour:ident;)*) => {$(
        #[test]
        #[ignore = "a diagnosis: run it by hand"]
        fn $name() {
            far_seam(ShaderQuality::$quality, Hour::$hour);
        }
    )*};
}

far_seam! {
    seam_far_off_noon: Off, Noon;
    seam_far_high_noon: High, Noon;
    seam_far_high_dusk: High, Dusk;
    seam_far_ultra_noon: Ultra, Noon;
}

/// A wall of stone 8 blocks south of the player, 4 wide and 5 high, its north face toward it.
const SOUTH_WALL_MOD: &str = r#"
    local rc = rustcraft
    rc.register_command("southwall", function(ctx, args)
        local p = rc.player_position(ctx.player)
        for x = -2, 1 do
            for y = -1, 3 do
                rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + 8 }, "stone")
            end
        end
        return "Wall built"
    end)
"#;

/// How far south of the blue portal the player goes before coming back to it, in blocks: past
/// the approach of the portal and the margin the client keeps its chunks in.
const AWAY: f32 = 96.0;

/// How long the chunks around the twin are waited for, once at the portal.
const AREA_WAIT: Duration = Duration::from_secs(180);

/// Turns the player's camera to `yaw`, level: 0 looks north (-z).
fn look(h: &mut Harness, yaw: f32) {
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = yaw;
    controller.angle_y = 0.0;
}

/// Where the player's eyes are.
fn eyes(h: &mut Harness) -> Vec3 {
    let world = h.world_mut();
    world
        .query_filtered::<&Transform, With<CameraController>>()
        .single(world)
        .unwrap()
        .translation
}

/// Whether the player flies, its terrain loaded.
fn in_the_air(w: &World) -> bool {
    local_player(w).is_some_and(|player| {
        let map = &w.resource::<ClientWorldMap>().blocks;
        player.is_flying && shared::players::movement::is_terrain_loaded(player.position, map)
    })
}

/// Holds `key` until `done`, within `frames` frames.
fn fly_until(h: &mut Harness, key: KeyCode, what: &str, done: impl Fn(Vec3) -> bool) {
    h.key_down(key);
    let mut frames = 0;
    while !local_player(h.world()).is_some_and(|player| done(player.position)) {
        h.frames(1);
        frames += 1;
        assert!(
            frames < 3000,
            "never reached {what}: {:?}",
            local_player(h.world()).map(|p| p.position)
        );
    }
    h.key_up(key);
}

/// How many of the chunks within the render distance `distance` of the chunk `twin` the client
/// holds, and how many there are.
fn held_around(w: &World, twin: IVec3, distance: i32) -> (usize, usize) {
    let map = &w.resource::<ClientWorldMap>().blocks;
    let offsets = shared::world::render_distance_offsets(distance);
    let held = offsets
        .iter()
        .filter(|offset| map.is_chunk_loaded(twin + **offset))
        .count();
    (held, offsets.len())
}

/// Two portals of the gun in the air, `apart` chunks from each other along z: the orange one on
/// the north face of a wall, facing north, the blue one south of it on the south face of a
/// wall, facing south, a translation from one to the other. Through the blue one the player
/// looks north out of the orange one, away from both. The player places them, goes `AWAY`
/// blocks south of the blue one, comes back to it and waits for the chunks around the orange
/// one; the picture through the portal is taken from an eye just in front of it, then the
/// player flies through for real and the picture straight is taken from the same eye carried
/// through. How long the chunks took to come, and how many, goes into `approach.tsv`. With
/// `gated`, the seam must be within the gates of the close twins (`assert_seamless`).
fn approach_seam(
    quality: ShaderQuality,
    hour: Hour,
    apart: i32,
    render_distance: u32,
    gated: bool,
) {
    let start = Vec3::new(-60.5, 110.9, 0.5);
    let folder = folder_at("Approach", start, false, true, render_distance)
        .with_mod("southwall", SOUTH_WALL_MOD);
    let mut h = enter_world_with(folder, "Approach", options());
    install(&mut h);
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    h.wait_until("the player in the air", SERVER, in_the_air);
    let place = |h: &mut Harness, south: bool, button: MouseButton, color: PortalColor| {
        let here = local_player(h.world()).unwrap().position.floor().as_ivec3();
        let y = eyes(h).y.floor() as i32;
        let (command, wall, face) = if south {
            (
                "/southwall",
                IVec3::new(here.x, y, here.z + 8),
                Vec3::new(0.5, 0.5, 0.0),
            )
        } else {
            (
                "/wall",
                IVec3::new(here.x, y, here.z - 8),
                Vec3::new(0.5, 0.5, 1.0),
            )
        };
        h.command(command);
        h.wait_until("the wall", SERVER, |w| {
            w.resource::<ClientWorldMap>()
                .blocks
                .get_block(wall)
                .is_some()
        });
        h.aim_at(wall.as_vec3() + face, wall);
        hold_click(h, &L::marker::<Hotbar>(), button, 3);
        h.wait_until("the portal", SERVER, |w| own_portal(w, color).is_some());
    };
    place(&mut h, true, MouseButton::Right, PortalColor::Orange);
    look(&mut h, 0.0);
    let z0 = local_player(h.world()).unwrap().position.z;
    let flight = (apart * 16 + 16) as f32;
    fly_until(&mut h, KeyCode::KeyS, "the place of the blue portal", |p| {
        p.z > z0 + flight
    });
    h.frames(20);
    h.wait_until("the player in the air", SERVER, in_the_air);
    place(&mut h, false, MouseButton::Left, PortalColor::Blue);
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    let orange = own_portal(h.world(), PortalColor::Orange).unwrap();
    let through = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .through(&blue)
        .unwrap();
    assert!(through.transform_vector3(Vec3::X).distance(Vec3::X) < 1e-4);
    println!("blue {blue:?}, orange {orange:?}");
    let case = format!(
        "{}_{}_{}_rd{render_distance}",
        if apart > 12 { "far" } else { "mid" },
        quality_name(quality),
        format!("{hour:?}").to_lowercase()
    );
    let distance = render_distance as i32;

    // Away, then back to the portal
    look(&mut h, 0.0);
    let plane = blue.frame().translation.z;
    fly_until(&mut h, KeyCode::KeyS, "away from the blue portal", |p| {
        p.z > plane + AWAY
    });
    h.frames(30);
    let away = held_around(h.world(), orange.chunk(), distance);
    let back = std::time::Instant::now();
    fly_until(&mut h, KeyCode::KeyW, "the blue portal", |p| {
        p.z < plane + 3.0
    });
    let flight_back = back.elapsed().as_secs_f32();
    let at_portal = held_around(h.world(), orange.chunk(), distance);
    let mut waited = None;
    while back.elapsed() < flight_back_limit(flight_back) {
        let (held, all) = held_around(h.world(), orange.chunk(), distance);
        if held == all {
            waited = Some(back.elapsed().as_secs_f32());
            break;
        }
        h.frames(1);
    }
    let then = held_around(h.world(), orange.chunk(), distance);
    let line = format!(
        "{case}\t{apart} chunks apart\taway {}/{}\tat the portal after {flight_back:.1} s {}/{}\t\
         all held after {}\t{}/{}\n",
        away.0,
        away.1,
        at_portal.0,
        at_portal.1,
        waited.map_or("never".to_string(), |s| format!("{s:.1} s")),
        then.0,
        then.1,
    );
    print!("{line}");
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir().join("approach.tsv"))
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
    h.frames(60);
    set_quality(&mut h, quality);

    let pitch = -25f32.to_radians();
    let before = eye_in_front(&blue, CROSSING, pitch);
    let after = carried(through, &eye_in_front(&blue, -CROSSING, pitch));
    let a = steady_shots(&mut h, before, hour);
    describe_cameras(&mut h);

    // Through, for real
    pinned(&mut h).eye = None;
    let player = local_player(h.world()).unwrap().position;
    let target = Vec3::from(blue.frame().translation);
    look(
        &mut h,
        (-(target.x - player.x)).atan2(-(target.z - player.z)),
    );
    h.frames(5);
    let exit = Vec3::from(orange.frame().translation);
    fly_until(&mut h, KeyCode::KeyW, "the orange portal", |p| {
        p.distance(exit) < 12.0
    });
    pinned(&mut h).eye = Some(after);
    set_hour(&mut h, hour);
    let first = h.screenshot();
    let b = steady_shots(&mut h, after, hour);
    let pixels = sample_pixels(&mut h, &after, Some((&before, &blue)));
    class_image(&pixels)
        .save(out_dir().join(format!("{case}_classes.png")))
        .unwrap();
    record(&case, "noise", &b[0], &b[1], &pixels);
    let (a, b) = (mean_picture(&a), mean_picture(&b));
    save(&case, "baseline", &a, &b, true);
    save(&case, "first_frame", &a, &first, false);
    let first_frame = record(&case, "first_frame", &a, &first, &pixels);
    let popin = record(&case, "popin_0", &first, &b, &pixels);
    let rows = record(&case, "baseline", &a, &b, &pixels);
    if gated {
        assert_seamless(&case, quality, &rows, SCREEN_SEAMLESS);
        // The first frame after crossing too, and nothing comes in after it
        assert_seamless(
            &format!("{case} (first frame)"),
            quality,
            &first_frame,
            FIRST_FRAME_SCREEN_SEAMLESS,
        );
        assert_no_popin(&case, &popin);
    }
}

/// How much, on average, each class of pixels may change from the first frame after crossing to
/// the view settled: chunks or their fog coming in late move a class by tens (9.8–16.7 in the
/// 96+ class in this lot's history). Since the client holds the twin's area on arrival,
/// 0.00–0.06 without the shaders; at High, where the first frame has its noise (TAA, the rays of
/// light) against a mean of three, the worst class is 96+, in two modes over 39 runs:
/// 0.41–0.56 and 1.24–1.74.
const POPIN_SEAMLESS: f32 = 3.0;

/// How much, on average, the whole screen of the first frame after crossing may differ from the
/// view before it, with the haze (`assert_seamless`). At High it is brighter than the settled
/// view (the `lighter` column at −0.8 to −1.3: TAA starts anew) and sits 0.25–0.3 above the
/// baseline, steadily: 1.75–1.86 through a twin 6 chunks away and 2.00–2.06 through one 21 away,
/// over 4 runs; without the shaders it equals the baseline, 1.93–1.94 and 2.36–2.38 over 3. What
/// the gates are for moved it by 16–79 in this lot's history.
const FIRST_FRAME_SCREEN_SEAMLESS: f32 = 3.0;

fn assert_no_popin(case: &str, rows: &[Row]) {
    let failed: Vec<String> = rows
        .iter()
        .filter(|(_, count, mean, ..)| *count > 0 && *mean >= POPIN_SEAMLESS)
        .map(|(class, _, mean, ..)| format!("the {class} class by {mean:.2}"))
        .collect();
    assert!(
        failed.is_empty(),
        "{case}: the view changed after the first frame past the portal: {}",
        failed.join(", ")
    );
}

/// How long to wait for the chunks around the twin from the start of the flight back to the
/// portal, which took `flight_back` seconds.
fn flight_back_limit(flight_back: f32) -> Duration {
    Duration::from_secs_f32(flight_back) + AREA_WAIT
}

macro_rules! approach_seam_regression {
    ($($name:ident: $quality:ident, $hour:ident, $apart:expr;)*) => {$(
        #[test]
        #[ignore = "a regression test of minutes on lavapipe: run it by hand with --ignored"]
        fn $name() {
            approach_seam(
                ShaderQuality::$quality,
                Hour::$hour,
                $apart,
                RENDER_DISTANCE,
                true,
            );
        }
    )*};
}

// Through a twin 6 chunks away, the common case, and 21 chunks away, beyond the render
// distance: once the player came to the portal, crossing is as seamless as through a close twin
approach_seam_regression! {
    seam_mid_off_noon_is_seamless: Off, Noon, 6;
    seam_mid_high_noon_is_seamless: High, Noon, 6;
    seam_far_off_noon_is_seamless: Off, Noon, 21;
    seam_far_high_noon_is_seamless: High, Noon, 21;
}

/// How long the chunks around a far twin take to come at a render distance of 12, measured.
#[test]
#[ignore = "a measure of minutes on lavapipe: run it by hand with --ignored"]
fn seam_far_off_noon_rd12() {
    approach_seam(ShaderQuality::Off, Hour::Noon, 21, 12, false);
}

/// The player's own view, a portal off the screen, before and after the portal is approached:
/// the orange portal is placed, the player flies 21 chunks south and puts up the wall for the
/// blue one, and the view from 30 blocks west of it, looking north towards the twin, the portal
/// out of the picture, is taken; then the blue portal opens on the wall and, once the area
/// around the twin is held, the same view is taken again. It must be the same: the area around
/// the twin, its clouds, its shadows and the haze of the portal camera are only for the portal
/// cameras.
fn own_view_with_a_portal_approached(quality: ShaderQuality, hour: Hour) {
    let render_distance = RENDER_DISTANCE;
    let start = Vec3::new(-60.5, 110.9, 0.5);
    let folder = folder_at("Approach", start, false, true, render_distance)
        .with_mod("southwall", SOUTH_WALL_MOD);
    let mut h = enter_world_with(folder, "Approach", options());
    install(&mut h);
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    h.wait_until("the player in the air", SERVER, in_the_air);
    let wall_at = |h: &mut Harness, command: &str, dz: i32| {
        let here = local_player(h.world()).unwrap().position.floor().as_ivec3();
        let wall = IVec3::new(here.x, eyes(h).y.floor() as i32, here.z + dz);
        h.command(command);
        h.wait_until("the wall", SERVER, |w| {
            w.resource::<ClientWorldMap>()
                .blocks
                .get_block(wall)
                .is_some()
        });
        wall
    };
    let wall = wall_at(&mut h, "/southwall", 8);
    h.aim_at(wall.as_vec3() + Vec3::new(0.5, 0.5, 0.0), wall);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 3);
    h.wait_until("the orange portal", SERVER, |w| {
        own_portal(w, PortalColor::Orange).is_some()
    });
    look(&mut h, 0.0);
    let z0 = local_player(h.world()).unwrap().position.z;
    fly_until(&mut h, KeyCode::KeyS, "the place of the blue portal", |p| {
        p.z > z0 + (21 * 16 + 16) as f32
    });
    h.frames(20);
    h.wait_until("the player in the air", SERVER, in_the_air);
    let wall = wall_at(&mut h, "/wall", -8);
    set_quality(&mut h, quality);
    let case = format!(
        "own_view_{}_{}",
        quality_name(quality),
        format!("{hour:?}").to_lowercase()
    );

    // From 30 blocks west, looking north towards the twin, a little down: the portal's wall is
    // 75° to the right, off the screen
    let player = local_player(h.world()).unwrap().position;
    let pitch = -8f32.to_radians();
    let eye = Transform::from_translation(player + Vec3::new(-30.0, 1.6, 0.0))
        .looking_to(Vec3::new(0.0, pitch.sin(), -pitch.cos()), Vec3::Y);
    let alone = settled_shots(&mut h, eye, hour);

    // The blue portal opens, and the player approaches it
    pinned(&mut h).eye = None;
    h.frames(5);
    h.aim_at(wall.as_vec3() + Vec3::new(0.5, 0.5, 1.0), wall);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.wait_until("the blue portal", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_some()
    });
    let orange = own_portal(h.world(), PortalColor::Orange).unwrap();
    let distance = render_distance as i32;
    let waited = std::time::Instant::now();
    while held_around(h.world(), orange.chunk(), distance).0
        < held_around(h.world(), orange.chunk(), distance).1
    {
        assert!(
            waited.elapsed() < AREA_WAIT,
            "the area around the twin never came"
        );
        h.frames(1);
    }
    let approach = h
        .world()
        .resource::<crate::portal::approach::Approach>()
        .clone();
    assert_eq!(approach.twins, vec![orange.chunk()], "{approach:?}");
    let approached = settled_shots(&mut h, eye, hour);

    // Within the noise: two shots of a window, the view held still, differ by as much
    let pixels = sample_pixels(&mut h, &eye, None);
    let changed = unchanged_view(
        &case,
        "approached",
        (&alone, &approached),
        &pixels,
        0.3,
        Flicker::Checked,
    );
    assert!(
        changed.is_empty(),
        "{case}: the player's own view changed once a portal was approached: {}",
        changed.join(", ")
    );

    // A bolt struck 20 blocks out of the twin, far past the player's reach, at the brightest of
    // its first stroke: drawn through the portal, never in the player's own view, which is the
    // same as with the strike's flash alone (the bolt taken away)
    let reach = (render_distance * shared::CHUNK_SIZE as u32) as f32;
    let twin = Vec3::from(orange.frame().translation);
    let at = (twin + orange.normal() * 20.0).floor().as_ivec3();
    let player = eye.translation;
    assert!(at.as_vec3().xz().distance(player.xz()) > reach + 100.0);
    let seed = 20_261_023;
    let mut strike = LiveStrike::seen(Strike { at, seed }, player, &[twin], reach);
    strike.age = Strokes::new(seed).peaks().next().unwrap();
    assert!(strike.bolt.is_some(), "the bolt by the twin is drawn");
    pinned(&mut h).strikes = Some(vec![strike.clone()]);
    let bolted = settled_shots(&mut h, eye, hour);
    strike.bolt = None;
    pinned(&mut h).strikes = Some(vec![strike]);
    let flash_only = settled_shots(&mut h, eye, hour);
    pinned(&mut h).strikes = None;
    let changed = unchanged_view(
        &case,
        "bolt_by_the_twin",
        (&flash_only, &bolted),
        &pixels,
        0.3,
        Flicker::Checked,
    );
    assert!(
        changed.is_empty(),
        "{case}: a bolt by the far twin showed in the player's own view: {}",
        changed.join(", ")
    );
}

/// The player's own view is the same with a portal approached, at noon and at dusk, when the
/// haze is thickest.
#[test]
#[ignore = "a regression test of minutes on lavapipe: run it by hand with --ignored"]
fn own_view_high_noon_is_unchanged_by_an_approached_portal() {
    own_view_with_a_portal_approached(ShaderQuality::High, Hour::Noon);
}

#[test]
#[ignore = "a regression test of minutes on lavapipe: run it by hand with --ignored"]
fn own_view_high_dusk_is_unchanged_by_an_approached_portal() {
    own_view_with_a_portal_approached(ShaderQuality::High, Hour::Dusk);
}

/// How much the haze must change the view of `own_view_high_dusk_is_unchanged_by_a_stacked_...`
/// on average, for the gate to check it: a layer of haze drawn twice must show.
const HAZE_SEEN: f32 = 0.5;

/// A floor portal straight under a ceiling portal, the same opening, the same up: as a player
/// makes them by shooting down, then up, from where it stands. `Portals::through` is then a
/// vertical translation, and the portal cameras of every level stand straight above the
/// player's camera. The player's own view, looking down into the floor portal at High dusk,
/// the haze at its thickest, against the same view with the ceiling portal gone, the floor
/// portal lone: the same but for the opening and its rim. Each camera draws only its own layer
/// of haze, whatever the cameras over or under it.
#[test]
#[ignore = "a regression test of minutes on lavapipe: run it by hand with --ignored"]
fn own_view_high_dusk_is_unchanged_by_a_stacked_floor_and_ceiling_pair() {
    let (quality, hour) = (ShaderQuality::High, Hour::Dusk);
    let case = "own_view_stacked_high_dusk";
    let player = Vec3::new(-60.5, 140.9, 4.5);
    let mut h = enter_world_with(folder("Seam", player, false, false), "Seam", options());
    install(&mut h);
    wait_for_terrain(&mut h, player.with_y(0.0));
    // A stone floor and, 12 blocks over it, a stone ceiling, the air between them cleared
    let (x0, z0) = (-40, 4);
    let floor_y = ground(&h, x0, z0 - 4, z0 + 6);
    let ceiling_y = floor_y + 12;
    {
        let blocks = &mut h.world_mut().resource_mut::<ClientWorldMap>().blocks;
        let stone = BlockData::new(BlockId::Stone, BlockDirection::Front);
        for x in x0 - 5..=x0 + 4 {
            for z in z0 - 4..=z0 + 6 {
                blocks.set_block(IVec3::new(x, floor_y, z), stone);
                blocks.set_block(IVec3::new(x, ceiling_y, z), stone);
                for y in floor_y + 1..ceiling_y {
                    blocks.remove_block(IVec3::new(x, y, z));
                }
            }
        }
    }
    let floor = Portal {
        owner: 1,
        color: PortalColor::Blue,
        anchor: IVec3::new(x0, floor_y, z0),
        face: FaceDirection::PlusY,
        up: FaceDirection::PlusZ,
    };
    let ceiling = Portal {
        owner: 1,
        color: PortalColor::Orange,
        anchor: IVec3::new(x0 - 1, ceiling_y, z0),
        face: FaceDirection::MinusY,
        up: FaceDirection::PlusZ,
    };
    let (floor_at, ceiling_at) = (
        Vec3::from(floor.frame().translation),
        Vec3::from(ceiling.frame().translation),
    );
    assert_eq!(floor_at.xz(), ceiling_at.xz());
    // From 4 blocks over the floor and 4 back from the opening, looking down into it
    let eye_at = floor_at + Vec3::new(0.0, 4.0, -4.0);
    let eye = Transform::from_translation(eye_at).looking_at(floor_at, Vec3::Y);
    h.world_mut()
        .resource_mut::<ClientWorldMap>()
        .blocks
        .portals_mut()
        .insert(floor);
    pinned(&mut h).eye = Some(eye);
    h.frames(60);
    set_quality(&mut h, quality);

    let lone = settled_shots(&mut h, eye, hour);
    h.world_mut()
        .resource_mut::<ClientWorldMap>()
        .blocks
        .portals_mut()
        .insert(ceiling);
    let through = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .through(&floor)
        .unwrap();
    // A vertical translation only
    assert!(through.transform_vector3(Vec3::X).distance(Vec3::X) < 1e-4);
    assert!(through.transform_vector3(Vec3::Z).distance(Vec3::Z) < 1e-4);
    assert!(Vec3::from(through.translation).xz().length() < 1e-4);
    let paired = settled_shots(&mut h, eye, hour);
    // And without the haze, which the gate must see. From the light: Bevy 0.19 leaves the rays
    // of a camera drawn once it loses its `VolumetricFog`, as long as some light has a
    // `VolumetricLight`
    let haze = take::<VolumetricLight>(&mut h, false);
    let unhazed = settled_shots(&mut h, eye, hour);
    give_back(&mut h, haze);
    // The portal cameras drew, straight above the player's
    let above: Vec<Vec3> = {
        let world = h.world_mut();
        world
            .query::<(&GlobalTransform, &Camera, &PortalCamera)>()
            .iter(world)
            .filter(|(_, camera, _)| camera.is_active)
            .map(|(at, ..)| at.translation())
            .collect()
    };
    println!("{case}: portal cameras drawing at {above:?}");
    assert!(!above.is_empty(), "no portal camera drew");
    assert!(above.iter().all(|at| at.xz().distance(eye_at.xz()) < 1e-3));

    // Every pixel but those of the opening and its rim, half a block around it
    let pixels: Vec<(u32, u32, usize)> = sample_pixels(&mut h, &eye, None)
        .into_iter()
        .filter(|&(x, y, _)| {
            let direction = ray(&eye, x, y);
            let along = -floor.signed_distance(eye_at) / direction.dot(floor.normal());
            let local = floor.local(eye_at + direction * along);
            let half = Vec2::new(PORTAL_WIDTH as f32, PORTAL_HEIGHT as f32) / 2.0 + 0.5;
            along <= 0.0 || local.x.abs() > half.x || local.y.abs() > half.y
        })
        .collect();
    let hazed = record(
        case,
        "haze",
        &mean_picture(&unhazed),
        &mean_picture(&paired),
        &pixels,
    );
    let all = hazed.last().unwrap().2;
    assert!(
        all > HAZE_SEEN,
        "{case}: the haze changes the view by {all:.2} only"
    );
    let changed = unchanged_view(
        case,
        "stacked",
        (&lone, &paired),
        &pixels,
        0.3,
        Flicker::Checked,
    );
    assert!(
        changed.is_empty(),
        "{case}: the player's own view changed with a ceiling portal over the floor portal: {}",
        changed.join(", ")
    );
}

/// A wall of stone 224 blocks south of the player, 48 wide, 2 deep, from 60 blocks under it up
/// to it.
const SHADOW_WALL_MOD: &str = r#"
    local rc = rustcraft
    rc.register_command("shadowwall", function(ctx, args)
        local p = rc.player_position(ctx.player)
        for x = -24, 23 do
            for y = -60, 0 do
                for z = 224, 225 do
                    rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + z }, "stone")
                end
            end
        end
        return "Wall built"
    end)
"#;

/// The player's render distance in `own_view_dusk_is_unchanged_by_the_shadows_of_a_close_twin`:
/// past `SHADOW_CASTER_DISTANCE`, so that the player holds groups that cast shadows only around
/// a twin.
const SHADOWS_RENDER_DISTANCE: u32 = 18;

/// The player's own view at High dusk, looking away from the sun, a little down, with a twin
/// approached 6 chunks towards the sun and without: the groups within `SHADOW_CASTER_DISTANCE`
/// of the twin cast shadows for the portal cameras, some of them 12 to 18 chunks from the
/// player, among its own chunks at a render distance of 18. A stone wall 60 blocks high and 48
/// wide stands 14 chunks towards the sun among them (`SHADOW_WALL_MOD`): with the sun that
/// low, its shadow would reach the player. Nothing but the approach changes: no portal, no
/// clouds. As a control, the wall's shadow shows once its group casts in every view.
#[test]
#[ignore = "a regression test of a minute and a half on lavapipe: run it by hand with --ignored"]
fn own_view_dusk_is_unchanged_by_the_shadows_of_a_close_twin() {
    let hour = Hour::Dusk;
    // In the third chunk of its group along z: the groups 16 to 19 chunks south, 14 to 17 from
    // the player, cast no shadows but around a twin 6 chunks south, and are whole
    let start = Vec3::new(-60.5, 110.9, 40.5);
    let folder = folder_at("Approach", start, false, false, SHADOWS_RENDER_DISTANCE)
        .with_mod("shadowwall", SHADOW_WALL_MOD);
    let mut h = enter_world_with(folder, "Approach", options());
    install(&mut h);
    let reach = SHADOWS_RENDER_DISTANCE as i32 - 1;
    h.wait_until("the terrain", SERVER, |w| {
        let map = &w.resource::<ClientWorldMap>().blocks;
        (-reach..=reach).all(|cx| {
            (-reach..=reach).all(|cz| {
                map.get_height_ground(start + Vec3::new(cx as f32, 0.0, cz as f32) * 16.0) > 0
            })
        })
    });
    h.frames(120);
    pinned(&mut h).twins = Some(Vec::new());
    set_hour(&mut h, hour);
    let tick = pinned(&mut h).tick.unwrap().1;
    let sun = sky_rotation(tick) * Vec3::Z;
    let towards = sun.xz().normalize();
    // The sun is south at dusk, and low
    assert!(towards.y > 0.99 && sun.y < 0.1, "{sun}");
    let towards = Vec3::new(towards.x, 0.0, towards.y);
    let twin = shared::world::world_position_to_chunk_position(start) + IVec3::Z * 6;
    let case = "own_view_shadows_dusk";
    h.command("/shadowwall");
    let wall = (start + Vec3::new(0.0, -10.0, 224.0)).floor().as_ivec3();
    h.wait_until("the wall", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(wall)
            .is_some()
    });
    h.frames(120);
    set_quality(&mut h, ShaderQuality::High);
    let pitch = -12f32.to_radians();
    // Away from the sun: the faces it lights, which a shadow darkens, are in view
    let eye = Transform::from_translation(start + Vec3::Y * 1.6)
        .looking_to(-towards * pitch.cos() + Vec3::Y * pitch.sin(), Vec3::Y);
    let alone = settled_shots(&mut h, eye, hour);
    pinned(&mut h).twins = Some(vec![twin]);
    h.frames(10);
    let approached = settled_shots(&mut h, eye, hour);
    // The control: the groups that cast for the twin only cast in every view
    let world = h.world_mut();
    let for_twins: Vec<Entity> = world
        .query_filtered::<Entity, With<CastsForTwinsOnly>>()
        .iter(world)
        .collect();
    let wall_group = group_of(shared::world::world_position_to_chunk_position(
        wall.as_vec3(),
    ));
    let wall_entity = world.resource::<ClientWorldMap>().render[&wall_group].entity;
    assert!(
        wall_entity.is_some_and(|wall| for_twins.contains(&wall)),
        "the wall casts for the twin only"
    );
    for &entity in &for_twins {
        world.entity_mut(entity).remove::<CastsForTwinsOnly>();
    }
    h.frames(10);
    let in_every_view = settled_shots(&mut h, eye, hour);
    let world = h.world_mut();
    for &entity in &for_twins {
        world.entity_mut(entity).insert(CastsForTwinsOnly);
    }
    pinned(&mut h).twins = Some(Vec::new());
    h.frames(10);
    let pixels = sample_pixels(&mut h, &eye, None);
    let control = unchanged_view(
        case,
        "in_every_view",
        (&alone, &in_every_view),
        &pixels,
        0.3,
        Flicker::Unchecked,
    );
    assert!(
        control.iter().any(|reason| reason.starts_with("64-96 by")),
        "{case}: the wall's shadow does not show even in every view: {control:?}"
    );
    let changed = unchanged_view(
        case,
        "approached",
        (&alone, &approached),
        &pixels,
        0.3,
        Flicker::Checked,
    );
    assert!(
        changed.is_empty(),
        "{case}: the player's own view changed with the twin approached: {changed:?}"
    );
}

/// The sky the chunks fade into, at High noon, before and after the shaders are switched off at
/// dusk and on again at noon: the environment map of the sky follows the hour again.
#[test]
#[ignore = "a regression test of minutes on lavapipe: run it by hand with --ignored"]
fn the_sky_follows_the_hour_after_the_shaders_are_switched_off_and_on() {
    let start = Vec3::new(-60.5, 110.9, 0.5);
    let mut h = enter_world_with(
        folder("Approach", start, false, false),
        "Approach",
        options(),
    );
    install(&mut h);
    wait_for_terrain(&mut h, start);
    set_quality(&mut h, ShaderQuality::High);
    let pitch = -8f32.to_radians();
    let eye = Transform::from_translation(start + Vec3::new(0.0, 1.6, 0.0))
        .looking_to(Vec3::new(0.0, pitch.sin(), -pitch.cos()), Vec3::Y);
    let case = "sky_switched_high_noon";
    let before = settled_shots(&mut h, eye, Hour::Noon);
    let _ = steady_shots(&mut h, eye, Hour::Dusk);
    set_quality(&mut h, ShaderQuality::Off);
    set_hour(&mut h, Hour::Noon);
    h.frames(SETTLE);
    set_quality(&mut h, ShaderQuality::High);
    let after = settled_shots(&mut h, eye, Hour::Noon);

    let pixels = sample_pixels(&mut h, &eye, None);
    let changed = unchanged_view(
        case,
        "switched",
        (&before, &after),
        &pixels,
        0.5,
        Flicker::Unchecked,
    );
    assert!(
        changed.is_empty(),
        "{case}: the view changed once the shaders were switched off and on: {}",
        changed.join(", ")
    );
}

/// The mean colour of the pixels of `shot` whose ray from `eye` meets no block within 400 blocks
/// and rises by `rise` (its `y`, the ray normalised).
fn mean_unblocked(
    h: &Harness,
    shot: &RgbImage,
    eye: &Transform,
    rise: std::ops::Range<f32>,
) -> (Vec3, usize) {
    let map = &h.world().resource::<ClientWorldMap>().blocks;
    let mut sum = Vec3::ZERO;
    let mut n = 0;
    for y in (0..HEIGHT).step_by(2) {
        for x in (0..WIDTH).step_by(2) {
            let direction = ray(eye, x, y);
            if on_the_hud(x, y)
                || !rise.contains(&direction.y)
                || hit_distance(map, eye.translation, direction, 400.0).is_some()
            {
                continue;
            }
            sum += Vec3::from(shot.get_pixel(x, y).0.map(f32::from));
            n += 1;
        }
    }
    (sum / n.max(1) as f32, n)
}

/// Where the sky the chunks fade into is measured: the horizon skirt under the horizon, which is
/// drawn with it (`horizon.wgsl`, `horizon_direction`: the sky at 0.03 over the horizon), against
/// the sky itself at that height, drawn by Bevy from the tables of the atmosphere.
const SKIRT_RISE: std::ops::Range<f32> = -0.12..-0.035;
const SKY_RISE: std::ops::Range<f32> = 0.025..0.035;

/// The colour of the sky the chunks fade into (the horizon skirt), against that of the sky at the
/// height it is read at, from 30 blocks over the plains of the close seam, looking west at
/// `hour`: with the shaders switched on before the portals are opened, after, or with no portals.
fn horizon_colour(quality: ShaderQuality, hour: Hour, portals: Option<bool>) -> f32 {
    let player = Vec3::new(-60.5, 140.9, 4.5);
    let mut h = enter_world_with(folder("Seam", player, false, false), "Seam", options());
    install(&mut h);
    wait_for_terrain(&mut h, player.with_y(0.0));
    let floor = ground(&h, -61, 4, 4);
    let pitch = -6f32.to_radians();
    let eye = Transform::from_translation(Vec3::new(-60.5, floor as f32 + 30.0, 4.5))
        .looking_to(Vec3::new(-pitch.cos(), pitch.sin(), 0.0), Vec3::Y);
    pinned(&mut h).eye = Some(eye);
    let open = |h: &mut Harness| {
        let blue_y = ground(h, -20, 12, 19) + 1;
        build_portal(
            h,
            Portal {
                owner: 1,
                color: PortalColor::Blue,
                anchor: IVec3::new(-20, blue_y, 16),
                face: FaceDirection::PlusX,
                up: FaceDirection::PlusY,
            },
        );
        let orange_y = ground(h, -19, -12, -5) + 1;
        build_portal(
            h,
            Portal {
                owner: 1,
                color: PortalColor::Orange,
                anchor: IVec3::new(-19, orange_y, -9),
                face: FaceDirection::MinusX,
                up: FaceDirection::PlusY,
            },
        );
        h.frames(10);
    };
    if portals == Some(true) {
        open(&mut h);
    }
    set_quality(&mut h, quality);
    if portals == Some(false) {
        open(&mut h);
    }
    let shots = steady_shots(&mut h, eye, hour);
    let shot = mean_picture(&shots);
    let case = format!(
        "horizon_{}_{}_{}",
        quality_name(quality),
        format!("{hour:?}").to_lowercase(),
        match portals {
            None => "no_portals",
            Some(true) => "portals_first",
            Some(false) => "portals_after",
        }
    );
    shot.save(out_dir().join(format!("{case}.png"))).unwrap();
    let (skirt, n_skirt) = mean_unblocked(&h, &shot, &eye, SKIRT_RISE);
    let (sky, n_sky) = mean_unblocked(&h, &shot, &eye, SKY_RISE);
    // Both read over enough pixels (9712 and 1368 as built): an empty band would read black on
    // both sides, and pass
    assert!(
        n_skirt >= 500 && n_sky >= 500,
        "{case}: the horizon read over {n_skirt} pixels of skirt and {n_sky} of sky"
    );
    let apart = (skirt - sky).abs().element_sum() / 3.0;
    let line = format!(
        "{case}\tskirt {:.0},{:.0},{:.0} ({n_skirt})\tsky {:.0},{:.0},{:.0} ({n_sky})\tapart {apart:.2}\n",
        skirt.x, skirt.y, skirt.z, sky.x, sky.y, sky.z
    );
    print!("{line}");
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir().join("horizon.tsv"))
        .unwrap()
        .write_all(line.as_bytes())
        .unwrap();
    apart
}

/// How far apart the horizon skirt and the sky over it may be (`horizon_colour`, out of 255, the
/// mean of the channels). With the sky of the portal cameras given as before lot 11, Bevy drew
/// the player's map of the sky with a portal camera's tables and they were 49.4–49.7 apart, the
/// skirt a dark grey-brown 68,66,60 under a sky of 105,120,120 (F6 of the seam diagnosis); since,
/// 0.28–0.54 at noon and 1.20–1.22 at dusk (lot 12 report).
const HORIZON_APART: f32 = 4.0;

macro_rules! horizon_colour {
    ($($name:ident: $quality:ident, $hour:ident, $portals:expr;)*) => {$(
        #[test]
        #[ignore = "a regression test of a minute on lavapipe: run it by hand with --ignored"]
        fn $name() {
            let apart = horizon_colour(ShaderQuality::$quality, Hour::$hour, $portals);
            assert!(
                apart < HORIZON_APART,
                "the sky the chunks fade into is {apart:.2} off the sky at the horizon"
            );
        }
    )*};
}

// The sky the chunks fade into is the sky at the horizon, whenever the portals are opened
horizon_colour! {
    horizon_high_noon_no_portals: High, Noon, None;
    horizon_high_noon_portals_first: High, Noon, Some(true);
    horizon_high_noon_portals_after: High, Noon, Some(false);
    horizon_high_dusk_portals_first: High, Dusk, Some(true);
    horizon_ultra_noon_portals_first: Ultra, Noon, Some(true);
    horizon_medium_noon_portals_first: Medium, Noon, Some(true);
}
