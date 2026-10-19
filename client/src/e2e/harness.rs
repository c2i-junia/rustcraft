//! The client running headless: the real app, on a window with no surface, drawn by lavapipe
//! (see `ui/golden.rs`). The tests own the update loop.

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::{
    asset::{RenderAssetUsages, UnapprovedPathMode},
    audio::{AudioPlugin, GlobalVolume, Volume},
    gilrs::GilrsPlugin,
    log::LogPlugin,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat, TextureUsages},
        settings::{InstanceFlags, RenderCreation, WgpuFeatures, WgpuSettings},
        RenderPlugin,
    },
    window::{ExitCondition, PrimaryWindow, WindowCloseRequested, WindowResolution},
    winit::WinitPlugin,
};
use shared::GameFolderPaths;

use super::helpers::is_in;
use super::seed::GameFolder;
use crate::app::{add_client, ClientSettings};
use crate::constants::TEXTURE_PATH_BASE;
use crate::network::LocalServerHandle;
use crate::state::GameState;
use crate::ui::menus::MenuState;

/// How long a screen may take to appear.
pub const SCREEN: Duration = Duration::from_secs(2);
/// How long the first screen may take to appear: a client starting while the machine is busy,
/// with other tests or a build, is slow to load its first assets.
pub const STARTUP: Duration = Duration::from_secs(10);
/// How long a journey through a server may take.
pub const SERVER: Duration = Duration::from_secs(30);

/// Whether the app ever asked to exit. A message of `AppExit` lives for two frames only.
#[derive(Resource, Default)]
struct ExitLatch(bool);

fn latch_exit(mut exits: MessageReader<AppExit>, mut latch: ResMut<ExitLatch>) {
    if exits.read().next().is_some() {
        latch.0 = true;
    }
}

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
struct Watchdog(
    // Only held for its drop
    #[allow(dead_code)] Stop,
);

thread_local! {
    /// The watchdogs started by the test of this thread. They stop when the thread ends, even
    /// when the test failed: the client is built with Cranelift, which runs no destructor while
    /// a panic unwinds, so the drop of a failed test's harness never comes.
    static WATCHDOGS: std::cell::RefCell<Vec<Stop>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Stops a watchdog when dropped.
struct Stop(std::sync::mpsc::Sender<()>);

impl Drop for Stop {
    fn drop(&mut self) {
        // The watchdog may be gone already
        let _ = self.0.send(());
    }
}

impl Watchdog {
    fn start() -> Self {
        let test = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let (done, wait) = std::sync::mpsc::channel();
        WATCHDOGS.with(|watchdogs| watchdogs.borrow_mut().push(Stop(done.clone())));
        std::thread::spawn(move || {
            if wait.recv_timeout(LIFETIME) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                eprintln!("the harness of {test} is stuck after {LIFETIME:?}: aborting");
                std::process::abort();
            }
        });
        // Dropped with the harness, it stops the watchdog before the thread ends
        Self(Stop(done))
    }
}

impl Harness {
    /// A client with an empty game folder and the default options.
    pub fn new() -> Self {
        Self::start(GameFolder::new(), Options::default())
    }

    /// A client with the game folder `folder`. The watchdog starts first, so that it also covers
    /// the start of the renderer.
    pub fn start(folder: GameFolder, options: Options) -> Self {
        let watchdog = Watchdog::start();
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
                // The game still plays its sounds, but nothing reaches the speakers
                .set(AudioPlugin {
                    global_volume: GlobalVolume::new(Volume::SILENT),
                    ..default()
                })
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
                    // as the game's: the close button asks to quit
                    close_when_requested: false,
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
        app.init_resource::<ExitLatch>();
        app.add_systems(Last, latch_exit);
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
            _watchdog: watchdog,
        }
    }

    /// Runs `n` frames.
    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// Runs frames until `condition` holds, and panics, saying what it waited for, if it takes
    /// longer than `timeout`.
    #[track_caller]
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

    /// Clicks the close button of the window: asks the app to close it, and runs a frame.
    pub fn request_close(&mut self) {
        let world = self.world_mut();
        let window = world
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(world)
            .unwrap();
        world.write_message(WindowCloseRequested { window });
        self.frames(1);
    }

    /// Whether the app asked to exit at any point.
    pub fn has_exited(&self) -> bool {
        self.world().resource::<ExitLatch>().0
    }
}

/// How long a harness dropped in game waits for the integrated server to save and stop.
const LEAVE_TIMEOUT: Duration = Duration::from_secs(10);

impl Drop for Harness {
    /// Leaves the game, so that the integrated server saves and stops before the next test
    /// rather than running on in the background. Nothing is done while a test panics: running
    /// frames then could panic again, which aborts the process.
    fn drop(&mut self) {
        if std::thread::panicking() {
            return;
        }
        let in_game = |world: &World| is_in(world, GameState::Game);
        if !in_game(self.world()) {
            return;
        }
        self.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        let start = Instant::now();
        while start.elapsed() < LEAVE_TIMEOUT {
            let server_runs = self
                .world()
                .get_resource::<LocalServerHandle>()
                .is_some_and(|handle| handle.0.is_some());
            if !in_game(self.world()) && !server_runs {
                return;
            }
            self.frames(1);
        }
        eprintln!("the integrated server did not stop within {LEAVE_TIMEOUT:?}");
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_client_starts_on_the_main_menu() {
    let mut h = Harness::new();
    h.wait_until("the main menu", STARTUP, |w| {
        *w.resource::<State<MenuState>>().get() == MenuState::Main
    });
    assert_eq!(h.game_state(), GameState::Menu);
    // the settings are only written when the player changes them
    assert!(!h.folder().join("keybindings.ron").exists());
}
