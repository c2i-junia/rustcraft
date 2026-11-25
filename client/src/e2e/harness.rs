//! The client running headless: the real app, on a window with no surface, drawn by lavapipe
//! (see `ui/golden.rs`). The tests own the update loop.

use std::io::Write;
use std::path::Path;
use std::sync::mpsc::{RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use bevy::{
    asset::{RenderAssetUsages, UnapprovedPathMode},
    audio::{AudioPlugin, GlobalVolume, Volume},
    camera::RenderTarget,
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
use crate::world::{GraphicsSettings, ShaderQuality};

/// How long a screen may take to appear.
pub const SCREEN: Duration = Duration::from_secs(2);
/// How long the first screen may take to appear: a client starting while the machine is busy,
/// with other tests or a build, is slow to load its first assets.
pub const STARTUP: Duration = Duration::from_secs(10);
/// How long a journey through a server may take.
pub const SERVER: Duration = Duration::from_secs(90);

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
    /// `None` starts as on a first launch: the saved name, else none.
    pub player_name: Option<String>,
    /// The shaders the client draws with, whatever its game folder says; `None` keeps the
    /// saved ones. Off by default: lavapipe draws the others slowly.
    pub shaders: Option<ShaderQuality>,
    /// Whether the render world counts the views the pass of the bolts draws, into
    /// `LIGHTNING_VIEWS_DRAWN`: for a single test, which reads it alone.
    pub count_bolt_views: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            player_name: Some("Tester".to_string()),
            shaders: Some(ShaderQuality::Off),
            count_bolt_views: false,
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

/// How long a frame of a harness may take before the watchdog aborts the process:
/// `E2E_FRAME_SECS`, or 300 seconds.
fn longest_frame() -> Duration {
    std::env::var("E2E_FRAME_SECS")
        .ok()
        .and_then(|secs| secs.parse().ok())
        .map_or(Duration::from_secs(300), Duration::from_secs)
}

/// Aborts the process, saying which test it was, if its harness starts no frame for
/// [`longest_frame`]. A frame stuck in the renderer cannot be timed out from the test thread, and
/// would otherwise block the whole run. A test that is only slow, on the few cores of a CI
/// machine, is left to its own timeouts.
struct Watchdog(
    // Only held for its drop
    #[allow(dead_code)] Stop,
);

/// What a watchdog is told.
enum Sign {
    Frame,
    Stop,
}

/// Tells the watchdog of the client that a frame starts.
#[derive(Resource)]
struct Frames(Sender<Sign>);

fn sign_frame(frames: Res<Frames>) {
    let _ = frames.0.send(Sign::Frame);
}

thread_local! {
    /// The watchdogs started by the test of this thread. They stop when the thread ends, even
    /// when the test failed: the client is built with Cranelift, which runs no destructor while
    /// a panic unwinds, so the drop of a failed test's harness never comes.
    static WATCHDOGS: std::cell::RefCell<Vec<Stop>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Stops a watchdog when dropped.
struct Stop(Sender<Sign>);

impl Drop for Stop {
    fn drop(&mut self) {
        // The watchdog may be gone already
        let _ = self.0.send(Sign::Stop);
    }
}

impl Watchdog {
    fn start() -> Self {
        let test = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let (signs, wait) = std::sync::mpsc::channel();
        WATCHDOGS.with(|watchdogs| watchdogs.borrow_mut().push(Stop(signs.clone())));
        std::thread::spawn(move || loop {
            match wait.recv_timeout(longest_frame()) {
                Ok(Sign::Frame) => continue,
                Ok(Sign::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Err(RecvTimeoutError::Timeout) => {
                    // Not `eprintln`, which the test captures: the abort would lose it
                    let _ = writeln!(
                        std::io::stderr(),
                        "the harness of {test} is stuck after {:?} in a frame: aborting",
                        longest_frame()
                    );
                    std::process::abort();
                }
            }
        });
        // Dropped with the harness, it stops the watchdog before the thread ends
        Self(Stop(signs))
    }

    fn frames(&self) -> Frames {
        Frames(self.0 .0.clone())
    }
}

/// How many tests may run clients at once: half the cores. Each client renders with lavapipe on
/// threads of its own: with more, the clients take the cores from one another and the whole run
/// is slower (on 32 threads, 60 s at 16 tests at once, 78 s at 32, 78 s at 8).
fn max_clients() -> usize {
    std::thread::available_parallelism().map_or(1, |cores| (cores.get() / 2).max(1))
}

/// The tests running clients now.
static CLIENT_TESTS: std::sync::Mutex<usize> = std::sync::Mutex::new(0);
/// Told when a test stops running clients.
static CLIENT_TEST_ENDED: std::sync::Condvar = std::sync::Condvar::new();

/// Held while a client makes its renderer, one client at a time (`Harness::start`).
static RENDERER_CREATION: std::sync::Mutex<()> = std::sync::Mutex::new(());

thread_local! {
    /// Whether the test of this thread may run clients. Held until the thread ends, as the
    /// watchdogs: a test with several clients takes one place, and a failed test gives it back.
    static CLIENT_PLACE: std::cell::RefCell<Option<ClientPlace>> =
        const { std::cell::RefCell::new(None) };
}

/// A place among the tests running clients, given back when dropped.
struct ClientPlace;

impl ClientPlace {
    /// Waits for a place for the test of this thread, unless it has one.
    fn take() {
        CLIENT_PLACE.with(|place| {
            if place.borrow().is_some() {
                return;
            }
            let max = max_clients();
            let mut running = CLIENT_TESTS.lock().unwrap();
            while *running >= max {
                running = CLIENT_TEST_ENDED.wait(running).unwrap();
            }
            *running += 1;
            *place.borrow_mut() = Some(ClientPlace);
        });
    }
}

impl Drop for ClientPlace {
    fn drop(&mut self) {
        *CLIENT_TESTS.lock().unwrap() -= 1;
        CLIENT_TEST_ENDED.notify_one();
    }
}

impl Harness {
    /// A client with an empty game folder and the default options.
    pub fn new() -> Self {
        Self::start(GameFolder::new(), Options::default())
    }

    /// A client with the game folder `folder`, once the test has a place among those running
    /// clients. The watchdog starts next, so that it also covers the start of the renderer.
    pub fn start(folder: GameFolder, options: Options) -> Self {
        ClientPlace::take();
        let watchdog = Watchdog::start();
        let Options {
            width,
            height,
            player_name,
            shaders,
            count_bolt_views,
        } = options;
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../data");
        // `BENCH_GPU` renders a benchmark on the GPU of the machine, compiling the pipelines
        // in the background as the game does
        let gpu = std::env::var_os("BENCH_GPU").is_some();
        let mut app = App::new();
        let plugins = DefaultPlugins
            .set(crate::app::task_pool_plugin())
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
                    force_fallback_adapter: !gpu,
                    features: WgpuFeatures::POLYGON_MODE_LINE,
                    // The Vulkan validation layer deadlocks the process when it exits
                    // while render threads still run
                    instance_flags: InstanceFlags::empty(),
                    ..default()
                })),
                // Every pipeline is ready for the first frame
                synchronous_pipeline_compilation: !gpu,
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
            .disable::<GilrsPlugin>();
        // The renderer's Vulkan instance is made while the plugins are added. The Vulkan loader
        // is not safe while two threads make instances at once: one enumerating the instance
        // extensions while another loads a driver crashed the test process (SIGSEGV through a
        // null pointer in `libvulkan`). The game makes one instance per process; the tests,
        // one per client, so they take turns
        let creating_the_renderer = RENDERER_CREATION
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // `BENCH_TRACE` keeps the logger, which writes the spans of a build with the
        // `bevy/trace_chrome` feature into a trace file
        app.add_plugins(match std::env::var_os("BENCH_TRACE") {
            Some(_) => plugins,
            None => plugins.disable::<LogPlugin>(),
        });
        drop(creating_the_renderer);

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

        // The UI picks a camera by itself only while the cameras render to a window. A camera
        // that renders into an image of its own, as a portal's, is not one of the screen
        app.add_systems(
            PostUpdate,
            |mut commands: Commands, cameras: Query<(Entity, &RenderTarget), Added<Camera>>| {
                for (camera, target) in &cameras {
                    if matches!(target, RenderTarget::Window(_)) {
                        commands.entity(camera).insert(IsDefaultUiCamera);
                    }
                }
            },
        );
        app.init_resource::<ExitLatch>();
        app.add_systems(Last, latch_exit);
        app.insert_resource(watchdog.frames());
        app.add_systems(First, sign_frame);
        add_client(
            &mut app,
            ClientSettings {
                game_folder_paths: GameFolderPaths {
                    game_folder_path: folder.path().to_path_buf(),
                    assets_folder_path: data,
                },
                texture_path: TEXTURE_PATH_BASE.to_string(),
                player_name,
                hub: rustcraft_protocol::DEFAULT_HUB.to_string(),
            },
        );
        if let Some(shaders) = shaders {
            app.world_mut().resource_mut::<GraphicsSettings>().shaders = shaders;
        }
        app.finish();
        // `BENCH_GPU_TIMES` times every frame on the GPU (`GPU_FRAMES_MS`), and counts the views
        // the drops are drawn in (`VIEWS_DRAWN`)
        if gpu && std::env::var_os("BENCH_GPU_TIMES").is_some() {
            let render_app = app.sub_app_mut(bevy::render::RenderApp);
            crate::world::rendering::bench::record_gpu_frames(render_app);
            render_app.add_systems(
                bevy::render::Render,
                crate::world::precipitation::render::count_views_drawn
                    .after(bevy::render::RenderSystems::PrepareBindGroups),
            );
        }
        if count_bolt_views {
            app.sub_app_mut(bevy::render::RenderApp).add_systems(
                bevy::render::Render,
                crate::world::lightning::render::count_lightning_views_drawn
                    .after(bevy::render::RenderSystems::PrepareBindGroups),
            );
        }
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
