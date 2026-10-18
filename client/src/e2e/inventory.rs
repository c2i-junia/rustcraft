//! The inventory panel in the game: where it opens, the stack held by the cursor, and the clicks.

use bevy::{input::keyboard::KeyCode, picking::hover::HoverMap, prelude::*, ui::UiGlobalTransform};
use image::RgbImage;
use shared::world::{grid_slot, ItemId};

use super::game::{enter_world, enter_world_with, floating, slot};
use super::harness::{Harness, Options, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::HINT_BACKPACK;
use crate::ui::hud::{
    CraftResultCell, FloatingStack, InventoryCell, InventoryDialog, InventoryRoot,
};
use crate::ui::widgets::slot::SlotIcon;
use crate::ui::widgets::tooltip::TooltipRoot;

/// How much a channel of a hotbar pixel may change when the inventory opens.
const CHANNEL_TOLERANCE: u8 = 16;

impl Harness {
    /// The rectangle of the element, in physical pixels.
    #[track_caller]
    fn rect_of(&mut self, locator: &L) -> Rect {
        let entity = self.get(locator);
        rect_of(self.world(), entity)
    }

    /// The rectangles of the visible elements the locator finds, in physical pixels.
    fn rects_of(&mut self, locator: &L) -> Vec<Rect> {
        let found = self.find(locator);
        found
            .into_iter()
            .map(|entity| rect_of(self.world(), entity))
            .collect()
    }

    /// The rectangles of the nine cells of the hotbar, by slot.
    fn hotbar_rects(&mut self) -> Vec<Rect> {
        let cells = self.find(&L::marker::<InventoryCell>().within(L::marker::<Hotbar>()));
        let mut cells: Vec<(u32, Rect)> = cells
            .into_iter()
            .map(|cell| {
                let world = self.world();
                (
                    world.get::<InventoryCell>(cell).unwrap().id,
                    rect_of(world, cell),
                )
            })
            .collect();
        cells.sort_by_key(|(id, _)| *id);
        assert_eq!(cells.len(), 9, "the hotbar shows nine cells");
        cells.into_iter().map(|(_, rect)| rect).collect()
    }
}

fn rect_of(world: &World, entity: Entity) -> Rect {
    let center = world.get::<UiGlobalTransform>(entity).unwrap().translation;
    let size = world.get::<ComputedNode>(entity).unwrap().size();
    Rect::from_center_size(center, size)
}

/// Asserts that every pixel inside `rect`, 2 px in from its edges, is about the same in both
/// images.
#[track_caller]
fn assert_same_pixels(before: &RgbImage, after: &RgbImage, rect: Rect) {
    let inner = rect.inflate(-2.0);
    for y in inner.min.y as u32..inner.max.y as u32 {
        for x in inner.min.x as u32..inner.max.x as u32 {
            let (b, a) = (before.get_pixel(x, y), after.get_pixel(x, y));
            let alike =
                b.0.iter()
                    .zip(a.0)
                    .all(|(b, a)| b.abs_diff(a) <= CHANNEL_TOLERANCE);
            assert!(alike, "the pixel ({x}, {y}) went from {b:?} to {a:?}");
        }
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_open_inventory_leaves_the_hotbar_alone_and_fits_the_window() {
    // The cobblestone left beyond the 2x2 grid gives the crafting row its 3x3 height, at the
    // first and smallest size: closing the inventory then empties the grid
    let folder = GameFolder::new().with_world("Panel").with_items(
        "Panel",
        "Tester",
        &[
            (0, ItemId::Dirt, 64),
            (10, ItemId::Sand, 33),
            (grid_slot(2, 2), ItemId::Cobblestone, 1),
        ],
    );
    let mut h = enter_world(folder, "Panel");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Sand, 33))
    });

    for (width, height) in [(800, 600), (1280, 720), (1366, 768)] {
        h.resize(width, height);
        let size = format!("{width}x{height}");

        // Closed, the room of the inventory above the hotbar takes no hover
        let closed = h.rect_of(&L::marker::<InventoryDialog>());
        let hotbar_top = h.hotbar_rects()[0].min.y;
        h.hover_at(Vec2::new(
            closed.center().x,
            (closed.min.y + hotbar_top) / 2.0,
        ));
        h.frames(1);
        let dialog = h.get(&L::marker::<InventoryDialog>());
        let hovered = h
            .world()
            .resource::<HoverMap>()
            .values()
            .any(|hits| hits.contains_key(&dialog));
        assert!(!hovered, "the closed panel takes the hover at {size}");

        // Away from every cell, so that none of them is hovered
        h.hover_at(Vec2::new(width as f32 - 1.0, 0.0));
        h.frames(2);
        let before = h.hotbar_rects();
        let before_shot = h.screenshot();

        h.press(KeyCode::KeyE);
        h.frames(2);
        assert!(h.is_visible(&L::marker::<InventoryRoot>()));

        // Neither moved, nor covered, nor dimmed
        let hotbar = h.hotbar_rects();
        assert_eq!(hotbar, before, "the hotbar moved at {size}");
        let after_shot = h.screenshot();
        for &rect in &hotbar {
            assert_same_pixels(&before_shot, &after_shot, rect);
        }

        let window = Rect::new(0.0, 0.0, width as f32, height as f32);
        let panel = h.rect_of(&L::marker::<InventoryDialog>());
        let mut cells = h.rects_of(&L::marker::<InventoryCell>());
        cells.extend(h.rects_of(&L::marker::<CraftResultCell>()));
        for rect in cells.iter().chain([&panel]) {
            assert!(
                window.contains(rect.min) && window.contains(rect.max),
                "{rect:?} leaves the window at {size}"
            );
        }
        // Everything else lies above the hotbar
        let hotbar_top = hotbar[0].min.y;
        let above = cells.iter().filter(|rect| !hotbar.contains(rect));
        // The backpack, the 2x2 grid with the leftover cell at the first size, and the result
        let grid = if (width, height) == (800, 600) { 5 } else { 4 };
        assert_eq!(
            above.clone().count(),
            27 + grid + 1,
            "the cells shown at {size}"
        );
        for rect in above {
            assert!(
                rect.max.y <= hotbar_top,
                "{rect:?} reaches the hotbar, at {hotbar_top}, at {size}"
            );
        }

        h.press(KeyCode::KeyE);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_held_stack_follows_the_cursor_and_stray_clicks_do_nothing() {
    let folder = GameFolder::new().with_world("Held").with_items(
        "Held",
        "Tester",
        &[(0, ItemId::Stick, 3), (10, ItemId::Dirt, 5)],
    );
    // A UiScale of 1.5
    let options = Options {
        width: 1920,
        height: 1080,
        ..default()
    };
    let mut h = enter_world_with(folder, "Held", options);
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 5))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    h.click(&L::cell(10));
    h.wait_until("the dirt held", SERVER, |w| {
        floating(w) == Some((ItemId::Dirt, 5))
    });

    // Centred on the cursor, at the size of a slot
    h.hover(&L::cell(20));
    let stack = h.rect_of(&L::marker::<FloatingStack>());
    let cell = h.rect_of(&L::cell(20));
    assert!(
        (stack.center() - cell.center()).abs().max_element() <= 1.0,
        "the held stack {stack:?} is not centred on the cursor, over {cell:?}"
    );
    assert_eq!(stack.size(), cell.size());
    // The icons are 32 logical px, so 48 physical px at this scale
    for scope in [L::marker::<FloatingStack>(), L::cell(0)] {
        let icon = h.rect_of(&L::marker::<SlotIcon>().within(scope));
        assert_eq!(icon.size(), Vec2::splat(48.0));
    }

    // Between the backpack and the hotbar, on the panel but on no slot
    let (above, below) = (h.rect_of(&L::cell(27)), h.rect_of(&L::cell(0)));
    h.click_at(Vec2::new(
        above.center().x,
        (above.max.y + below.min.y) / 2.0,
    ));
    h.frames(10);
    assert_eq!(floating(h.world()), Some((ItemId::Dirt, 5)));
    assert_eq!(slot(h.world(), 0), Some((ItemId::Stick, 3)));
    assert_eq!(slot(h.world(), 27), None);

    // Outside the panel: nothing is dropped, and the inventory stays open
    h.click_at(Vec2::new(10.0, 10.0));
    h.frames(10);
    assert_eq!(floating(h.world()), Some((ItemId::Dirt, 5)));
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));

    // The click goes through the held stack to the slot under it
    h.click(&L::cell(20));
    h.wait_until("the dirt in slot 20", SERVER, |w| {
        slot(w, 20) == Some((ItemId::Dirt, 5)) && floating(w).is_none()
    });
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn shift_click_number_keys_and_tooltips_in_the_open_inventory() {
    let folder = GameFolder::new().with_world("Moves").with_items(
        "Moves",
        "Tester",
        &[(2, ItemId::Stick, 3), (10, ItemId::Dirt, 5)],
    );
    let mut h = enter_world(folder, "Moves");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 5))
    });
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);

    // The tooltip names the item, its count and what a click does
    h.hover(&L::cell(10));
    let tooltip = L::marker::<TooltipRoot>;
    for text in ["Dirt", "5 items", HINT_BACKPACK] {
        h.wait_for(&L::text(text).within(tooltip()), SCREEN);
    }

    // Shift+click moves a stack from the backpack to the hotbar, and back
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::cell(10));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the dirt in the hotbar", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Dirt, 5)) && slot(w, 10).is_none()
    });
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::cell(0));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the dirt in the backpack", SERVER, |w| {
        slot(w, 9) == Some((ItemId::Dirt, 5))
    });

    // A number key over a slot swaps it with that hotbar slot
    h.hover(&L::cell(9));
    h.press(KeyCode::Digit3);
    h.wait_until("the swap", SERVER, |w| {
        slot(w, 2) == Some((ItemId::Dirt, 5)) && slot(w, 9) == Some((ItemId::Stick, 3))
    });

    // Closing the inventory hides the tooltip
    h.press(KeyCode::KeyE);
    h.frames(3);
    assert!(!h.is_visible(&tooltip()));
}
