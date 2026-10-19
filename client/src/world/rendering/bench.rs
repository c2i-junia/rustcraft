//! Benchmarks of the chunk meshes, on terrain made by the world generator. `cargo test` skips
//! them; run them in release, one at a time:
//!
//! ```sh
//! BENCH_LABEL=baseline cargo test -p client --release bench_ -- --ignored --nocapture --test-threads 1
//! ```
//!
//! Each bench writes `<label>-<bench>.json` into `BENCH_OUT` (default `target/bench`).
//! `bench_drawing` draws with the GPU, or with lavapipe when `BENCH_ADAPTER=lavapipe`, and saves
//! its screenshots next to its results.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use bevy::{
    app::{AppLabel, SubApps},
    asset::{RenderAssetUsages, UnapprovedPathMode},
    audio::AudioPlugin,
    camera::RenderTarget,
    ecs::system::RunSystemOnce,
    gilrs::GilrsPlugin,
    log::LogPlugin,
    mesh::Indices,
    pbr::wireframe::{WireframeConfig, WireframePlugin},
    prelude::*,
    render::{
        pipelined_rendering::PipelinedRenderingPlugin,
        render_resource::{
            Extent3d, PollType, TextureDimension, TextureFormat, TextureUsages, WgpuFeatures,
        },
        renderer::RenderDevice,
        settings::{RenderCreation, WgpuSettings},
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderApp, RenderPlugin,
    },
    tasks::{block_on, AsyncComputeTaskPool, TaskPool},
    window::ExitCondition,
    winit::WinitPlugin,
};
use serde_json::{json, Value};
use shared::world::{BlockId, ChunkMap, ItemId};
use shared::{GameFolderPaths, CHUNK_SIZE};

use super::meshing::{generate_chunk_mesh, ChunkMeshResponse};
use super::render::{spawn_meshing_tasks, update_chunk};
use crate::camera::SKY_COLOR;
use crate::constants::TEXTURE_PATH_BASE;
use crate::state::{PreLoadingCompletion, TexturePath};
use crate::world::celestial::{celestial_shadows_system, CelestialRoot};
use crate::world::{
    chunk_material_plugin, create_all_atlases, setup_materials, AtlasHandles, ChunkRender,
    MaterialResource,
};

const SEED: u32 = 42;
/// The chunk layers of every scene: heights 0 to 95, the ground and what grows on it.
const LAYERS: std::ops::Range<i32> = 0..6;
/// How many times each chunk is meshed; the first round only warms the caches up.
const ROUNDS: usize = 6;
/// How many times one chunk is meshed again, as when a block is broken.
const REMESHES: usize = 60;

/// A square of chunks, `2 * radius + 1` on a side, around the world column `center` (in
/// blocks), over the `LAYERS`.
struct Scene {
    name: &'static str,
    center: (i32, i32),
    radius: i32,
}

/// One scene per kind of terrain, at columns whose biome `generation` pins for the seed.
const SCENES: [Scene; 5] = [
    Scene {
        name: "plains",
        center: (0, 0),
        radius: 4,
    },
    Scene {
        name: "forest",
        center: (-1500, -1278),
        radius: 4,
    },
    Scene {
        name: "mountains",
        center: (-1500, -1056),
        radius: 4,
    },
    Scene {
        name: "desert",
        center: (-1500, -723),
        radius: 4,
    },
    Scene {
        name: "ocean",
        center: (-1463, -1352),
        radius: 4,
    },
];

/// The scene drawn by `bench_drawing`: mountains, seen from far.
const DRAWN_SCENE: Scene = Scene {
    name: "mountains",
    center: (-1500, -1056),
    radius: 8,
};

impl Scene {
    fn center_chunk(&self) -> IVec3 {
        IVec3::new(self.center.0, 0, self.center.1).div_euclid(IVec3::splat(CHUNK_SIZE))
    }

    fn chunk_positions(&self) -> Vec<IVec3> {
        let center = self.center_chunk();
        let mut positions = Vec::new();
        for x in -self.radius..=self.radius {
            for z in -self.radius..=self.radius {
                for y in LAYERS {
                    positions.push(IVec3::new(center.x + x, y, center.z + z));
                }
            }
        }
        positions
    }

    /// The chunks of the scene, generated on every core.
    fn generate(&self) -> ChunkMap {
        let positions = self.chunk_positions();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let chunks: Vec<_> = std::thread::scope(|scope| {
            let workers: Vec<_> = positions
                .chunks(positions.len().div_ceil(threads))
                .map(|part| {
                    scope.spawn(move || {
                        part.iter()
                            .map(|pos| (*pos, server::generate_chunk(*pos, SEED)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            workers
                .into_iter()
                .flat_map(|worker| worker.join().unwrap())
                .collect()
        });
        let mut world = ChunkMap::default();
        for (pos, chunk) in chunks {
            world.insert_chunk(pos, chunk);
        }
        world
    }

    /// The chunk of the ground at the centre of the scene: the highest one that has blocks.
    fn surface_chunk(&self, world: &ChunkMap) -> IVec3 {
        let center = self.center_chunk();
        LAYERS
            .rev()
            .map(|y| center.with_y(y))
            .find(|pos| world.chunk(*pos).is_some_and(|chunk| !chunk.is_empty()))
            .expect("the scene has ground")
    }
}

/// An atlas tile for each block texture of the game, as the mesher only reads their indices.
fn tiles() -> HashMap<String, u32> {
    let folder =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/graphics/base_textures/blocks");
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .filter_map(|file| Some(file.ok()?.path().file_stem()?.to_str()?.to_owned()))
        .collect();
    names.sort();
    names.into_iter().zip(0..).collect()
}

/// What the meshes of a scene take on the GPU.
#[derive(Default)]
struct MeshStats {
    meshes: usize,
    vertices: usize,
    triangles: usize,
    bytes: usize,
    /// Of the positions, normals, UVs, colours and indices, in order: equal for equal meshes.
    checksum: DefaultHasher,
}

impl MeshStats {
    fn add(&mut self, response: &ChunkMeshResponse) {
        for mesh in [&response.solid_mesh, &response.liquid_mesh]
            .into_iter()
            .flatten()
        {
            self.meshes += 1;
            self.vertices += mesh.count_vertices();
            let (indices, index_bytes) = match mesh.indices() {
                Some(Indices::U32(indices)) => (indices.len(), indices.len() * 4),
                Some(Indices::U16(indices)) => (indices.len(), indices.len() * 2),
                None => (0, 0),
            };
            self.triangles += indices / 3;
            for attribute in [
                Mesh::ATTRIBUTE_POSITION,
                Mesh::ATTRIBUTE_NORMAL,
                Mesh::ATTRIBUTE_UV_0,
                Mesh::ATTRIBUTE_COLOR,
            ] {
                if let Some(values) = mesh.attribute(attribute) {
                    values.get_bytes().hash(&mut self.checksum);
                }
            }
            if let Some(indices) = mesh.indices() {
                indices
                    .iter()
                    .for_each(|index| index.hash(&mut self.checksum));
            }
            self.bytes += index_bytes
                + mesh
                    .attributes()
                    .map(|(_, values)| values.get_bytes().len())
                    .sum::<usize>();
        }
    }

    fn json(&self) -> Value {
        json!({
            "meshes": self.meshes,
            "vertices": self.vertices,
            "triangles": self.triangles,
            "bytes": self.bytes,
            "checksum": format!("{:016x}", self.checksum.finish()),
        })
    }
}

/// Mean, quantiles and extremes of durations in milliseconds.
fn summary(mut samples: Vec<f64>) -> Value {
    samples.sort_by(f64::total_cmp);
    let quantile = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
    json!({
        "n": samples.len(),
        "mean": samples.iter().sum::<f64>() / samples.len() as f64,
        "min": samples[0],
        "p50": quantile(0.5),
        "p95": quantile(0.95),
        "max": samples[samples.len() - 1],
    })
}

fn millis(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1e3
}

fn output_dir() -> PathBuf {
    let dir = std::env::var_os("BENCH_OUT").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/bench"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn label() -> String {
    std::env::var("BENCH_LABEL").unwrap_or_else(|_| "current".into())
}

fn write_results(bench: &str, results: Value) {
    let path = output_dir().join(format!("{}-{bench}.json", label()));
    std::fs::write(&path, serde_json::to_string_pretty(&results).unwrap()).unwrap();
    println!("{bench}: wrote {}", path.display());
}

#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_meshing() {
    let tiles = tiles();
    AsyncComputeTaskPool::get_or_init(TaskPool::default);
    let mut scenes = Vec::new();

    for scene in &SCENES {
        let world = scene.generate();
        let positions = scene.chunk_positions();

        // One chunk after the other on this thread: the cost of each chunk
        let mut stats = MeshStats::default();
        let mut per_chunk = Vec::new();
        let mut sequential = Vec::new();
        for round in 0..ROUNDS {
            let start = Instant::now();
            for pos in &positions {
                let chunk_start = Instant::now();
                let response = generate_chunk_mesh(&world, world.chunk(*pos).unwrap(), pos, &tiles);
                if round > 0 {
                    per_chunk.push(millis(chunk_start));
                } else {
                    stats.add(&response);
                }
                black_box(response);
            }
            if round > 0 {
                sequential.push(millis(start));
            }
        }

        // As in the game when the world arrives: every chunk asked for at once, meshed on the
        // task pool while the main thread goes on
        let mut load_main_thread = Vec::new();
        let mut load_total = Vec::new();
        for round in 0..ROUNDS {
            let start = Instant::now();
            let tasks = spawn_meshing_tasks(&world, &tiles, positions.clone());
            let main_thread = millis(start);
            for task in tasks {
                black_box(block_on(task.thread));
            }
            if round > 0 {
                load_main_thread.push(main_thread);
                load_total.push(millis(start));
            }
        }

        // A block broken at the centre of the scene: its chunk is meshed again
        let surface = scene.surface_chunk(&world);
        let mut remesh_main_thread = Vec::new();
        let mut remesh_total = Vec::new();
        for _ in 0..REMESHES {
            let start = Instant::now();
            let tasks = spawn_meshing_tasks(&world, &tiles, vec![surface]);
            remesh_main_thread.push(millis(start));
            for task in tasks {
                black_box(block_on(task.thread));
            }
            remesh_total.push(millis(start));
        }

        println!(
            "{}: {} chunks, {} vertices, {:.1} ms per round",
            scene.name,
            positions.len(),
            stats.vertices,
            sequential.iter().sum::<f64>() / sequential.len() as f64
        );
        scenes.push(json!({
            "name": scene.name,
            "chunks": positions.len(),
            "mesh": stats.json(),
            "per_chunk_ms": summary(per_chunk),
            "sequential_ms": summary(sequential),
            "load_main_thread_ms": summary(load_main_thread),
            "load_total_ms": summary(load_total),
            "remesh_main_thread_ms": summary(remesh_main_thread),
            "remesh_total_ms": summary(remesh_total),
        }));
    }

    write_results(
        "meshing",
        json!({
            "label": label(),
            "threads": AsyncComputeTaskPool::get().thread_num(),
            "scenes": scenes,
        }),
    );
}

const WIDTH: u32 = 1600;
const HEIGHT: u32 = 900;
/// The frames that loading the textures or capturing the screen may take.
const MAX_FRAMES: usize = 500;

/// A headless app that draws the game's chunks into an image, with the game's materials, light
/// and sky.
struct Drawing {
    apps: SubApps,
    target: Handle<Image>,
}

impl Drawing {
    fn new(lavapipe: bool) -> Self {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let mut app = App::new();
        app.add_plugins((
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: data.to_string_lossy().into_owned(),
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                        force_fallback_adapter: lavapipe,
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        ..default()
                    })),
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                // One frame after the other, so that a frame is timed from its update to the
                // end of its drawing, and the render world can be read
                .disable::<PipelinedRenderingPlugin>()
                .disable::<WinitPlugin>()
                .disable::<AudioPlugin>()
                .disable::<GilrsPlugin>()
                .disable::<LogPlugin>(),
            WireframePlugin::default(),
            chunk_material_plugin,
        ))
        .insert_resource(MaterialResource::default())
        .insert_resource(AtlasHandles::<BlockId>::default())
        .insert_resource(AtlasHandles::<ItemId>::default())
        .insert_resource(PreLoadingCompletion {
            textures_loaded: false,
        })
        .insert_resource(TexturePath {
            path: TEXTURE_PATH_BASE.to_string(),
        })
        .insert_resource(GameFolderPaths {
            game_folder_path: data.clone(),
            assets_folder_path: data,
        })
        // As in `world_plugin`
        .insert_resource(GlobalAmbientLight {
            color: Color::WHITE,
            brightness: 400.0,
            ..default()
        });
        app.finish();
        app.cleanup();

        let mut drawing = Self {
            apps: std::mem::take(app.sub_apps_mut()),
            target: Handle::default(),
        };
        drawing
            .world_mut()
            .run_system_once(setup_materials)
            .unwrap();
        for _ in 0..MAX_FRAMES {
            drawing.apps.update();
            drawing
                .world_mut()
                .run_system_once(create_all_atlases)
                .unwrap();
            if drawing
                .world()
                .resource::<PreLoadingCompletion>()
                .textures_loaded
            {
                break;
            }
        }
        assert!(
            drawing
                .world()
                .resource::<PreLoadingCompletion>()
                .textures_loaded,
            "the textures did not load"
        );

        let mut target = Image::new_fill(
            Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        drawing.target = drawing
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(target);
        let target = RenderTarget::Image(drawing.target.clone().into());
        drawing.world_mut().spawn((
            Camera3d::default(),
            Camera {
                clear_color: ClearColorConfig::Custom(SKY_COLOR),
                ..default()
            },
            Projection::Perspective(PerspectiveProjection {
                fov: f32::to_radians(60.0),
                ..default()
            }),
            target,
        ));
        // The sun of the morning, as `setup_main_lighting` makes it. `BENCH_LIGHTS=game` adds
        // the moon below the horizon, opposite the sun around their anchor, and lets the
        // game's `celestial_shadows_system` choose which casts shadows; `none` draws no
        // shadows at all.
        let lights = std::env::var("BENCH_LIGHTS").unwrap_or_default();
        let sun = Quat::from_euler(EulerRot::YXZ, 0.7, -0.8, 0.);
        let light = |illuminance| DirectionalLight {
            illuminance,
            shadow_maps_enabled: lights != "none",
            ..default()
        };
        if lights == "game" {
            let world = drawing.world_mut();
            let sun_light = world.spawn((light(5000.), Transform::IDENTITY)).id();
            let moon_light = world
                .spawn((
                    light(100.),
                    Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)),
                ))
                .id();
            world
                .spawn((CelestialRoot, Transform::from_rotation(sun)))
                .add_children(&[sun_light, moon_light]);
            world.run_system_once(celestial_shadows_system).unwrap();
        } else {
            drawing
                .world_mut()
                .spawn((light(5000.), Transform::from_rotation(sun)));
        }
        drawing
    }

    fn world(&self) -> &World {
        self.apps.main.world()
    }

    fn world_mut(&mut self) -> &mut World {
        self.apps.main.world_mut()
    }

    /// Meshes every chunk of `world` and shows it, as `world_render_system` does.
    fn show(&mut self, world: &ChunkMap) -> MeshStats {
        let tiles = self
            .world()
            .resource::<MaterialResource>()
            .blocks
            .as_ref()
            .unwrap()
            .tiles
            .clone();
        let mut stats = MeshStats::default();
        let mut responses: Vec<_> = world
            .chunk_positions()
            .map(|pos| {
                let response = generate_chunk_mesh(world, world.chunk(pos).unwrap(), &pos, &tiles);
                stats.add(&response);
                (pos, response)
            })
            .collect();
        self.world_mut()
            .run_system_once(
                move |mut commands: Commands,
                      mut meshes: ResMut<Assets<Mesh>>,
                      materials: Res<MaterialResource>| {
                    for (pos, response) in responses.drain(..) {
                        let mut render = ChunkRender::new();
                        update_chunk(
                            &mut render,
                            &pos,
                            &materials,
                            &mut commands,
                            &mut meshes,
                            response,
                        );
                    }
                },
            )
            .unwrap();
        stats
    }

    fn render_world(&self) -> &World {
        self.apps.sub_apps[&RenderApp.intern()].world()
    }

    fn place_camera(&mut self, transform: Transform) {
        let mut cameras = self
            .world_mut()
            .query_filtered::<&mut Transform, With<Camera3d>>();
        *cameras.single_mut(self.world_mut()).unwrap() = transform;
    }

    /// Draws a frame and waits until the GPU is done with it.
    fn frame(&mut self) {
        self.apps.update();
        let device: &RenderDevice = self.render_world().resource();
        device.poll(PollType::wait_indefinitely()).unwrap();
    }

    fn screenshot(&mut self, path: &Path) {
        let shot = Arc::new(Mutex::new(None));
        let captured = shot.clone();
        let target = self.target.clone();
        self.world_mut().spawn(Screenshot::image(target)).observe(
            move |screenshot: On<ScreenshotCaptured>| {
                *captured.lock().unwrap() = Some(screenshot.image.clone());
            },
        );
        for _ in 0..MAX_FRAMES {
            self.frame();
            if let Some(image) = shot.lock().unwrap().take() {
                image
                    .try_into_dynamic()
                    .unwrap()
                    .to_rgb8()
                    .save(path)
                    .unwrap();
                return;
            }
        }
        panic!("the screenshot was never captured");
    }
}

#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_drawing() {
    let lavapipe = std::env::var("BENCH_ADAPTER").is_ok_and(|adapter| adapter == "lavapipe");
    let adapter = if lavapipe { "lavapipe" } else { "gpu" };
    let frames: usize = std::env::var("BENCH_FRAMES")
        .ok()
        .and_then(|frames| frames.parse().ok())
        .unwrap_or(if lavapipe { 40 } else { 400 });

    let mut drawing = Drawing::new(lavapipe);
    let adapter_name = drawing
        .render_world()
        .resource::<bevy::render::renderer::RenderAdapterInfo>()
        .name
        .clone();
    let world = DRAWN_SCENE.generate();
    let stats = drawing.show(&world);

    let center = DRAWN_SCENE.center_chunk() * CHUNK_SIZE + IVec3::splat(CHUNK_SIZE / 2);
    let ground = world.get_height_ground(center.as_vec3()) as f32;
    let eye = Vec3::new(center.x as f32, ground + 24., center.z as f32);
    // A full turn over the measured frames, looking a bit down
    let turn = |frame: usize, of: usize| {
        let yaw = std::f32::consts::TAU * frame as f32 / of as f32;
        Transform::from_translation(eye).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            yaw,
            -0.3,
            0.,
        ))
    };

    for frame in 0..30 {
        drawing.place_camera(turn(frame, 30));
        drawing.frame();
    }
    let mut frame_ms = Vec::new();
    for frame in 0..frames {
        drawing.place_camera(turn(frame, frames));
        let start = Instant::now();
        drawing.frame();
        frame_ms.push(millis(start));
    }

    let prefix = format!("{}-{adapter}", label());
    drawing.place_camera(turn(1, 8));
    drawing.screenshot(&output_dir().join(format!("{prefix}-view.png")));
    // Close to a slope, with the edges of every triangle drawn
    drawing.world_mut().insert_resource(WireframeConfig {
        global: true,
        default_color: Color::BLACK,
        ..default()
    });
    drawing.place_camera(
        Transform::from_translation(eye + Vec3::new(-6., -8., 10.))
            .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.9, -0.5, 0.)),
    );
    drawing.screenshot(&output_dir().join(format!("{prefix}-wireframe.png")));

    println!(
        "{adapter} ({adapter_name}): {} vertices, {:.2} ms per frame",
        stats.vertices,
        frame_ms.iter().sum::<f64>() / frame_ms.len() as f64
    );
    write_results(
        &format!("drawing-{adapter}"),
        json!({
            "label": label(),
            "adapter": adapter_name,
            "resolution": [WIDTH, HEIGHT],
            "scene": DRAWN_SCENE.name,
            "chunks": world.chunk_positions().count(),
            "mesh": stats.json(),
            "frame_ms": summary(frame_ms),
        }),
    );
}

#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_minimap() {
    use crate::ui::hud::minimap::colors::MinimapColors;
    use crate::ui::hud::minimap::compose::{compose, compose_world, MAP_SIZE};
    use crate::ui::hud::minimap::surface::MinimapSurface;
    use crate::ui::hud::minimap::ZOOMS;

    let colors = MinimapColors::of(BlockId::ALL.map(|id| (id, [id as u8; 3])));
    let mut pixels = vec![0; (MAP_SIZE * MAP_SIZE * 4) as usize];
    let mut scenes = Vec::new();
    for scene in &SCENES {
        let world = scene.generate();
        let center = scene.center_chunk().xz();
        let columns = (2 * scene.radius + 1).pow(2) as usize;

        // Every column of the scene scanned, as when the world arrives, and one column alone
        let mut scan_all = Vec::new();
        let mut scan_column = Vec::new();
        let mut surface = MinimapSurface::default();
        for _ in 0..ROUNDS {
            surface = MinimapSurface::of(&world);
            let start = Instant::now();
            surface.scan_dirty(&world, center, columns);
            scan_all.push(millis(start));
            let mut one = MinimapSurface::of(&world);
            let start = Instant::now();
            one.scan_dirty(&world, center, 1);
            scan_column.push(millis(start));
        }

        // The image drawn again, as when the player steps onto another block
        let mut drawn = serde_json::Map::new();
        for zoom in ZOOMS {
            let mut samples = Vec::new();
            for _ in 0..REMESHES {
                let start = Instant::now();
                compose(
                    &surface,
                    &colors,
                    IVec2::new(scene.center.0, scene.center.1),
                    zoom,
                    &mut pixels,
                );
                samples.push(millis(start));
                black_box(&pixels);
            }
            drawn.insert(format!("zoom_{zoom}"), summary(samples));
        }

        // The whole surface drawn for the world map, as while it is open
        let (min, max) = surface.bounds().unwrap();
        let size = (max - min + IVec2::ONE) * CHUNK_SIZE;
        let mut world_pixels = vec![0; (size.x * size.y * 4) as usize];
        let mut world_map = Vec::new();
        for _ in 0..REMESHES {
            let start = Instant::now();
            compose_world(
                &surface,
                &colors,
                min * CHUNK_SIZE,
                size.x as u32,
                &mut world_pixels,
            );
            world_map.push(millis(start));
            black_box(&world_pixels);
        }
        scenes.push(json!({
            "scene": scene.name,
            "columns": columns,
            "scan_all_ms": summary(scan_all),
            "scan_one_column_ms": summary(scan_column),
            "compose_ms": drawn,
            "world_map_px": [size.x, size.y],
            "compose_world_ms": summary(world_map),
        }));
    }
    let results = json!({ "scenes": scenes });
    println!("{}", serde_json::to_string_pretty(&results).unwrap());
    write_results("minimap", results);
}
