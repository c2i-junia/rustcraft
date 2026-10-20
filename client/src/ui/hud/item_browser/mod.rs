//! The item browser: every item of the game in a panel beside the open inventory, with a search,
//! category chips and a scrolling grid. Docked right while 4 columns fit beside the inventory, a
//! drawer over the right edge below.
//!
//! Nothing comes out of it yet: a click on an item only focuses it.

use bevy::prelude::*;
use shared::world::{ItemCategory, ItemId};

use crate::state::{GameSet, GameState};
use crate::ui::hud::style_inventory_panel;
use crate::ui::menus::pause::toggle_pause_menu;

pub mod catalog;
mod display;
#[cfg(test)]
pub(crate) mod goldens;
pub mod layout;
mod setup;

pub use display::*;
pub use setup::*;

/// The title of the panel.
pub const ITEM_BROWSER_TITLE: &str = "Items";
/// The accessible label and the placeholder of the search field.
pub const SEARCH_LABEL: &str = "Search items";

/// What the browser shows. It outlives the game screen: the search is still there the next time
/// the inventory opens, for the session.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct ItemBrowserState {
    /// What the search field holds.
    pub query: String,
    /// The category chosen with the chips, `None` for All.
    pub category: Option<ItemCategory>,
    /// Whether the drawer is open, when the browser is one.
    pub drawer_open: bool,
}

/// The panel of the browser.
#[derive(Component)]
pub struct ItemBrowser;

/// The input of the search field.
#[derive(Component)]
pub struct ItemSearchField;

/// How many items the catalog shows, out of all of them.
#[derive(Component)]
pub struct ItemCount;

/// A chip choosing a category, `None` for All.
#[derive(Component)]
pub struct CategoryChip(pub Option<ItemCategory>);

/// The grid of the catalog.
#[derive(Component)]
pub struct CatalogGrid;

/// The slot of an item of the catalog.
#[derive(Component)]
pub struct CatalogSlot(pub ItemId);

/// What the catalog says when nothing matches the search.
#[derive(Component)]
pub struct NoMatch;

/// The handle of the drawer, at the right edge while it is closed.
#[derive(Component)]
pub struct DrawerHandle;

/// The first line of `NoMatch`, which names the search.
#[derive(Component)]
pub struct NoMatchMessage;

pub fn item_browser_plugin(app: &mut App) {
    app.init_resource::<ItemBrowserState>()
        .init_resource::<layout::BrowserLayout>()
        .add_systems(
            OnEnter(GameState::Game),
            setup_item_browser.after(crate::ui::hud::setup_inventory),
        )
        .add_systems(
            Update,
            (
                read_search,
                layout_item_browser,
                show_catalog_icons,
                filter_catalog.run_if(
                    resource_changed::<ItemBrowserState>
                        .or_else(any_match_filter::<Added<CatalogSlot>>),
                ),
                show_item_browser,
                style_chips,
            )
                .chain()
                .after(style_inventory_panel)
                .run_if(in_state(GameState::Game)),
        )
        .add_systems(
            Update,
            drawer_keys
                .after(GameSet::UiMode)
                .after(toggle_pause_menu)
                .before(show_item_browser)
                .run_if(in_state(GameState::Game)),
        )
        .add_observer(scroll_focused_into_view);
}
