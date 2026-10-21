//! The ghost of a recipe over the crafting grid: laid by `+ Fill grid` on a card missing
//! ingredients, it shows where each ingredient goes, faint, on a red cell where the inventory
//! lacks it. Client state only: nothing is sent.

use bevy::prelude::*;
use shared::players::inventory::{grid_slots, plan_grid_fill};
use shared::players::Inventory;
use shared::world::{grid_slot, Ingredient, ItemStack, CRAFTING_GRID_SIZE};

use crate::ui::hud::item_browser::{tag_member, MISSING_TINT};
use crate::ui::hud::{CraftingGridCell, InventoryCell, InventoryRoot};
use crate::ui::theme::BEVEL_WIDTH;
use crate::ui::widgets::slot::{ICON_SIZE, SLOT_SIZE};
use crate::world::MaterialResource;

/// The opacity of the icon of a ghost cell.
pub const GHOST_ALPHA: f32 = 0.4;

/// The ghost laid over the crafting grid, if any.
#[derive(Resource, Default, Clone, PartialEq, Debug)]
pub struct GridGhost(pub Option<Ghost>);

/// A recipe laid over the crafting grid, at its top left corner as a fill puts it.
#[derive(Clone, PartialEq, Debug)]
pub struct Ghost {
    /// The stacks of the nine cells of the grid when it was laid: any change takes it away.
    pub grid: Vec<Option<ItemStack>>,
    /// The empty cells of the grid the recipe puts an ingredient into.
    pub cells: Vec<GhostCell>,
}

/// An empty cell of the grid under a ghost: the ingredient the recipe puts there, and whether
/// the inventory lacks it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GhostCell {
    pub slot: u32,
    pub ingredient: Ingredient,
    pub lacking: bool,
}

/// The tint of a crafting cell under a ghost whose ingredient the inventory lacks.
#[derive(Component)]
pub struct GhostTint;

/// The faint icon of a crafting cell under a ghost.
#[derive(Component)]
pub struct GhostIcon;

/// The stacks of the nine cells of the crafting grid of `inventory`.
pub fn grid_stacks(inventory: &Inventory) -> Vec<Option<ItemStack>> {
    grid_slots(CRAFTING_GRID_SIZE)
        .map(|slot| inventory.inner.get(&slot).copied())
        .collect()
}

/// The ghost of the recipe of index `recipe` over the grid of `size` of `inventory`: its cells
/// whose slot is empty, each lacking where one round of a fill would leave it without its
/// ingredient (`plan_grid_fill`, as the fill plans it). `None` when the fill is refused for
/// another reason than missing ingredients: the recipe does not fit, or the grid cannot go back.
pub fn ghost_of(inventory: &Inventory, recipe: u16, size: u32) -> Option<Ghost> {
    let fill = plan_grid_fill(inventory, recipe, 1, size).ok()?;
    let width = fill.recipe.width;
    let cells = fill
        .recipe
        .cells
        .iter()
        .zip(&fill.lacking)
        .enumerate()
        .filter_map(|(index, (cell, &lacking))| {
            let ingredient = (*cell)?;
            let slot = grid_slot((index / width) as u32, (index % width) as u32);
            (!inventory.inner.contains_key(&slot)).then_some(GhostCell {
                slot,
                ingredient,
                lacking,
            })
        })
        .collect();
    Some(Ghost {
        grid: grid_stacks(inventory),
        cells,
    })
}

/// The tint of a crafting cell, over its whole inside, hidden until a ghost shows it.
pub fn ghost_tint() -> impl Bundle {
    (
        GhostTint,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        BackgroundColor(MISSING_TINT),
        Visibility::Hidden,
        Pickable::IGNORE,
    )
}

/// The faint icon of a crafting cell, where the slot's icon sits, hidden until a ghost shows it.
pub fn ghost_icon(icon: ImageNode) -> impl Bundle {
    let inset = px((SLOT_SIZE - 2.0 * BEVEL_WIDTH - ICON_SIZE) / 2.0);
    (
        GhostIcon,
        ImageNode {
            color: Color::WHITE.with_alpha(GHOST_ALPHA),
            ..icon
        },
        Node {
            position_type: PositionType::Absolute,
            left: inset,
            top: inset,
            width: px(ICON_SIZE),
            height: px(ICON_SIZE),
            ..default()
        },
        Visibility::Hidden,
        Pickable::IGNORE,
    )
}

/// Shows the ghost over the crafting grid: in each of its cells the icon of the ingredient,
/// faint, a family's members in turn as on the cards, on a red tint where the inventory lacks
/// it. The other cells show no ghost.
#[allow(clippy::type_complexity)]
pub fn show_grid_ghost(
    ghost: Res<GridGhost>,
    time: Res<Time<Real>>,
    materials: Res<MaterialResource>,
    cells: Query<(&InventoryCell, &Children), With<CraftingGridCell>>,
    mut tints: Query<&mut Visibility, With<GhostTint>>,
    mut icons: Query<(&mut ImageNode, &mut Visibility), (With<GhostIcon>, Without<GhostTint>)>,
) {
    let shown = |visible: bool| {
        if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for (cell, children) in &cells {
        let on_cell = ghost
            .0
            .iter()
            .flat_map(|ghost| &ghost.cells)
            .find(|ghost| ghost.slot == cell.id);
        for child in children.iter() {
            if let Ok(mut visibility) = tints.get_mut(child) {
                visibility.set_if_neq(shown(on_cell.is_some_and(|ghost| ghost.lacking)));
            }
            if let Ok((mut image, mut visibility)) = icons.get_mut(child) {
                visibility.set_if_neq(shown(on_cell.is_some()));
                let Some(ghost) = on_cell else {
                    continue;
                };
                let item = match ghost.ingredient {
                    Ingredient::Item(item) => item,
                    Ingredient::Tag(tag) => tag_member(tag, time.elapsed()),
                };
                let sprite = materials
                    .items
                    .as_ref()
                    .and_then(|atlas| atlas.sprite(&format!("{item:?}")));
                if sprite.is_some() && image.texture_atlas != sprite {
                    image.texture_atlas = sprite;
                }
            }
        }
    }
}

/// Takes the ghost away when the inventory closes or the crafting grid changes.
pub fn clear_grid_ghost(
    inventory: Res<Inventory>,
    inventory_root: Query<&Visibility, With<InventoryRoot>>,
    mut ghost: ResMut<GridGhost>,
) {
    let Some(laid) = &ghost.0 else {
        return;
    };
    let open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    if !open || laid.grid != grid_stacks(&inventory) {
        ghost.0 = None;
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{recipes_for, ItemId, ItemTag};

    use super::*;

    fn stack(item_id: ItemId, nb: u32) -> ItemStack {
        ItemStack {
            item_id,
            item_type: item_id.get_default_type(),
            nb,
        }
    }

    fn index_of(item: ItemId) -> u16 {
        recipes_for(item).next().unwrap().index
    }

    #[test]
    fn a_ghost_shows_the_empty_cells_of_the_recipe() {
        // One plank in the grid, one in the inventory, no stick: the pickaxe at a table
        let mut inventory = Inventory::new();
        inventory.open_table = Some(IVec3::ZERO);
        inventory.inner.insert(9, stack(ItemId::OakPlanks, 1));
        inventory
            .inner
            .insert(grid_slot(0, 0), stack(ItemId::SprucePlanks, 1));
        let ghost = ghost_of(&inventory, index_of(ItemId::WoodenPickaxe), 3).unwrap();
        let planks = Ingredient::Tag(ItemTag::Planks);
        let stick = Ingredient::Item(ItemId::Stick);
        let cell = |slot, ingredient, lacking| GhostCell {
            slot,
            ingredient,
            lacking,
        };
        // the first cell holds a plank: no ghost there, but its plank serves a cell
        assert_eq!(
            ghost.cells,
            [
                cell(grid_slot(0, 1), planks, false),
                cell(grid_slot(0, 2), planks, true),
                cell(grid_slot(1, 1), stick, true),
                cell(grid_slot(2, 1), stick, true),
            ]
        );
        assert_eq!(ghost.grid, grid_stacks(&inventory));
        // A recipe larger than the grid in use has no ghost
        assert_eq!(
            ghost_of(&inventory, index_of(ItemId::WoodenPickaxe), 2),
            None
        );
    }

    /// A world of the open inventory with the nine crafting cells and their ghost parts.
    fn grid_world() -> World {
        let mut world = World::new();
        world.init_resource::<Inventory>();
        world.init_resource::<GridGhost>();
        world.init_resource::<MaterialResource>();
        let mut time = Time::<Real>::default();
        time.update_with_duration(Duration::ZERO);
        world.insert_resource(time);
        world.spawn((InventoryRoot, Visibility::Visible));
        for row in 0..3 {
            for col in 0..3 {
                world
                    .spawn((
                        InventoryCell {
                            id: grid_slot(row, col),
                        },
                        CraftingGridCell { row, col },
                    ))
                    .with_children(|cell| {
                        cell.spawn(ghost_tint());
                        cell.spawn(ghost_icon(ImageNode::default()));
                    });
            }
        }
        world
    }

    /// The slots whose ghost tint shows, and those whose ghost icon shows.
    fn shown(world: &mut World) -> (Vec<u32>, Vec<u32>) {
        world.run_system_once(show_grid_ghost).unwrap();
        let mut cells = world.query::<(&InventoryCell, &Children)>();
        let (mut tinted, mut iconed) = (Vec::new(), Vec::new());
        for (cell, children) in cells.iter(world) {
            for child in children.iter() {
                let shown = world.get::<Visibility>(child) != Some(&Visibility::Hidden);
                if world.get::<GhostTint>(child).is_some() && shown {
                    tinted.push(cell.id);
                }
                if world.get::<GhostIcon>(child).is_some() && shown {
                    iconed.push(cell.id);
                    let alpha = world.get::<ImageNode>(child).unwrap().color.alpha();
                    assert!((alpha - 0.4).abs() < 1e-6, "{alpha}");
                }
            }
        }
        tinted.sort_unstable();
        iconed.sort_unstable();
        (tinted, iconed)
    }

    #[test]
    fn the_ghost_cells_show_faint_icons_on_red_where_lacking() {
        let mut world = grid_world();
        assert_eq!(shown(&mut world), (vec![], vec![]));
        // One stick, no coal: the torch's coal is red, its stick only faint
        world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, stack(ItemId::Stick, 1));
        let inventory = world.resource::<Inventory>().clone();
        let ghost = ghost_of(&inventory, index_of(ItemId::Torch), 2);
        world.resource_mut::<GridGhost>().0 = ghost;
        assert_eq!(
            shown(&mut world),
            (
                vec![grid_slot(0, 0)],
                vec![grid_slot(0, 0), grid_slot(1, 0)]
            )
        );
        world.resource_mut::<GridGhost>().0 = None;
        assert_eq!(shown(&mut world), (vec![], vec![]));
    }

    #[test]
    fn the_ghost_goes_when_the_grid_changes_or_the_inventory_closes() {
        let mut world = grid_world();
        let lay = |world: &mut World| {
            let inventory = world.resource::<Inventory>().clone();
            world.resource_mut::<GridGhost>().0 = ghost_of(&inventory, index_of(ItemId::Torch), 2);
            world.run_system_once(clear_grid_ghost).unwrap();
            world.resource::<GridGhost>().0.is_some()
        };
        let cleared = |world: &mut World| {
            world.run_system_once(clear_grid_ghost).unwrap();
            world.resource::<GridGhost>().0.is_none()
        };
        // Another slot changes: it stays
        assert!(lay(&mut world));
        world
            .resource_mut::<Inventory>()
            .inner
            .insert(9, stack(ItemId::Dirt, 1));
        assert!(!cleared(&mut world));
        // A stack goes into the grid: it goes
        world
            .resource_mut::<Inventory>()
            .inner
            .insert(grid_slot(1, 1), stack(ItemId::Dirt, 1));
        assert!(cleared(&mut world));
        // A stack leaves the grid: it goes
        assert!(lay(&mut world));
        world
            .resource_mut::<Inventory>()
            .inner
            .remove(&grid_slot(1, 1));
        assert!(cleared(&mut world));
        // The inventory closes: it goes
        assert!(lay(&mut world));
        let mut root = world.query_filtered::<&mut Visibility, With<InventoryRoot>>();
        *root.single_mut(&mut world).unwrap() = Visibility::Hidden;
        assert!(cleared(&mut world));
    }
}
