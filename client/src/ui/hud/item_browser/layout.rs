//! Where the item browser goes: how many columns fit beside the inventory panel, and where that
//! panel goes to leave them room.

use bevy::prelude::*;

use crate::ui::theme::{BEVEL_WIDTH, SPACE_L, SPACE_S, SPACE_XS};
use crate::ui::widgets::list::{RING_ROOM, SCROLLBAR_WIDTH};
use crate::ui::widgets::slot::SLOT_SIZE;

/// The most columns the docked browser takes.
pub const MAX_COLUMNS: u32 = 8;
/// The fewest columns the docked browser takes: below, it is a drawer.
pub const MIN_COLUMNS: u32 = 4;
/// The most columns of the drawer.
pub const MAX_DRAWER_COLUMNS: u32 = 6;
/// The width around the grid: the padding and the bevel of the panel, the room of the focus ring
/// on both sides of the grid, and the scrollbar with its gap.
pub const BROWSER_CHROME: f32 =
    2.0 * (SPACE_S + BEVEL_WIDTH) + 2.0 * RING_ROOM + SPACE_XS + SCROLLBAR_WIDTH;

/// The width of a column: a slot and the gap after it.
const COLUMN: f32 = SLOT_SIZE + SPACE_XS;

/// The width of the browser holding `columns` columns.
pub fn browser_width(columns: u32) -> f32 {
    columns as f32 * COLUMN - SPACE_XS + BROWSER_CHROME
}

/// How many columns fit in `room`, at least 0.
fn columns_in(room: f32) -> u32 {
    ((room - BROWSER_CHROME + SPACE_XS) / COLUMN)
        .floor()
        .max(0.0) as u32
}

/// Where the browser and the inventory panel go, in logical px.
#[derive(Resource, Clone, Copy, PartialEq, Debug, Default)]
pub struct BrowserLayout {
    pub columns: u32,
    /// Whether the browser is a drawer over the right edge, as fewer than `MIN_COLUMNS` columns
    /// fit beside the inventory.
    pub compact: bool,
    /// The width of the browser.
    pub width: f32,
    /// The left of the inventory panel while it is open.
    pub inventory_left: f32,
}

/// The layout of the browser in a window of `window` logical px, beside an inventory panel
/// `inventory_width` wide. The room beside the panel, with `SPACE_L` on its left, between the panel
/// and the browser, and on the right of the browser, gives the columns, up to `MAX_COLUMNS`; the
/// panel stays centred when the browser leaves it the room, else it slides left, as far as
/// `SPACE_L` from the edge. Below `MIN_COLUMNS` columns the browser is a drawer of up to
/// `MAX_DRAWER_COLUMNS` columns over the right edge, and the panel stays centred.
pub fn browser_layout(window: Vec2, inventory_width: f32) -> BrowserLayout {
    let centred = (window.x - inventory_width) / 2.0;
    let free = window.x - inventory_width - 3.0 * SPACE_L;
    let columns = columns_in(free).min(MAX_COLUMNS);
    if columns >= MIN_COLUMNS {
        let width = browser_width(columns);
        let beside = window.x - width - 2.0 * SPACE_L - inventory_width;
        return BrowserLayout {
            columns,
            compact: false,
            width,
            inventory_left: centred.min(beside).max(SPACE_L),
        };
    }
    let columns = columns_in(window.x - 2.0 * SPACE_L).clamp(1, MAX_DRAWER_COLUMNS);
    BrowserLayout {
        columns,
        compact: true,
        width: browser_width(columns),
        inventory_left: centred,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::SPACE_L;

    /// The width of the inventory panel: 9 slots, 8 gaps, its padding and its bevel.
    const INVENTORY: f32 = 502.0;

    fn layout(width: f32, height: f32) -> BrowserLayout {
        browser_layout(Vec2::new(width, height), INVENTORY)
    }

    #[test]
    fn a_wide_window_docks_eight_columns_and_slides_the_inventory_left() {
        let docked = layout(1280.0, 720.0);
        assert_eq!(
            docked,
            BrowserLayout {
                columns: 8,
                compact: false,
                width: 470.0,
                inventory_left: 276.0,
            }
        );
        // 16 px between the panel and the browser, and between the browser and the edge
        assert_eq!(1280.0 - SPACE_L - docked.width, 794.0);
        assert_eq!(docked.inventory_left + INVENTORY + SPACE_L, 794.0);
    }

    #[test]
    fn a_very_wide_window_keeps_the_inventory_centred() {
        let docked = layout(1920.0, 1079.0);
        assert_eq!(docked.columns, 8);
        assert_eq!(docked.inventory_left, (1920.0 - INVENTORY) / 2.0);
    }

    #[test]
    fn a_small_window_docks_four_columns() {
        let docked = layout(854.0, 480.0);
        assert_eq!((docked.columns, docked.compact), (4, false));
        assert_eq!(docked.width, 254.0);
        assert_eq!(docked.inventory_left, 66.0);
    }

    #[test]
    fn below_four_columns_the_browser_is_a_drawer() {
        for width in [800.0, 640.0] {
            let drawer = layout(width, 480.0);
            assert!(drawer.compact, "{width}");
            assert_eq!(drawer.columns, 6, "{width}");
            assert_eq!(drawer.width, 362.0, "{width}");
            assert_eq!(drawer.inventory_left, (width - INVENTORY) / 2.0, "{width}");
        }
    }

    #[test]
    fn the_docked_browser_never_covers_the_inventory() {
        for width in 400..=3000 {
            let width = width as f32;
            let found = layout(width, 720.0);
            if found.compact {
                assert!(found.width <= width, "{width}: {found:?}");
                assert!((1..=MAX_DRAWER_COLUMNS).contains(&found.columns), "{width}");
                continue;
            }
            assert!(
                (MIN_COLUMNS..=MAX_COLUMNS).contains(&found.columns),
                "{width}: {found:?}"
            );
            assert_eq!(found.width, browser_width(found.columns));
            assert!(found.inventory_left >= SPACE_L, "{width}: {found:?}");
            assert!(
                found.inventory_left + INVENTORY + SPACE_L <= width - SPACE_L - found.width,
                "{width}: {found:?}"
            );
        }
    }
}
