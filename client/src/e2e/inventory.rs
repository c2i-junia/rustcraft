//! The inventory panel in the game: where it opens, the stack held by the cursor, the clicks and
//! the keyboard.

use bevy::{
    input::keyboard::KeyCode, input_focus::InputFocus, picking::hover::HoverMap, prelude::*,
    ui::UiGlobalTransform,
};
use image::RgbImage;
use shared::world::{grid_slot, ItemId};

use super::game::{enter_world, enter_world_with, floating, slot};
use super::harness::{Harness, Options, SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::item_browser::ItemBrowser;
use crate::ui::hud::HINT_BACKPACK;
use crate::ui::hud::{
    CraftResultCell, FloatingStack, InventoryCell, InventoryDialog, InventoryRoot, UIMode,
};
use crate::ui::theme::ACCENT;
use crate::ui::widgets::slot::SlotIcon;
use crate::ui::widgets::tooltip::TooltipRoot;

/// How much a channel of a hotbar pixel may change when the inventory opens.
const CHANNEL_TOLERANCE: u8 = 16;

impl Harness {
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

/// The slot of the cell the focus is on, if it is on a cell.
fn focused_cell(w: &World) -> Option<u32> {
    let focus = w.resource::<InputFocus>().get()?;
    w.get::<InventoryCell>(focus).map(|cell| cell.id)
}

/// Whether the focus is on the inventory panel or inside it.
fn focus_in_panel(w: &World) -> bool {
    let Some(mut entity) = w.resource::<InputFocus>().get() else {
        return false;
    };
    loop {
        if w.get::<InventoryDialog>(entity).is_some() {
            return true;
        }
        match w.get::<ChildOf>(entity) {
            Some(parent) => entity = parent.parent(),
            None => return false,
        }
    }
}

/// The slots of the cells whose focus ring shows.
fn ringed_cells(h: &mut Harness) -> Vec<u32> {
    let mut cells = h.world_mut().query::<(&InventoryCell, &Outline)>();
    cells
        .iter(h.world())
        .filter(|(_, outline)| outline.color == ACCENT)
        .map(|(cell, _)| cell.id)
        .collect()
}

fn rect_of(world: &World, entity: Entity) -> Rect {
    let center = world.get::<UiGlobalTransform>(entity).unwrap().translation;
    let size = world.get::<ComputedNode>(entity).unwrap().size();
    Rect::from_center_size(center, size)
}

/// Asserts that every pixel inside `before_rect` of `before`, 2 px in from its edges, is about the
/// same at its place in `after_rect` of `after`, a rectangle of the same size.
#[track_caller]
fn assert_same_pixels(before: &RgbImage, before_rect: Rect, after: &RgbImage, after_rect: Rect) {
    let inner = before_rect.inflate(-2.0);
    let offset = after_rect.min - before_rect.min;
    let (dx, dy) = (offset.x as i64, offset.y as i64);
    for y in inner.min.y as u32..inner.max.y as u32 {
        for x in inner.min.x as u32..inner.max.x as u32 {
            let moved = ((x as i64 + dx) as u32, (y as i64 + dy) as u32);
            let (b, a) = (before.get_pixel(x, y), after.get_pixel(moved.0, moved.1));
            let alike =
                b.0.iter()
                    .zip(a.0)
                    .all(|(b, a)| b.abs_diff(a) <= CHANNEL_TOLERANCE);
            assert!(
                alike,
                "the pixel ({x}, {y}) went from {b:?} to {a:?} at {moved:?}"
            );
        }
    }
}

/// The hotbar keeps its cells whole when the inventory opens: their size and their height, and
/// their pixels, at the place they slide to beside the item browser, or where they were when the
/// browser is a drawer.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_open_inventory_keeps_its_hotbar_whole_and_fits_beside_the_item_browser() {
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
        h.hover_at(Vec2::new(0.0, 0.0));
        h.frames(2);
        let before = h.hotbar_rects();
        let before_shot = h.screenshot();
        // Centred while closed
        let centre = (before[0].min.x + before[8].max.x) / 2.0;
        assert!((centre - width as f32 / 2.0).abs() <= 1.0, "at {size}");

        h.press(KeyCode::KeyE);
        h.frames(2);
        assert!(h.is_visible(&L::marker::<InventoryRoot>()));

        // Moved sideways only, all of it by the same amount, and neither covered nor dimmed: in a
        // drawer, the browser leaves the panel where it was
        let hotbar = h.hotbar_rects();
        let shift = hotbar[0].min.x - before[0].min.x;
        let compact = !h.is_visible(&L::marker::<ItemBrowser>());
        assert_eq!(compact, width == 800, "the browser docks at {size}");
        if compact {
            assert_eq!(shift, 0.0, "the hotbar moved at {size}");
        } else {
            assert!(shift < 0.0, "the panel stays centred at {size}");
        }
        let after_shot = h.screenshot();
        for (after, before) in hotbar.iter().zip(&before) {
            assert_eq!(
                *after,
                Rect::from_corners(before.min + Vec2::X * shift, before.max + Vec2::X * shift),
                "the hotbar moved other than sideways at {size}"
            );
            assert_same_pixels(&before_shot, *before, &after_shot, *after);
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
        if !compact {
            let browser = h.rect_of(&L::marker::<ItemBrowser>());
            assert!(
                window.contains(browser.min) && window.contains(browser.max),
                "the browser {browser:?} leaves the window at {size}"
            );
            assert!(
                browser.intersect(panel).is_empty(),
                "the browser {browser:?} covers the panel {panel:?} at {size}"
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

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_keyboard_alone_moves_stacks_in_the_inventory() {
    let folder = GameFolder::new().with_world("Keyboard").with_items(
        "Keyboard",
        "Tester",
        &[(10, ItemId::Dirt, 5)],
    );
    let mut h = enter_world(folder, "Keyboard");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 10) == Some((ItemId::Dirt, 5))
    });

    // Opening puts the focus on the selected hotbar slot, without its ring
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    assert_eq!(focused_cell(h.world()), Some(0));
    assert!(ringed_cells(&mut h).is_empty());

    // The arrows move the focus between the slots as they lie, and show its ring
    for key in [
        KeyCode::ArrowUp,
        KeyCode::ArrowUp,
        KeyCode::ArrowUp,
        KeyCode::ArrowRight,
    ] {
        h.press(key);
    }
    assert_eq!(focused_cell(h.world()), Some(10));
    assert_eq!(ringed_cells(&mut h), [10]);

    // The ringed slot shows its tooltip beside it, inside the window
    let tooltip = L::marker::<TooltipRoot>;
    h.wait_for(&L::text("Dirt").within(tooltip()), SCREEN);
    let (panel, cell) = (h.rect_of(&tooltip()), h.rect_of(&L::cell(10)));
    assert!(
        panel.intersect(cell).is_empty(),
        "the tooltip {panel:?} covers the focused slot {cell:?}"
    );
    let window = Rect::new(0.0, 0.0, 1280.0, 720.0);
    assert!(
        window.contains(panel.min) && window.contains(panel.max),
        "the tooltip {panel:?} leaves the window"
    );

    // Enter takes the stack, which then sits on the focused slot rather than on the cursor
    h.press(KeyCode::Enter);
    h.wait_until("the dirt held", SERVER, |w| {
        floating(w) == Some((ItemId::Dirt, 5))
    });
    let stack = h.rect_of(&L::marker::<FloatingStack>());
    let cell = h.center_of(&L::cell(10));
    assert!(
        (stack.center() - cell).abs().max_element() <= 1.0,
        "the held stack {stack:?} is not centred on the focused slot, at {cell:?}"
    );

    // Enter places it, Shift+Enter moves it to the backpack
    for _ in 0..3 {
        h.press(KeyCode::ArrowDown);
    }
    assert_eq!(focused_cell(h.world()), Some(1));
    h.press(KeyCode::Enter);
    h.wait_until("the dirt in slot 1", SERVER, |w| {
        slot(w, 1) == Some((ItemId::Dirt, 5)) && floating(w).is_none()
    });
    h.key_down(KeyCode::ShiftLeft);
    h.press(KeyCode::Enter);
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the dirt in the backpack", SERVER, |w| {
        slot(w, 9) == Some((ItemId::Dirt, 5)) && slot(w, 1).is_none()
    });

    // A click between the backpack and the hotbar, on no slot, leaves the keyboard a focus
    let (above, below) = (h.rect_of(&L::cell(27)), h.rect_of(&L::cell(0)));
    h.click_at(Vec2::new(
        above.center().x,
        (above.max.y + below.min.y) / 2.0,
    ));
    h.press(KeyCode::ArrowRight);
    assert_eq!(focused_cell(h.world()), Some(2));

    // The panel keeps Tab to itself
    for _ in 0..50 {
        h.press(KeyCode::Tab);
        assert!(focus_in_panel(h.world()), "Tab left the panel");
    }

    // Closing it lets go of the focus, and E opens it again
    h.press(KeyCode::KeyE);
    assert_eq!(h.world().resource::<InputFocus>().get(), None);
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn keys_act_only_on_a_slot_the_player_can_see() {
    let folder = GameFolder::new().with_world("Unseen").with_items(
        "Unseen",
        "Tester",
        &[
            (0, ItemId::Dirt, 5),
            (2, ItemId::Stick, 3),
            (4, ItemId::Sand, 2),
        ],
    );
    let mut h = enter_world(folder, "Unseen");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 4) == Some((ItemId::Sand, 2))
    });
    let unchanged = |h: &Harness| {
        let w = h.world();
        (slot(w, 0), slot(w, 2), slot(w, 4), slot(w, 20), floating(w))
    };
    let before = unchanged(&h);
    // The top-right corner of the window, over no slot
    let corner = Vec2::new(1279.0, 0.0);

    // The focus sits on the selected slot without its ring: a number key over nothing does nothing
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
    h.hover_at(corner);
    h.press(KeyCode::Digit3);
    h.frames(10);
    assert_eq!(
        unchanged(&h),
        before,
        "a number key over nothing moved a stack"
    );

    // Nor do Enter and Space
    h.press(KeyCode::Enter);
    h.press(KeyCode::Space);
    h.frames(10);
    assert_eq!(unchanged(&h), before, "Enter or Space moved a stack");

    // A click focuses its slot without a ring: away from it, a number key does nothing
    h.click(&L::cell(20));
    h.hover_at(corner);
    h.press(KeyCode::Digit5);
    h.frames(10);
    assert_eq!(
        unchanged(&h),
        before,
        "a number key away from the clicked slot moved a stack"
    );
}

/// Guards the modal `TabGroup` of the panel: Tab in the game gathers only the non-modal groups.
/// The `TabIndex(-1)` of the closed slots is covered by a unit test of `sync_inventory_focus`.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn tab_in_gameplay_leaves_the_hotbar_unfocused() {
    let mut h = enter_world(GameFolder::new().with_world("Tab"), "Tab");

    h.press(KeyCode::Tab);
    h.frames(1);
    assert_eq!(focused_cell(h.world()), None);
    assert!(ringed_cells(&mut h).is_empty());
    assert!(*h.world().resource::<UIMode>() == UIMode::Closed);

    // The game keys still work
    h.press(KeyCode::KeyE);
    h.wait_for(&L::marker::<InventoryRoot>(), SCREEN);
}
