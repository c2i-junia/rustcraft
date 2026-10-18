//! The client running headless: the real app, on a window with no surface, drawn by lavapipe
//! (see `ui/golden.rs`). The tests own the update loop.

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::{
    asset::{RenderAssetUsages, UnapprovedPathMode},
    gilrs::GilrsPlugin,
    log::LogPlugin,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        settings::{InstanceFlags, RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    },
    window::{ExitCondition, WindowResolution},
    winit::WinitPlugin,
};
use shared::GameFolderPaths;

use super::seed::GameFolder;
use crate::app::{add_client, ClientSettings};
use crate::constants::TEXTURE_PATH_BASE;
use crate::state::GameState;
use crate::ui::button::MenuState;

/// How long a screen may take to appear.
pub const SCREEN: Duration = Duration::from_secs(2);
/// How long a journey through a server may take.
pub const SERVER: Duration = Duration::from_secs(30);

/// How the client is started.
pub struct Options {
    pub width: u32,
    pub height: u32,
    pub player_name: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            player_name: "Tester".to_string(),
        }
    }
}

/// A running client, and the game folder it uses.
pub struct Harness {
    pub(super) app: App,
    folder: GameFolder,
    /// The image the screenshots are taken from.
    pub(super) target: Handle<Image>,
    /// Dropped last, so that it also covers the shutdown of the app.
    _watchdog: Watchdog,
}

/// How long a harness may live before the watchdog aborts the process.
const LIFETIME: Duration = Duration::from_secs(300);

/// Aborts the process, saying which test it was, if its harness lives longer than [`LIFETIME`].
/// A frame stuck in the renderer cannot be timed out from the test thread, and would otherwise
/// block the whole run.
struct Watchdog(std::sync::mpsc::Sender<()>);

impl Watchdog {
    fn start() -> Self {
        let test = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let (done, wait) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            if wait.recv_timeout(LIFETIME) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                eprintln!("the harness of {test} is stuck after {LIFETIME:?}: aborting");
                std::process::abort();
            }
        });
        Self(done)
    }
}

impl Harness {
    /// A client with an empty game folder and the default options.
    pub fn new() -> Self {
        Self::start(GameFolder::new(), Options::default())
    }

    pub fn start(folder: GameFolder, options: Options) -> Self {
        let Options {
            width,
            height,
            player_name,
        } = options;
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
                        features: WgpuFeatures::POLYGON_MODE_LINE,
                        // The Vulkan validation layer deadlocks the process when it exits
                        // while render threads still run
                        instance_flags: InstanceFlags::empty(),
                        ..default()
                    })),
                    // Every pipeline is ready for the first frame
                    synchronous_pipeline_compilation: true,
                    ..default()
                })
                // Without winit the window has no surface, but it still gives the UI a cursor
                // and a resolution
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        resolution: WindowResolution::new(width, height),
                        ..default()
                    }),
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>()
                .disable::<GilrsPlugin>()
                .disable::<LogPlugin>(),
        );

        let mut image = Image::new_fill(
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            &[0, 0, 0, 255],
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
        let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);

        // The UI picks a camera by itself only while the cameras render to a window
        app.add_systems(
            PostUpdate,
            |mut commands: Commands, cameras: Query<Entity, Added<Camera>>| {
                for camera in &cameras {
                    commands.entity(camera).insert(IsDefaultUiCamera);
                }
            },
        );
        add_client(
            &mut app,
            ClientSettings {
                game_folder_paths: GameFolderPaths {
                    game_folder_path: folder.path().to_path_buf(),
                    assets_folder_path: data,
                },
                texture_path: TEXTURE_PATH_BASE.to_string(),
                player_name,
            },
        );
        app.finish();
        app.cleanup();
        Self {
            app,
            folder,
            target,
            _watchdog: Watchdog::start(),
        }
    }

    /// Runs `n` frames.
    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// Runs frames until `condition` holds, and panics if it takes longer than `timeout`.
    pub fn wait_until(
        &mut self,
        what: &str,
        timeout: Duration,
        mut condition: impl FnMut(&World) -> bool,
    ) {
        let start = Instant::now();
        while !condition(self.world()) {
            assert!(
                start.elapsed() < timeout,
                "timed out after {timeout:?} waiting for {what}"
            );
            self.frames(1);
        }
    }

    pub fn world(&self) -> &World {
        self.app.world()
    }

    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    pub fn game_state(&self) -> GameState {
        *self.world().resource::<State<GameState>>().get()
    }

    pub fn menu_state(&self) -> MenuState {
        *self.world().resource::<State<MenuState>>().get()
    }

    /// The game folder of the client.
    pub fn folder(&self) -> &Path {
        self.folder.path()
    }

    /// Whether the app asked to exit.
    pub fn has_exited(&self) -> bool {
        self.app.should_exit().is_some()
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_client_starts_on_the_main_menu() {
    let mut h = Harness::new();
    h.wait_until("the main menu", SCREEN, |w| {
        *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    assert_eq!(h.game_state(), GameState::Menu);
    assert!(h.folder().join("keybindings.ron").exists());
}
