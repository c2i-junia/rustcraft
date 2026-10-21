//! Golden screenshots of the UI. A scene is rendered off screen, with the game's textures and
//! its UI systems, then compared with the image saved for it in `client/tests/golden/`.
//!
//! - A missing golden is written, and the test fails until it is checked and committed.
//! - `UPDATE_GOLDEN=1 cargo test` rewrites the goldens of the tests it runs.
//! - On a mismatch, `<name>.actual.png` and `<name>.diff.png` (differing pixels in red) are
//!   written next to the golden.
//!
//! Every machine renders with lavapipe, the software Vulkan driver of Mesa, as GPUs sample
//! the pixel art of the icons a bit differently: it is the `vulkan-swrast` package on Arch,
//! `mesa-vulkan-drivers` on Debian and Ubuntu.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::{
    app::SubApps,
    asset::{RenderAssetUsages, UnapprovedPathMode},
    audio::AudioPlugin,
    camera::RenderTarget,
    ecs::system::{IntoSystem, RunSystemOnce},
    gilrs::GilrsPlugin,
    log::LogPlugin,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        settings::{InstanceFlags, RenderCreation, WgpuSettings},
        view::screenshot::{Screenshot, ScreenshotCaptured},
        RenderPlugin,
    },
    text::FontSource,
    window::ExitCondition,
    winit::WinitPlugin,
};
use image::{Rgb, RgbImage};
use shared::world::{BlockId, ItemId};
use shared::GameFolderPaths;

use crate::constants::TEXTURE_PATH_BASE;
use crate::state::{PreLoadingCompletion, TexturePath};
use crate::world::{
    chunk_material_plugin, create_all_atlases, setup_materials, AtlasHandles, MaterialResource,
};

/// The size of the window the goldens are drawn in.
pub const WIDTH: u32 = 1024;
pub const HEIGHT: u32 = 768;

/// A channel may differ by this much, as versions of lavapipe may blend the edges of shapes a bit
/// differently.
const CHANNEL_TOLERANCE: u8 = 16;
/// How many pixels may still differ: far fewer than a cell moved by a single pixel.
const MAX_DIFFERING_PIXELS: usize = 50;
/// The frames that loading the textures or capturing the screen may take.
const MAX_FRAMES: usize = 500;

/// A headless app holding the game's textures, which renders its UI to an image.
pub struct UiGolden {
    apps: SubApps,
    target: Handle<Image>,
}

impl UiGolden {
    /// An app with the item and block atlases built, and a UI camera rendering to an image of
    /// `WIDTH` by `HEIGHT`.
    pub fn new() -> Self {
        Self::with_size(WIDTH, HEIGHT)
    }

    /// An app with the item and block atlases built, and a UI camera rendering to an image of
    /// `width` by `height`.
    pub fn with_size(width: u32, height: u32) -> Self {
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        let mut app = App::new();
        app.add_plugins(
            DefaultPlugins
                .set(ImagePlugin::default_nearest())
                .set(AssetPlugin {
                    file_path: data.to_string_lossy().into_owned(),
                    unapproved_path_mode: UnapprovedPathMode::Allow,
                    ..default()
                })
                .set(RenderPlugin {
                    render_creation: RenderCreation::Automatic(Box::new(WgpuSettings {
                        // The software renderer, lavapipe
                        force_fallback_adapter: true,
                        // The Vulkan validation layer deadlocks the process when it exits
                        // while render threads still run
                        instance_flags: InstanceFlags::empty(),
                        ..default()
                    })),
                    // Every pipeline is ready for the first frame
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                // The test owns the update loop, and needs no sound, gamepad or log
                .disable::<WinitPlugin>()
                .disable::<AudioPlugin>()
                .disable::<GilrsPlugin>()
                .disable::<LogPlugin>(),
        )
        .add_plugins(chunk_material_plugin)
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
        });
        app.finish();
        app.cleanup();

        let mut golden = Self {
            apps: std::mem::take(app.sub_apps_mut()),
            target: Handle::default(),
        };
        golden.run_system(setup_materials);
        for _ in 0..MAX_FRAMES {
            golden.update();
            golden.run_system(create_all_atlases);
            if golden
                .world()
                .resource::<PreLoadingCompletion>()
                .textures_loaded
            {
                break;
            }
        }
        assert!(
            golden
                .world()
                .resource::<PreLoadingCompletion>()
                .textures_loaded,
            "the textures did not load"
        );

        let size = Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let mut target = Image::new_fill(
            size,
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        target.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        golden.target = golden
            .world_mut()
            .resource_mut::<Assets<Image>>()
            .add(target);
        let target = RenderTarget::Image(golden.target.clone().into());
        golden
            .world_mut()
            .spawn((Camera2d, target, IsDefaultUiCamera));
        golden
    }

    pub fn world(&self) -> &World {
        self.apps.main.world()
    }

    pub fn world_mut(&mut self) -> &mut World {
        self.apps.main.world_mut()
    }

    pub fn run_system<M>(&mut self, system: impl IntoSystem<(), (), M>) {
        self.world_mut().run_system_once(system).unwrap();
    }

    fn update(&mut self) {
        self.apps.update();
    }

    /// Runs `n` frames: one lays the UI out.
    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.update();
        }
    }

    /// Lays the UI out, draws it once its fonts and its images are loaded, and checks it against
    /// the golden `name`.
    pub fn assert_matches(&mut self, name: &str) {
        self.wait_for_fonts();
        self.wait_for_images();
        let shot = self.screenshot();
        assert_image_matches(name, &shot);
    }

    /// Runs frames until the image of every `ImageNode` is loaded, such as the sprites of the
    /// furnace: an image is not drawn before. The atlases, made at runtime, are already there.
    pub fn wait_for_images(&mut self) {
        for _ in 0..MAX_FRAMES {
            let world = self.world_mut();
            let mut nodes = world.query::<&ImageNode>();
            let images = world.resource::<Assets<Image>>();
            if nodes.iter(world).all(|node| images.contains(&node.image)) {
                return;
            }
            self.update();
        }
        panic!("the images did not load");
    }

    /// Runs frames until the font of every `TextFont` is loaded: a text is not drawn before.
    pub fn wait_for_fonts(&mut self) {
        for _ in 0..MAX_FRAMES {
            let world = self.world_mut();
            let mut texts = world.query::<&TextFont>();
            let fonts = world.resource::<Assets<Font>>();
            let loaded = texts.iter(world).all(|text| match &text.font {
                FontSource::Handle(handle) => fonts.contains(handle),
                _ => true,
            });
            if loaded {
                return;
            }
            self.update();
        }
        panic!("the fonts did not load");
    }

    /// The image drawn by the next frames.
    fn screenshot(&mut self) -> RgbImage {
        // A first frame lays the UI out, the next ones draw it
        self.update();
        let shot = Arc::new(Mutex::new(None));
        let captured = shot.clone();
        let target = self.target.clone();
        self.world_mut().spawn(Screenshot::image(target)).observe(
            move |screenshot: On<ScreenshotCaptured>| {
                *captured.lock().unwrap() = Some(screenshot.image.clone());
            },
        );
        for _ in 0..MAX_FRAMES {
            self.update();
            if let Some(image) = shot.lock().unwrap().take() {
                return image.try_into_dynamic().unwrap().to_rgb8();
            }
        }
        panic!("the screenshot was never captured");
    }
}

/// Checks `shot` against the golden `name`, and writes the golden when asked to or when it is
/// missing.
pub fn assert_image_matches(name: &str, shot: &RgbImage) {
    let golden = golden_path(name, "");
    let update = std::env::var_os("UPDATE_GOLDEN").is_some_and(|value| value == "1");
    if update || !golden.exists() {
        shot.save(&golden).unwrap();
        assert!(
            update,
            "no golden for {name}: wrote {}, check it and commit it",
            golden.display()
        );
        return;
    }

    let expected = image::open(&golden).unwrap().to_rgb8();
    let (actual, diff) = (golden_path(name, ".actual"), golden_path(name, ".diff"));
    let (diff_image, differing) = compare(&expected, shot);
    if differing <= MAX_DIFFERING_PIXELS {
        // Those of an earlier failure
        let _ = std::fs::remove_file(&actual);
        let _ = std::fs::remove_file(&diff);
        return;
    }
    shot.save(&actual).unwrap();
    diff_image.save(&diff).unwrap();
    panic!(
        "{name}: {differing} pixels differ from {}, see {} and {}",
        golden.display(),
        actual.display(),
        diff.display()
    );
}

fn golden_path(name: &str, suffix: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}{suffix}.png"))
}

/// The pixels of `actual` that differ from `expected`, in red over a faded `actual`, and their
/// count.
fn compare(expected: &RgbImage, actual: &RgbImage) -> (RgbImage, usize) {
    if expected.dimensions() != actual.dimensions() {
        return (actual.clone(), usize::MAX);
    }
    let mut diff = RgbImage::new(actual.width(), actual.height());
    let mut differing = 0;
    for (x, y, pixel) in actual.enumerate_pixels() {
        let alike = pixel
            .0
            .iter()
            .zip(expected.get_pixel(x, y).0)
            .all(|(a, e)| a.abs_diff(e) <= CHANNEL_TOLERANCE);
        let faded = Rgb(pixel.0.map(|channel| channel / 4));
        diff.put_pixel(x, y, if alike { faded } else { Rgb([255, 0, 0]) });
        differing += usize::from(!alike);
    }
    (diff, differing)
}
