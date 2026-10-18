//! Pictures of the client: golden comparisons, and snapshots for a human to review.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy::{
    camera::RenderTarget,
    picking::PickingSettings,
    prelude::*,
    render::{
        render_resource::Extent3d,
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
    window::{PrimaryWindow, WindowRef, WindowResized},
};
use image::RgbImage;

use super::harness::{Harness, SCREEN};
use crate::ui::golden::assert_image_matches;

/// The frames that capturing the screen may take.
const MAX_FRAMES: usize = 500;

/// Where the snapshot `name` of `test` is saved. The module path of the test, whose `::` are not
/// welcome in a file name, is the folder.
fn snapshot_path(test: &str, name: &str, width: u32, height: u32) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/ui-snapshots")
        .join(test.replace("::", "__"))
        .join(format!("{name}_{width}x{height}.png"))
}

impl Harness {
    /// The image of the screen, drawn off screen. The hover is kept as it is meanwhile: the
    /// cursor is on the window, which the cameras no longer draw into, so the picking would find
    /// nothing under it and the tooltip and the hovered slot would not be in the picture.
    pub fn screenshot(&mut self) -> RgbImage {
        let hover = self.world().resource::<PickingSettings>().is_hover_enabled;
        self.world_mut()
            .resource_mut::<PickingSettings>()
            .is_hover_enabled = false;
        self.set_camera_targets(RenderTarget::Image(self.target.clone().into()));
        self.frames(1);
        let shot = Arc::new(Mutex::new(None));
        let captured = shot.clone();
        let target = self.target.clone();
        self.world_mut().spawn(Screenshot::image(target)).observe(
            move |screenshot: On<ScreenshotCaptured>| {
                *captured.lock().unwrap() = Some(screenshot.image.clone());
            },
        );
        let mut image = None;
        for _ in 0..MAX_FRAMES {
            self.frames(1);
            image = shot.lock().unwrap().take();
            if image.is_some() {
                break;
            }
        }
        self.set_camera_targets(RenderTarget::Window(WindowRef::Primary));
        self.frames(1);
        self.world_mut()
            .resource_mut::<PickingSettings>()
            .is_hover_enabled = hover;
        image
            .expect("the screenshot was never captured")
            .try_into_dynamic()
            .unwrap()
            .to_rgb8()
    }

    /// Runs frames until every image of the UI is loaded: the buttons are only borders before.
    /// Meant for the menus: in game, the atlas built at runtime never counts as loaded.
    #[track_caller]
    pub fn wait_for_ui_images(&mut self) {
        self.wait_until("the images of the UI", SCREEN, |w| {
            let server = w.resource::<AssetServer>();
            let mut nodes = w.try_query::<&ImageNode>().unwrap();
            nodes
                .iter(w)
                .all(|node| server.is_loaded_with_dependencies(&node.image))
        });
    }

    /// Checks the menu screen against the golden `name`, once its images are loaded.
    #[track_caller]
    pub fn assert_screenshot(&mut self, name: &str) {
        self.wait_for_ui_images();
        let shot = self.screenshot();
        assert_image_matches(name, &shot);
    }

    /// Saves the screen as `target/ui-snapshots/<test>/<name>_<width>x<height>.png` for a human to
    /// review, when `UI_SNAPSHOTS=1`. Each test has its own folder, as tests run in parallel.
    pub fn snapshot(&mut self, name: &str) {
        if !std::env::var_os("UI_SNAPSHOTS").is_some_and(|value| value == "1") {
            return;
        }
        let shot = self.screenshot();
        let thread = std::thread::current();
        let path = snapshot_path(
            thread.name().unwrap_or("unnamed"),
            name,
            shot.width(),
            shot.height(),
        );
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        shot.save(path).unwrap();
    }

    /// Resizes the window and the screenshots, and lays the UI out again. The cameras only read the
    /// new size of a window from a `WindowResized` message, so one is sent.
    pub fn resize(&mut self, width: u32, height: u32) {
        let world = self.world_mut();
        let (entity, mut window) = world
            .query_filtered::<(Entity, &mut Window), With<PrimaryWindow>>()
            .single_mut(world)
            .unwrap();
        window.resolution.set_physical_resolution(width, height);
        // The cameras only read the new size of a window from this event
        world.write_message(WindowResized {
            window: entity,
            width: width as f32,
            height: height as f32,
        });
        let target = self.target.clone();
        self.world_mut()
            .resource_mut::<Assets<Image>>()
            .get_mut(&target)
            .unwrap()
            .resize(Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            });
        self.frames(1);
    }

    fn set_camera_targets(&mut self, target: RenderTarget) {
        let world = self.world_mut();
        let mut cameras = world.query::<&mut RenderTarget>();
        for mut camera in cameras.iter_mut(world) {
            *camera = target.clone();
        }
    }
}

#[cfg(test)]
mod tests {

    use super::snapshot_path;
    use crate::e2e::harness::{Harness, SCREEN};
    use crate::e2e::locator::Locator as L;
    use crate::ui::button::MenuState;

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn the_main_menu_matches_its_golden() {
        let mut h = Harness::new();
        h.wait_for(&L::text("Multiplayer"), SCREEN);
        h.assert_screenshot("menu_main");
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_click_after_a_screenshot_still_reaches_its_button() {
        let mut h = Harness::new();
        h.wait_for(&L::text("Multiplayer"), SCREEN);
        h.screenshot();
        h.click(&L::text("Multiplayer"));
        assert_eq!(h.menu_state(), MenuState::Multi);
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn resizing_lays_the_menu_out_again() {
        let mut h = Harness::new();
        h.wait_for(&L::text("Quit game"), SCREEN);
        h.resize(800, 600);
        let quit = h.center_of(&L::text("Quit game"));
        // in the column of buttons on the left, inside the window
        assert!((0.0..400.0).contains(&quit.x), "Quit game is at {quit}");
        assert!((0.0..600.0).contains(&quit.y), "Quit game is at {quit}");
        assert_eq!(h.screenshot().dimensions(), (800, 600));
    }

    #[test]
    fn a_snapshot_is_saved_in_the_folder_of_its_test() {
        let path = snapshot_path("e2e::menu::a_test", "main", 800, 600);
        assert!(path.ends_with("ui-snapshots/e2e__menu__a_test/main_800x600.png"));
    }
}
