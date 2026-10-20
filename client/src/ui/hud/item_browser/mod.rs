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
pub mod recipes;
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
    /// Whether the catalog keeps only the items craftable now, in Survival.
    pub craftable: bool,
    /// The recipes or the uses shown instead of the catalog, if any.
    pub view: Option<ItemView>,
    /// The views the back button and Escape go back to, the last one first.
    pub history: Vec<ItemView>,
}

/// Which recipes of an item the view shows: those making it, or those it goes into.
// First used by the recipe view (Task 4)
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecipeTab {
    Recipes,
    Uses,
}

/// The recipes or the uses of an item.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ItemView {
    pub item: ItemId,
    pub tab: RecipeTab,
}

// First used by the recipe view (Task 4)
#[allow(dead_code)]
impl ItemBrowserState {
    /// Shows `view`, the open one going on the history, and opens the drawer it is in.
    pub fn open(&mut self, view: ItemView, compact: bool) {
        if self.view != Some(view) {
            if let Some(open) = self.view.replace(view) {
                self.history.push(open);
            }
        }
        if compact {
            self.drawer_open = true;
        }
    }

    /// Goes back to the last view of the history, or to the catalog.
    pub fn back(&mut self) {
        self.view = self.history.pop();
    }

    /// Goes back to the catalog, forgetting the history.
    pub fn leave_view(&mut self) {
        self.view = None;
        self.history.clear();
    }
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
            (focus_search, drawer_keys)
                .chain()
                .after(GameSet::UiMode)
                .after(toggle_pause_menu)
                .before(show_item_browser)
                .run_if(in_state(GameState::Game)),
        )
        .add_observer(scroll_focused_into_view);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(item: ItemId, tab: RecipeTab) -> ItemView {
        ItemView { item, tab }
    }

    #[test]
    fn opening_views_pushes_the_history_and_back_pops_it() {
        let pickaxe = view(ItemId::WoodenPickaxe, RecipeTab::Recipes);
        let stick = view(ItemId::Stick, RecipeTab::Recipes);
        let mut state = ItemBrowserState::default();
        state.open(pickaxe, false);
        assert_eq!(state.view, Some(pickaxe));
        assert!(state.history.is_empty());
        assert!(!state.drawer_open);
        state.open(stick, false);
        assert_eq!(state.history, [pickaxe]);
        state.open(stick, false);
        assert_eq!(state.history, [pickaxe]);

        state.back();
        assert_eq!(state.view, Some(pickaxe));
        assert!(state.history.is_empty());
        state.back();
        assert_eq!(state.view, None);

        state.open(pickaxe, true);
        state.open(stick, true);
        assert!(state.drawer_open);
        state.leave_view();
        assert_eq!(state.view, None);
        assert!(state.history.is_empty());
    }
}
