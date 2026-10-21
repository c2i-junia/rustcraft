//! The furnace, through the integrated server: placed, opened, filled and emptied like a player
//! does, with the mouse and the keyboard, while it smelts.

use std::time::Instant;

use accesskit::Role;
use bevy::{input::keyboard::KeyCode, prelude::*};
use shared::players::inventory::{FURNACE_FUEL_SLOT, FURNACE_INPUT_SLOT, FURNACE_OUTPUT_SLOT};
use shared::players::Inventory;
use shared::world::{BlockId, Furnace, ItemId};

use super::game::{enter_world, floating, hold_click, place_ahead, slot};
use super::harness::{Harness, SCREEN, SERVER};
use super::inventory::focused_cell;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::item_browser::FILL_GRID;
use crate::ui::hud::{GaugeFill, FURNACE_TITLE, HINT_TO_FURNACE};
use crate::ui::widgets::tooltip::TooltipRoot;
use crate::world::ClientWorldMap;

/// The item and the count of a stack, which is all a player sees of it.
type Shown = Option<(ItemId, u32)>;

/// What the furnace at `position` holds, as the client knows it.
fn furnace(w: &World, position: IVec3) -> Option<Furnace> {
    w.resource::<ClientWorldMap>()
        .blocks
        .furnace(position)
        .cloned()
}

/// Its input, its fuel and its output.
fn stacks(w: &World, position: IVec3) -> [Shown; 3] {
    let furnace = furnace(w, position).unwrap_or_default();
    [furnace.input, furnace.fuel, furnace.output]
        .map(|stack| stack.map(|stack| (stack.item_id, stack.nb)))
}

/// The lit part of the gauge labelled `label`, while some of it shows.
fn lit(label: &str) -> L {
    L::marker::<GaugeFill>().within(L::role(Role::ProgressIndicator, label))
}

impl Harness {
    /// The width of the lit part of the arrow, in logical px, 0 while none of it shows.
    fn arrow_width(&mut self) -> f32 {
        if !self.is_visible(&lit("Smelting")) {
            return 0.0;
        }
        let fill = self.get(&lit("Smelting"));
        match self.world().get::<Node>(fill).unwrap().width {
            Val::Px(width) => width,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_furnace_smelts_sand_into_glass_put_in_and_taken_out_like_a_player_does() {
    let folder = GameFolder::new().with_world("Forge").with_items(
        "Forge",
        "Tester",
        &[
            (0, ItemId::Furnace, 1),
            (9, ItemId::Sand, 1),
            (10, ItemId::Coal, 1),
            (11, ItemId::Dirt, 5),
        ],
    );
    let mut h = enter_world(folder, "Forge");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Furnace, 1))
    });

    // Placed, it faces the player; a right click opens it, empty
    let position = place_ahead(&mut h);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_for(&L::text(FURNACE_TITLE), SCREEN);
    h.wait_until("its empty slots", SERVER, |w| {
        furnace(w, position).is_some_and(|furnace| furnace == Furnace::default())
    });
    for id in [FURNACE_INPUT_SLOT, FURNACE_FUEL_SLOT, FURNACE_OUTPUT_SLOT] {
        assert!(
            h.is_visible(&L::cell(id)),
            "the furnace slot {id} is hidden"
        );
    }
    assert!(!h.is_visible(&lit("Fuel")) && !h.is_visible(&lit("Smelting")));
    h.snapshot("furnace_empty");

    // What neither smelts nor burns stays where it is, and says nothing of the furnace
    h.hover(&L::cell(11));
    h.wait_for(&L::text("Dirt").within(L::marker::<TooltipRoot>()), SCREEN);
    assert!(!h.is_visible(&L::text(HINT_TO_FURNACE)));
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::cell(11));
    h.key_up(KeyCode::ShiftLeft);
    h.frames(30);
    assert_eq!(slot(h.world(), 11), Some((ItemId::Dirt, 5)));
    assert_eq!(stacks(h.world(), position), [None, None, None]);

    // Shift+click sends the sand to the input and the coal to the fuel: it lights, and cooks
    h.hover(&L::cell(9));
    h.wait_for(&L::text(HINT_TO_FURNACE), SCREEN);
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::cell(9));
    h.click(&L::cell(10));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the sand and the coal in the furnace", SERVER, |w| {
        furnace(w, position).is_some_and(|furnace| furnace.input.is_some() && furnace.is_lit())
    });
    assert_eq!(slot(h.world(), 9), None);
    assert_eq!(slot(h.world(), 10), None);
    h.wait_for(&lit("Fuel"), SCREEN);
    h.wait_until("the block lit", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(position)
            .is_some_and(|block| block.id == BlockId::LitFurnace)
    });
    // The arrow fills as the sand cooks
    let start = Instant::now();
    let mut widths = Vec::new();
    while widths.len() < 2 {
        assert!(start.elapsed() < SERVER, "the arrow stopped at {widths:?}");
        let width = h.arrow_width();
        if width > 0.0 && widths.last() != Some(&width) {
            widths.push(width);
        }
        h.frames(1);
    }
    assert!(widths[0] < widths[1], "the arrow went from {widths:?}");
    h.snapshot("furnace_smelting");

    // Ten seconds later the glass is out, and the arrow empty again
    h.wait_until("the glass", SERVER, |w| {
        stacks(w, position) == [None, None, Some((ItemId::Glass, 1))]
    });
    h.frames(2);
    assert_eq!(h.arrow_width(), 0.0);

    // The output takes nothing: holding dirt, a click on it leaves both
    h.click(&L::cell(11));
    h.wait_until("the dirt held", SERVER, |w| {
        floating(w) == Some((ItemId::Dirt, 5))
    });
    h.click(&L::cell(FURNACE_OUTPUT_SLOT));
    h.frames(30);
    assert_eq!(floating(h.world()), Some((ItemId::Dirt, 5)));
    assert_eq!(stacks(h.world(), position)[2], Some((ItemId::Glass, 1)));
    h.click(&L::cell(11));
    h.wait_until("the dirt back", SERVER, |w| floating(w).is_none());

    // The arrows of the keyboard reach the furnace's slots, and Enter takes the glass out
    let mut presses = 0;
    while focused_cell(h.world()) != Some(FURNACE_FUEL_SLOT) {
        h.press(KeyCode::ArrowUp);
        presses += 1;
        assert!(
            presses <= 6,
            "ArrowUp went to {:?}",
            focused_cell(h.world())
        );
    }
    h.press(KeyCode::ArrowUp);
    assert_eq!(focused_cell(h.world()), Some(FURNACE_INPUT_SLOT));
    h.press(KeyCode::ArrowRight);
    assert_eq!(focused_cell(h.world()), Some(FURNACE_OUTPUT_SLOT));
    h.press(KeyCode::Enter);
    h.wait_until("the glass held", SERVER, |w| {
        floating(w) == Some((ItemId::Glass, 1)) && stacks(w, position)[2].is_none()
    });

    // Closed, the glass goes into the inventory, and the furnace burns on
    h.press(KeyCode::KeyE);
    h.wait_until("the glass in the inventory", SERVER, |w| {
        let inventory = w.resource::<Inventory>();
        floating(w).is_none()
            && inventory
                .inner
                .values()
                .any(|stack| stack.item_id == ItemId::Glass)
    });
    assert!(!h.is_visible(&L::text(FURNACE_TITLE)));
    h.snapshot("furnace_lit_in_the_world");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn charcoal_smelts_iron_ore_into_an_ingot_whose_smelting_the_browser_shows() {
    let folder = GameFolder::new().with_world("Iron").with_items(
        "Iron",
        "Tester",
        &[
            (0, ItemId::Furnace, 1),
            (9, ItemId::IronOre, 1),
            (10, ItemId::Charcoal, 1),
        ],
    );
    let mut h = enter_world(folder, "Iron");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::Furnace, 1))
    });
    let position = place_ahead(&mut h);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_for(&L::text(FURNACE_TITLE), SCREEN);

    // The ore goes to the input, the charcoal to the fuel, and an ingot comes out
    h.key_down(KeyCode::ShiftLeft);
    h.click(&L::cell(9));
    h.click(&L::cell(10));
    h.key_up(KeyCode::ShiftLeft);
    h.wait_until("the ore and the charcoal in the furnace", SERVER, |w| {
        stacks(w, position)[..2] == [Some((ItemId::IronOre, 1)), None]
            && furnace(w, position).is_some_and(|furnace| furnace.is_lit())
    });
    h.wait_until("the ingot", SERVER, |w| {
        stacks(w, position) == [None, None, Some((ItemId::IronIngot, 1))]
    });

    // R over it shows its recipes: out of its block, and out of the furnace, which no button fills
    h.hover(&L::cell(FURNACE_OUTPUT_SLOT));
    h.press(KeyCode::KeyR);
    h.wait_for(&L::role(Role::Group, "Smelting of Iron Ingot"), SCREEN);
    for shown in [
        L::text("Furnace · 10 s"),
        L::role(Role::Group, "Recipe of Iron Ingot"),
        L::role(Role::Button, "Recipes 2"),
    ] {
        assert!(h.is_visible(&shown), "{shown:?}");
    }
    assert_eq!(h.find(&L::role(Role::Button, FILL_GRID)).len(), 1);
    h.snapshot("iron_ingot_recipes");
}
