//! The minimap in the game: drawn from the world around the player at the top-right, above the
//! FPS counter and the toasts, hidden under the pause menu, and zoomed. Its key opens the world
//! map over the whole window, which drags, zooms and closes.

use bevy::{ecs::system::RunSystemOnce, prelude::*, window::PrimaryWindow};

use super::game::enter_world;
use super::harness::{Harness, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::chat::ChatRoot;
use crate::ui::hud::fps_counter::FpsCounter;
use crate::ui::hud::minimap::compose::{MAP_SIZE, UNLOADED};
use crate::ui::hud::minimap::world_map::{
    WorldMap, WorldMapArrow, WorldMapRoot, WorldMapView, SCALES,
};
use crate::ui::hud::minimap::{column_of, MinimapCoords, MinimapImage, MinimapRoot};
use crate::ui::menus::pause::PauseMenu;
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_minimap_draws_the_world_top_right_and_its_key_opens_the_world_map() {
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
    let world_map = L::marker::<WorldMapRoot>();
    h.press(KeyCode::KeyT);
    h.wait_for(&L::marker::<ChatRoot>(), SCREEN);
    h.press(KeyCode::KeyM);
    h.frames(2);
    assert!(!h.is_visible(&world_map));
    h.press(KeyCode::Escape);
    h.frames(2);

    // its key opens the world map over the whole window, the HUD hidden under it
    h.press(KeyCode::KeyM);
    h.frames(2);
    let screen = h.rect_of(&world_map);
    assert!(
        screen.min == Vec2::ZERO && screen.max.x >= window - 1.0,
        "{screen:?}"
    );
    assert!(!h.is_visible(&minimap));
    h.wait_until("the world drawn on the map", SERVER, |w| {
        let map = w.resource::<WorldMap>();
        let column = column_of(player) - map.corner;
        column.cmpge(IVec2::ZERO).all() && column.cmplt(map.size.as_ivec2()).all()
    });
    // the arrow of the player at the centre of the view, across
    h.frames(2);
    let view = h.rect_of(&L::marker::<WorldMapView>());
    let arrow = h.rect_of(&L::marker::<WorldMapArrow>());
    assert!(
        (arrow.center() - view.center()).abs().max_element() < 2.0,
        "{arrow:?} in {view:?}"
    );

    // the wheel zooms around the cursor, which is at the centre: the map still follows
    let scale = |h: &mut Harness| h.world().resource::<WorldMap>().scale;
    let first = scale(&mut h);
    h.hover_at(view.center());
    h.scroll(1.0);
    assert_eq!(scale(&mut h), first + 1);
    h.press(KeyCode::Minus);
    assert_eq!(scale(&mut h), first);
    for _ in 0..SCALES.len() {
        h.scroll(-1.0);
    }
    assert_eq!(scale(&mut h), 0);
    assert_eq!(h.world().resource::<WorldMap>().focus, None);

    // a drag moves the map: the arrow with it; Space centres it on the player again
    let focus = |h: &mut Harness| h.world().resource::<WorldMap>().focus;
    h.mouse_down(&L::marker::<WorldMapView>());
    h.hover_at(view.center() + Vec2::new(40.0, 0.0));
    h.mouse_up();
    h.frames(2);
    let dragged = focus(&mut h).expect("a map dragged away from the player");
    assert!(dragged.x < player.x - 1.0, "{dragged} for {player}");
    let moved = h.rect_of(&L::marker::<WorldMapArrow>());
    assert!(moved.center().x > arrow.center().x + 20.0, "{moved:?}");
    h.press(KeyCode::Space);
    assert_eq!(focus(&mut h), None);

    // its key closes it, as Escape does, without opening the pause menu
    h.press(KeyCode::KeyM);
    h.frames(2);
    assert!(!h.is_visible(&world_map));
    assert!(h.is_visible(&minimap));
    h.press(KeyCode::KeyM);
    h.frames(2);
    assert!(h.is_visible(&world_map));
    h.press(KeyCode::Escape);
    h.frames(2);
    assert!(!h.is_visible(&world_map));
    assert!(!h.is_visible(&L::marker::<PauseMenu>()));
    assert!(h.is_visible(&minimap));
}
