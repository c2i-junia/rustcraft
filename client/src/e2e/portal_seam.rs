//! The seam of a crossing, for its diagnosis (`.superpowers/sdd/portals/seam-diagnosis.md`): the
//! view through a portal from an eye just in front of it, against the view straight from the
//! same eye carried through the portal (`Portals::through`), as the player sees them on the last
//! frame before it walks through and on the first after. Most are measures, not checks; the
//! `*_is_seamless` tests also check that a close twin is crossed without a seam, at noon at every
//! level and at dusk at High. Each test saves its pictures, their differences and its measures into
//! `.superpowers/sdd/portals/seam-screens/`, and is ignored: each takes minutes. Run them, two
//! at most at once (with three, a server timed out), with
//! `E2E_LIFETIME_SECS=3000 cargo test -p client e2e::portal_seam -- --ignored --test-threads=2`.
//!
//! The player's camera is pinned to the eye measured (`Pinned`), so that both pictures are taken
//! from exactly the same place, and some suspects of the seam can be turned off, one by one, to
//! see how much of it each makes.

use std::io::Write as _;
use std::path::PathBuf;

use bevy::anti_alias::taa::TemporalAntiAliasing;
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
    PortalColor, PORTAL_HEIGHT, PORTAL_WIDTH,
};
use shared::{DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::{enter_world_with, hold_click, slot};
use super::harness::{Harness, Options, SERVER};
use super::locator::Locator as L;
use super::portals::{local_player, own_portal, FAR_MOD};
use super::seed::GameFolder;
use crate::camera::{CameraController, FOV_DEGREES};
use crate::portal::PortalCamera;
use crate::state::GameSet;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::celestial::sun_height;
use crate::world::rendering::shaders::ShaderFeatures;
use crate::world::time::ClientTime;
use crate::world::{ClientWorldMap, GraphicsSettings, ShaderQuality};
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
/// only the pixels of the opening of that portal from that eye.
fn sample_pixels(
    h: &Harness,
    seen: &Transform,
    before: Option<(&Transform, &Portal)>,
) -> Vec<(u32, u32, usize)> {
    let map = &h.world().resource::<ClientWorldMap>().blocks;
    let mut pixels = Vec::new();
    for y in (0..HEIGHT).step_by(2) {
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
    pixels
}

/// Whether the pixel `(x, y)` is under the minimap, the hotbar or the crosshair.
fn on_the_hud(x: u32, y: u32) -> bool {
    (x >= 780 && y <= 172)
        || ((234..=726).contains(&x) && (450..=512).contains(&y))
        || ((466..=494).contains(&x) && (256..=284).contains(&y))
}

fn luma(pixel: &image::Rgb<u8>) -> f32 {
    let [r, g, b] = pixel.0.map(f32::from);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The difference of `a` and `b` over `pixels`, by class and over all: how many pixels, the mean
/// of the mean difference of their channels, its 95th percentile and its maximum, from 0 to 255,
/// and how much brighter `a` is on average (luma).
fn measure(
    a: &RgbImage,
    b: &RgbImage,
    pixels: &[(u32, u32, usize)],
) -> Vec<(String, usize, f32, f32, f32, f32)> {
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
                return (name, 0, 0.0, 0.0, 0.0, 0.0);
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
) -> Vec<(String, usize, f32, f32, f32, f32)> {
    let rows = measure(a, b, pixels);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out_dir().join("results.tsv"))
        .unwrap();
    let mut text = String::new();
    for (class, n, mean, p95, max, lighter) in &rows {
        text += &format!(
            "{case}\t{variant}\t{class}\t{n}\t{mean:.2}\t{p95:.1}\t{max:.1}\t{lighter:+.2}\n"
        );
    }
    file.write_all(text.as_bytes()).unwrap();
    print!("{text}");
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
    fn measure(
        &self,
        h: &mut Harness,
        variant: &str,
        pictures: bool,
    ) -> Vec<(String, usize, f32, f32, f32, f32)> {
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

fn remove_from_portals<T: Component>(h: &mut Harness) {
    let world = h.world_mut();
    let cameras: Vec<Entity> = world
        .query_filtered::<Entity, With<PortalCamera>>()
        .iter(world)
        .collect();
    for camera in cameras {
        world.entity_mut(camera).remove::<T>();
    }
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
                "(render_distance: {RENDER_DISTANCE}, clouds: {})",
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
    let orange_y = ground(&h, -19, -12, -5) + 1;
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
    let pixels = sample_pixels(&h, &after, Some((&before, &blue)));
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
        Test::PortalSsao => {
            crossing.measure(&mut h, "baseline", true);
            remove_from_portals::<ScreenSpaceAmbientOcclusion>(&mut h);
            crossing.measure(&mut h, "portal_no_ssao", true);
        }
        Test::LatePortals => {
            crossing.case += "_late_portals";
            crossing.measure(&mut h, "baseline", true);
        }
        Test::Regression => {
            crossing.case += "_regression";
            let rows = crossing.measure(&mut h, "baseline", true);
            // With the haze, what it veils counts too: the far terrain, the sky, the screen
            let hazed = ShaderFeatures::of(quality).god_rays.is_some();
            for (class, _, mean, ..) in &rows {
                let threshold = match class.as_str() {
                    "32-64" | "64-96" => SEAMLESS,
                    "96+" if hazed => SEAMLESS,
                    "sky" if hazed => SKY_SEAMLESS,
                    "all" if hazed => SCREEN_SEAMLESS,
                    _ => continue,
                };
                assert!(
                    *mean < threshold,
                    "{}: the {class} class differs by {mean:.2} across the portal",
                    crossing.case
                );
            }
        }
    }
}

/// How much, on average, the 32–64 and 64–96 blocks classes may differ across a crossing (out of
/// 255, `measure`) at Off, Low and Medium, which draw neither the god-ray haze nor SSAO. With
/// the portal camera's fog shortened to 64 blocks they differed by 16 to 70; with the player's
/// fog, by 2 to 4: the parallax of the step and the noise of the frame (lot 9 report).
const SEAMLESS: f32 = 6.0;

/// How much, on average, the sky and the whole screen may differ across a crossing at High and
/// Ultra, which draw the god-ray haze and SSAO, and the 96+ blocks class there (`SEAMLESS`).
/// Before the portal cameras drew the haze, the sky differed by 6 at noon and 55 at dusk, the
/// whole screen by 5.8 and 23, the 96+ class by 17 and 10; after, with no SSAO on the portal
/// cameras, by 0.1–0.2, 0.5–1.1 and 0.4–2.4, against a noise of 0.2–2.3, 0.4–1.7 and 0.6–2.4
/// between two pictures from the same eye (lot 10 report).
const SKY_SEAMLESS: f32 = 3.0;
const SCREEN_SEAMLESS: f32 = 2.5;

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
    /// The seam, then the seam without SSAO on the portal cameras.
    PortalSsao,
    /// The seam, which must stay under `SEAMLESS` from 32 to 96 blocks, and with the haze under
    /// `SEAMLESS` past 96 blocks, `SKY_SEAMLESS` in the sky and `SCREEN_SEAMLESS` over all.
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
    seam_close_high_noon_portal_ssao: High, Noon, PortalSsao, false;
    seam_close_high_dusk_portal_ssao: High, Dusk, PortalSsao, false;
    seam_close_ultra_noon_portal_ssao: Ultra, Noon, PortalSsao, false;
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
    let pixels = sample_pixels(&h, &after, Some((&before, &blue)));
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
