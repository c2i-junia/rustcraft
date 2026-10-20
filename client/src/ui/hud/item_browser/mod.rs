//! The item browser: every item of the game in a panel beside the open inventory, with a search,
//! category chips and a scrolling grid. Docked right while 4 columns fit beside the inventory, a
//! drawer over the right edge below.
//!
//! It is also the recipe book: the recipes or the uses of an item replace the catalog in the same
//! panel, from a click on an item in Survival or from R / U over any item slot. Nothing comes out
//! of it yet: in Creative a click on an item only focuses it.

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
mod view;

pub use display::*;
pub use setup::*;
pub use view::*;

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

/// The second line of `NoMatch`, which says how to see more.
#[derive(Component)]
pub struct NoMatchHint;

/// The chip keeping only the items craftable now, in Survival.
#[derive(Component)]
pub struct CraftableChip;

/// The small square in the top-right corner of a catalog slot whose item is craftable now.
#[derive(Component)]
pub struct CraftableDot;

/// The footer of the browser in Survival (`true`) or in Creative.
#[derive(Component)]
pub struct ModeFooter(pub bool);

/// The title of the panel: `Items`, `Recipes` or `Uses`.
#[derive(Component)]
pub struct BrowserTitle;

/// A part of the panel showing the catalog, hidden while a view shows: the search, the chips and
/// the grid.
#[derive(Component)]
pub struct CatalogPart;

/// The recipe view, shown instead of the catalog.
#[derive(Component)]
pub struct RecipePanel;

/// The button going back in the history of the view.
#[derive(Component)]
pub struct BackButton;

/// The icon of the item of the view.
#[derive(Component)]
pub struct ViewedItem;

/// The name of the item of the view.
#[derive(Component)]
pub struct ViewedItemName;

/// The category of the item of the view.
#[derive(Component)]
pub struct ViewedItemCategory;

/// A chip showing the recipes or the uses of the item of the view.
#[derive(Component)]
pub struct RecipeTabChip(pub RecipeTab);

/// The column of the cards of the view, and the view they were built for.
#[derive(Component)]
pub struct RecipeCards(pub Option<ItemView>);

/// The card of a recipe.
#[derive(Component)]
pub struct RecipeCard;

/// A slot of a card: an ingredient of its recipe, or its output, `count` of it.
#[derive(Component)]
pub struct RecipeSlot {
    pub item: ItemId,
    pub count: u32,
}

/// A focusable part of the view, which takes Tab only while the view shows.
#[derive(Component)]
pub struct RecipeStop;

/// The tint over an ingredient of a recipe, shown while the player lacks that item.
#[derive(Component)]
pub struct MissingTint {
    pub recipe: u16,
    pub item: ItemId,
}

/// Under a card: where its recipe is made, or why it cannot be made now.
#[derive(Component)]
pub struct RecipeNote(pub u16);

/// The button that will fill the crafting grid with a recipe, disabled until the server can.
#[derive(Component)]
pub struct FillGridButton;

/// Whether the local player is in Survival, and the items it can craft now: some recipe of
/// theirs has its ingredients in the inventory and the crafting grid, whatever the grid size.
/// Nothing is craftable in Creative, where nothing is shown as such.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct CraftableNow {
    pub survival: bool,
    pub items: Vec<ItemId>,
}

pub fn item_browser_plugin(app: &mut App) {
    app.init_resource::<ItemBrowserState>()
        .init_resource::<layout::BrowserLayout>()
        .init_resource::<CraftableNow>()
        .add_systems(
            OnEnter(GameState::Game),
            setup_item_browser.after(crate::ui::hud::setup_inventory),
        )
        .add_systems(
            Update,
            (
                read_search,
                layout_item_browser,
                show_item_icons,
                update_craftable_now,
                filter_catalog.run_if(
                    resource_changed::<ItemBrowserState>
                        .or_else(resource_changed::<CraftableNow>)
                        .or_else(any_match_filter::<Added<CatalogSlot>>),
                ),
                (mark_craftable_items, show_mode_footer).run_if(
                    resource_changed::<CraftableNow>
                        .or_else(any_match_filter::<Added<CatalogSlot>>)
                        .or_else(any_match_filter::<Added<RecipeSlot>>),
                ),
                show_view.run_if(
                    resource_changed::<ItemBrowserState>
                        .or_else(any_match_filter::<Added<RecipePanel>>),
                ),
                style_recipe_cards.run_if(
                    resource_changed::<shared::players::Inventory>
                        .or_else(any_match_filter::<Added<RecipeNote>>),
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
            (focus_search, drawer_keys, view_keys, click_items)
                .chain()
                .after(GameSet::UiMode)
                .after(toggle_pause_menu)
                .before(show_item_browser)
                .run_if(in_state(GameState::Game)),
        )
        .add_observer(scroll_focused_into_view)
        .add_observer(go_back)
        .add_observer(choose_tab);
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
