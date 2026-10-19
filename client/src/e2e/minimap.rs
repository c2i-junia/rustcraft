//! The minimap in the game: drawn from the world around the player at the top-right, above the
//! FPS counter and the toasts, hidden by its key and under the pause menu, and zoomed.

use std::fs;

use bevy::{ecs::system::RunSystemOnce, prelude::*, window::PrimaryWindow};

use super::game::enter_world;
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::constants::INTERFACE_SETTINGS_PATH;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::chat::ChatRoot;
use crate::ui::hud::fps_counter::FpsCounter;
use crate::ui::hud::minimap::compose::{MAP_SIZE, UNLOADED};
use crate::ui::hud::minimap::{column_of, MinimapCoords, MinimapImage, MinimapRoot};
use crate::ui::settings::{InterfaceSettings, MinimapChoice};
use crate::ui::widgets::toast::{show_toast, ToastKind};

/// The colour of the pixel at the centre of the map, under the player.
fn center_pixel(w: &World) -> [u8; 3] {
    let image = w.resource::<MinimapImage>();
    let data = w
        .resource::<Assets<Image>>()
        .get(&image.handle)
        .and_then(|image| image.data.clone())
        .unwrap_or_default();
    let index = ((MAP_SIZE / 2) * MAP_SIZE + MAP_SIZE / 2) as usize * 4;
    data.get(index..index + 3)
        .map(|pixel| [pixel[0], pixel[1], pixel[2]])
        .unwrap_or(UNLOADED)
}

fn saved_minimap(h: &Harness) -> MinimapChoice {
    let text = fs::read_to_string(h.folder().join(INTERFACE_SETTINGS_PATH)).expect("a saved file");
    ron::from_str::<InterfaceSettings>(&text)
        .expect("a file in RON")
        .minimap
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_minimap_draws_the_world_top_right_and_its_key_hides_it() {
    let mut h = enter_world(GameFolder::new().with_world("Map"), "Map");
    let minimap = L::marker::<MinimapRoot>();
    h.wait_for(&minimap, SCREEN);
    h.wait_until("the ground under the player on the map", SERVER, |w| {
        center_pixel(w) != UNLOADED
    });

    // at the top-right of the window, the coordinates of the player under the map
    let window = h
        .world_mut()
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap()
        .physical_width() as f32;
    let map = h.rect_of(&minimap);
    assert!(map.max.x > window * 0.97, "{map:?} in {window} px");
    assert!(map.min.y < 20.0, "{map:?}");
    let player = h
        .world_mut()
        .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(h.world())
        .unwrap()
        .translation;
    let coords = h.text_of(&L::marker::<MinimapCoords>());
    let column = column_of(player);
    assert!(
        coords.starts_with(&format!("X {}  Y ", column.x))
            && coords.ends_with(&format!("Z {}", column.y)),
        "{coords} for {player}"
    );

    // the FPS counter goes under the map, and a toast under both
    h.press(KeyCode::F8);
    h.frames(2);
    let counter = h.rect_of(&L::marker::<FpsCounter>());
    assert!(counter.min.y > map.max.y, "{counter:?} under {map:?}");
    let font = h
        .world()
        .resource::<AssetServer>()
        .load(crate::ui::assets::FONT_PATH);
    h.world_mut()
        .run_system_once(move |mut commands: Commands| {
            show_toast(&mut commands, "World saved", ToastKind::Info, font.clone());
        })
        .unwrap();
    h.frames(3);
    let toast = h.rect_of(&L::text("World saved"));
    assert!(toast.min.y > counter.max.y, "{toast:?} under {counter:?}");

    // the zoom keys go from one block a pixel to four, and no further
    let zoom = |h: &mut Harness| h.world().resource::<MinimapImage>().blocks_per_pixel();
    for expected in [2, 4, 4] {
        h.press(KeyCode::Minus);
        assert_eq!(zoom(&mut h), expected);
    }
    for expected in [2, 1, 1] {
        h.press(KeyCode::Equal);
        assert_eq!(zoom(&mut h), expected);
    }

    // hidden under the pause menu, keeping its room: the FPS counter does not move
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&minimap));
    assert_eq!(h.rect_of(&L::marker::<FpsCounter>()), counter);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(h.is_visible(&minimap));

    // its key is a letter of the chat while the player types
    h.press(KeyCode::KeyT);
    h.wait_for(&L::marker::<ChatRoot>(), SCREEN);
    h.press(KeyCode::KeyM);
    h.frames(2);
    assert!(h.is_visible(&minimap));
    h.press(KeyCode::Escape);
    h.frames(2);

    // its key hides it, the FPS counter takes its place, and the choice is saved
    h.press(KeyCode::KeyM);
    h.frames(2);
    assert!(!h.is_visible(&minimap));
    assert_eq!(saved_minimap(&h), MinimapChoice::Off);
    let counter = h.rect_of(&L::marker::<FpsCounter>());
    assert!(counter.min.y < map.min.y + 1.0, "{counter:?}");
    h.press(KeyCode::KeyM);
    h.frames(2);
    assert!(h.is_visible(&minimap));
    assert_eq!(saved_minimap(&h), MinimapChoice::On);
}
