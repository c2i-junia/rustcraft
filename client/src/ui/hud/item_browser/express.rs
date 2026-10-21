//! The express recipe book: Ctrl+click on an item slot of the browser (the catalog, a bookmark,
//! a recent item) fills the crafting grid with a recipe of the item at once, without the recipe
//! view, and `+ Fill grid` on a card missing ingredients lays the ghost of its recipe over the
//! grid.

use bevy::{picking::hover::Hovered, prelude::*};
use shared::players::inventory::{plan_grid_fill, InventoryAction};
use shared::players::Inventory;
use shared::world::{recipes, recipes_for, ItemId, RecipeView};

use super::recipes::{fill_button, FillButton};
use super::{BrowserRequests, CatalogSlot};
use crate::network::buffered_client::CurrentFrameInputs;
use crate::ui::hud::inventory::{ghost_of, grid_in_use, GridGhost};
use crate::ui::hud::InventoryRoot;
use crate::ui::widgets::slot::ActionRefused;

/// Whether `inventory` fills the grid of `grid_size` with `recipe` now: one round of it is there
/// once the grid is back in the inventory, as `FillCraftingGrid` plans it (`plan_grid_fill`),
/// and no chest nor furnace takes the place of the grid in the panel.
pub fn fills_now(inventory: &Inventory, recipe: &RecipeView, grid_size: u32) -> bool {
    inventory.open_chest.is_none()
        && inventory.open_furnace.is_none()
        && plan_grid_fill(inventory, recipe.index, 1, grid_size)
            .is_ok_and(|fill| fill.plan.is_some())
}

/// The first recipe of `item` that fills the grid of `grid_size` now, if any.
pub fn first_recipe_made_now(inventory: &Inventory, item: ItemId, grid_size: u32) -> Option<u16> {
    recipes_for(item)
        .find(|recipe| fills_now(inventory, recipe, grid_size))
        .map(|recipe| recipe.index)
}

/// Sends the fills of the grid the browser asks for, as actions of the frame's input, which the
/// client predicts and the server applies; a fill sent takes the ghost away. `+ Fill grid` fills
/// with its recipe (Shift: as many rounds as possible), or lays its ghost over the grid when only
/// ingredients are missing. Ctrl+click on an item slot of the browser, the inventory open, in
/// both modes, fills the grid in use with the first recipe of its item made now, once, or as
/// many times as possible with Shift; when none is, the slot is refused and nothing else happens.
#[allow(clippy::too_many_arguments)]
pub fn express_fill(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    inventory: Res<Inventory>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    catalog: Query<(Entity, &Hovered, &CatalogSlot)>,
    mut requests: ResMut<BrowserRequests>,
    mut ghost: ResMut<GridGhost>,
    mut frame_inputs: ResMut<CurrentFrameInputs>,
    mut refusals: MessageWriter<ActionRefused>,
) {
    let grid_size = grid_in_use(&inventory);
    let times = |shift: bool| if shift { u32::MAX } else { 1 };
    let actions = &mut frame_inputs.0.inventory_actions;
    if let Some((recipe, shift)) = requests.fill.take() {
        let button = recipes()
            .nth(recipe as usize)
            .and_then(|view| fill_button(&inventory, &view, grid_size));
        match button {
            Some(FillButton::Fill) => {
                let times = times(shift);
                actions.push(InventoryAction::FillCraftingGrid { recipe, times });
                ghost.0 = None;
            }
            Some(FillButton::Ghost) => ghost.0 = ghost_of(&inventory, recipe, grid_size),
            None => {}
        }
    }

    if !matches!(inventory_root.single(), Ok(&Visibility::Visible))
        || !mouse.just_pressed(MouseButton::Left)
        || !keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight])
    {
        return;
    }
    let Some((slot, item)) = catalog
        .iter()
        .find_map(|(slot, hovered, item)| hovered.get().then_some((slot, item.0)))
    else {
        return;
    };
    let Some(recipe) = first_recipe_made_now(&inventory, item, grid_size) else {
        refusals.write(ActionRefused { slot: Some(slot) });
        return;
    };
    let times = times(keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]));
    actions.push(InventoryAction::FillCraftingGrid { recipe, times });
    ghost.0 = None;
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{grid_slot, ItemStack};

    use super::*;

    fn put(world: &mut World, slot: u32, item_id: ItemId, nb: u32) {
        let stack = ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        };
        world.resource_mut::<Inventory>().inner.insert(slot, stack);
    }

    /// A world with the open inventory and a catalog slot of each item of `items`, none hovered.
    fn browser(items: &[ItemId]) -> (World, Vec<Entity>) {
        let mut world = World::new();
        world.init_resource::<Inventory>();
        world.init_resource::<CurrentFrameInputs>();
        world.init_resource::<ButtonInput<MouseButton>>();
        world.init_resource::<ButtonInput<KeyCode>>();
        world.init_resource::<Messages<ActionRefused>>();
        world.init_resource::<BrowserRequests>();
        world.init_resource::<GridGhost>();
        world.spawn((InventoryRoot, Visibility::Visible));
        let slots = items
            .iter()
            .map(|&item| world.spawn((CatalogSlot(item), Hovered(false))).id())
            .collect();
        (world, slots)
    }

    /// Presses the left button over `slot` with `keys` held, runs `express_fill`, and returns the
    /// actions it sent and the slots it refused.
    fn ctrl_click(
        world: &mut World,
        slot: Entity,
        keys: &[KeyCode],
    ) -> (Vec<InventoryAction>, Vec<Option<Entity>>) {
        world.entity_mut(slot).insert(Hovered(true));
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        for &key in keys {
            world.resource_mut::<ButtonInput<KeyCode>>().press(key);
        }
        world.run_system_once(express_fill).unwrap();
        world.entity_mut(slot).insert(Hovered(false));
        world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
        let actions = std::mem::take(
            &mut world
                .resource_mut::<CurrentFrameInputs>()
                .0
                .inventory_actions,
        );
        let refused = world
            .resource_mut::<Messages<ActionRefused>>()
            .drain()
            .map(|refused| refused.slot)
            .collect();
        (actions, refused)
    }

    fn fill(recipe: RecipeView, times: u32) -> InventoryAction {
        InventoryAction::FillCraftingGrid {
            recipe: recipe.index,
            times,
        }
    }

    #[test]
    fn a_ctrl_click_fills_the_grid_with_the_first_recipe_made_now() {
        let (mut world, slots) = browser(&[ItemId::Stick, ItemId::Torch]);
        put(&mut world, 9, ItemId::SprucePlanks, 1);
        put(&mut world, 10, ItemId::Charcoal, 1);
        // the grid goes back first: its plank and its stick count
        put(&mut world, grid_slot(1, 1), ItemId::SprucePlanks, 1);
        put(&mut world, grid_slot(0, 0), ItemId::Stick, 1);
        let sticks = recipes_for(ItemId::Stick).next().unwrap();
        assert_eq!(
            ctrl_click(&mut world, slots[0], &[KeyCode::ControlLeft]),
            (vec![fill(sticks, 1)], vec![])
        );
        // The torch of coal cannot be made, that of charcoal can
        let [coal, charcoal] = recipes_for(ItemId::Torch).collect::<Vec<_>>()[..] else {
            panic!("two recipes of torches");
        };
        assert_ne!(coal.index, charcoal.index);
        assert_eq!(
            ctrl_click(&mut world, slots[1], &[KeyCode::ControlRight]),
            (vec![fill(charcoal, 1)], vec![])
        );
    }

    #[test]
    fn ctrl_shift_fills_as_many_rounds_as_possible() {
        let (mut world, slots) = browser(&[ItemId::Stick]);
        put(&mut world, 9, ItemId::OakPlanks, 8);
        let sticks = recipes_for(ItemId::Stick).next().unwrap();
        assert_eq!(
            ctrl_click(
                &mut world,
                slots[0],
                &[KeyCode::ControlLeft, KeyCode::ShiftLeft]
            ),
            (vec![fill(sticks, u32::MAX)], vec![])
        );
    }

    #[test]
    fn a_ctrl_click_with_nothing_to_make_shakes_the_slot_and_sends_nothing() {
        let items = [ItemId::Dirt, ItemId::Stick, ItemId::WoodenPickaxe];
        let (mut world, slots) = browser(&items);
        let ctrl = [KeyCode::ControlLeft];
        // No recipe, then no planks
        for &slot in &slots[..2] {
            assert_eq!(
                ctrl_click(&mut world, slot, &ctrl),
                (vec![], vec![Some(slot)])
            );
        }
        // The pickaxe needs a crafting table
        put(&mut world, 9, ItemId::OakPlanks, 3);
        put(&mut world, 10, ItemId::Stick, 2);
        assert_eq!(
            ctrl_click(&mut world, slots[2], &ctrl),
            (vec![], vec![Some(slots[2])])
        );
        // A chest takes the place of the grid
        world.resource_mut::<Inventory>().open_chest = Some(IVec3::ZERO);
        assert_eq!(
            ctrl_click(&mut world, slots[1], &ctrl),
            (vec![], vec![Some(slots[1])])
        );
    }

    #[test]
    fn only_a_ctrl_and_left_click_on_an_item_of_the_open_inventory_fills() {
        let (mut world, slots) = browser(&[ItemId::Stick]);
        put(&mut world, 9, ItemId::OakPlanks, 2);
        assert_eq!(ctrl_click(&mut world, slots[0], &[]), (vec![], vec![]));
        // Not with the right button
        world.entity_mut(slots[0]).insert(Hovered(true));
        world
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        world
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ControlLeft);
        world.run_system_once(express_fill).unwrap();
        let actions = &world.resource::<CurrentFrameInputs>().0.inventory_actions;
        assert!(actions.is_empty());
        world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
        // Nor on a closed inventory
        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut world).unwrap() = Visibility::Hidden;
        assert_eq!(
            ctrl_click(&mut world, slots[0], &[KeyCode::ControlLeft]),
            (vec![], vec![])
        );
    }

    #[test]
    fn a_ctrl_click_opens_no_view_in_survival() {
        use crate::ui::hud::item_browser::layout::BrowserLayout;
        use crate::ui::hud::item_browser::{click_items, CraftableNow, ItemBrowserState};
        use crate::ui::hud::UIMode;
        use bevy::input_focus::{InputFocus, InputFocusVisible};

        let (mut world, slots) = browser(&[ItemId::Stick]);
        world.insert_resource(UIMode::Opened);
        world.init_resource::<InputFocus>();
        world.init_resource::<InputFocusVisible>();
        world.init_resource::<BrowserLayout>();
        world.init_resource::<ItemBrowserState>();
        world.insert_resource(CraftableNow {
            survival: true,
            items: Vec::new(),
        });
        let click = |world: &mut World, keys: &[KeyCode]| {
            world.entity_mut(slots[0]).insert(Hovered(true));
            world
                .resource_mut::<ButtonInput<MouseButton>>()
                .press(MouseButton::Left);
            for &key in keys {
                world.resource_mut::<ButtonInput<KeyCode>>().press(key);
            }
            world.run_system_once(click_items).unwrap();
            world.resource_mut::<ButtonInput<MouseButton>>().reset_all();
            world.resource_mut::<ButtonInput<KeyCode>>().reset_all();
            world.resource::<ItemBrowserState>().view
        };
        assert_eq!(click(&mut world, &[KeyCode::ControlLeft]), None);
        assert!(click(&mut world, &[]).is_some());
    }

    /// Asks for `+ Fill grid` on the card of `recipe`, as its button does, and runs
    /// `express_fill`: the actions it sent.
    fn press_fill_grid(world: &mut World, recipe: RecipeView) -> Vec<InventoryAction> {
        world.resource_mut::<BrowserRequests>().fill = Some((recipe.index, false));
        world.run_system_once(express_fill).unwrap();
        std::mem::take(
            &mut world
                .resource_mut::<CurrentFrameInputs>()
                .0
                .inventory_actions,
        )
    }

    #[test]
    fn fill_grid_on_a_card_missing_ingredients_lays_a_ghost_and_sends_nothing() {
        let (mut world, slots) = browser(&[ItemId::Stick]);
        put(&mut world, 9, ItemId::Stick, 1);
        let torch = recipes_for(ItemId::Torch).next().unwrap();
        assert_eq!(press_fill_grid(&mut world, torch), []);
        let inventory = world.resource::<Inventory>().clone();
        let laid = world.resource::<GridGhost>().0.clone();
        assert_eq!(laid, ghost_of(&inventory, torch.index, 2));
        assert!(laid.is_some());

        // Another ghost replaces it
        let sticks = recipes_for(ItemId::Stick).next().unwrap();
        assert_eq!(press_fill_grid(&mut world, sticks), []);
        let laid = world.resource::<GridGhost>().0.clone();
        assert_eq!(laid, ghost_of(&inventory, sticks.index, 2));

        // A fill sent replaces it too, from the button or a Ctrl+click
        put(&mut world, 10, ItemId::OakPlanks, 2);
        assert_eq!(press_fill_grid(&mut world, sticks), [fill(sticks, 1)]);
        assert_eq!(world.resource::<GridGhost>().0, None);
        world.resource_mut::<GridGhost>().0 = ghost_of(&inventory, torch.index, 2);
        assert_eq!(
            ctrl_click(&mut world, slots[0], &[KeyCode::ControlLeft]).0,
            [fill(sticks, 1)]
        );
        assert_eq!(world.resource::<GridGhost>().0, None);

        // A card that needs a table neither fills nor lays a ghost
        world.resource_mut::<GridGhost>().0 = None;
        let pickaxe = recipes_for(ItemId::WoodenPickaxe).next().unwrap();
        assert_eq!(press_fill_grid(&mut world, pickaxe), []);
        assert_eq!(world.resource::<GridGhost>().0, None);
    }
}
