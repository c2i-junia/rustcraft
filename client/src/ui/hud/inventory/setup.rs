use crate::constants::{HOTBAR_BORDER, HOTBAR_CELL_SIZE, HOTBAR_PADDING, MAX_HOTBAR_SLOTS};
use crate::state::GameState;
use crate::ui::hud::{
    CraftResultCell, CraftingGridCell, FloatingStack, InventoryCell, InventoryDialog,
    InventoryRoot, StackCount, StackIcon, UiDialog,
};
use crate::world::{AtlasWrapper, MaterialResource};
use bevy::{prelude::*, ui::FocusPolicy};
use shared::world::{grid_slot, CRAFTING_GRID_SIZE};
use shared::MAX_INVENTORY_SLOTS;

/// Border of a cell that is neither selected nor hovered.
pub const CELL_BORDER_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
const STACK_COUNT_FONT_SIZE: f32 = 15.0;

/// The count of a stack, drawn over its icon. Empty until `display_inventory_system` fills it.
fn stack_count() -> impl Bundle {
    (
        StackCount,
        Text::new(""),
        TextColor(Color::WHITE),
        TextFont::from_font_size(STACK_COUNT_FONT_SIZE),
        Node {
            position_type: PositionType::Absolute,
            ..default()
        },
        ZIndex(1),
    )
}

/// The icon of a stack. It starts on any sprite of the items atlas, hidden.
fn stack_icon(atlas: &AtlasWrapper, node: Node) -> impl Bundle {
    (
        StackIcon,
        ImageNode::from_atlas_image(
            atlas.texture.clone(),
            atlas.sprite("Dirt").unwrap_or_default(),
        ),
        node,
        Visibility::Hidden,
    )
}

/// Spawns the cell of inventory slot `slot`, as the hotbar and the inventory dialog show it.
pub fn spawn_inventory_cell(parent: &mut ChildSpawnerCommands, slot: u32, atlas: &AtlasWrapper) {
    spawn_cell(parent, InventoryCell { id: slot }, atlas);
}

/// Spawns a cell showing a stack, marked by `marker`.
fn spawn_cell(parent: &mut ChildSpawnerCommands, marker: impl Bundle, atlas: &AtlasWrapper) {
    parent
        .spawn((
            marker,
            Button,
            BorderColor::all(CELL_BORDER_COLOR),
            FocusPolicy::Block,
            Node {
                width: Val::Px(HOTBAR_CELL_SIZE),
                height: Val::Px(HOTBAR_CELL_SIZE),
                margin: UiRect::ZERO,
                position_type: PositionType::Relative,
                padding: UiRect::all(Val::Px(HOTBAR_PADDING)),
                border: UiRect::all(Val::Px(HOTBAR_BORDER)),
                ..default()
            },
        ))
        .with_children(|cell| {
            cell.spawn(stack_count());
            cell.spawn(stack_icon(
                atlas,
                Node {
                    width: Val::Px(HOTBAR_CELL_SIZE - 2. * (HOTBAR_PADDING + HOTBAR_BORDER)),
                    position_type: PositionType::Relative,
                    ..default()
                },
            ));
        });
}

pub fn setup_inventory(mut commands: Commands, materials_resource: Res<MaterialResource>) {
    let Some(atlas) = materials_resource.items.as_ref() else {
        error!("The item textures are not loaded: the inventory cannot be shown");
        return;
    };

    // Inventory root: root container for the inventory
    let root = commands
        .spawn((
            UiDialog,
            InventoryRoot,
            DespawnOnExit(GameState::Game),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(0.),
                right: Val::Percent(0.),
                bottom: Val::Percent(0.),
                top: Val::Percent(0.),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::BLACK.with_alpha(0.4)),
            GlobalZIndex(2),
            Visibility::Hidden,
        ))
        .id();

    let dialog = commands
        .spawn((
            InventoryDialog,
            Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Percent(7.)),
                border_radius: BorderRadius::all(Val::Percent(10.)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.4, 0.4, 0.4)),
        ))
        .id();

    let inventory_title = commands
        .spawn((
            Text::new("Inventory"),
            TextFont {
                font_size: 24.0.into(),
                ..default()
            },
            Node {
                align_content: AlignContent::Center,
                ..default()
            },
        ))
        .id();

    // The slots after those of the hotbar, which shows the first ones itself
    let inventory_grid = commands
        .spawn((
            Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::auto(MAX_HOTBAR_SLOTS as u16),
                margin: UiRect::all(Val::Px(10.)),
                position_type: PositionType::Relative,
                ..default()
            },
            BorderColor::all(Color::BLACK),
        ))
        .with_children(|grid| {
            for slot in MAX_HOTBAR_SLOTS..MAX_INVENTORY_SLOTS {
                spawn_inventory_cell(grid, slot, atlas);
            }
        })
        .id();

    let crafting = spawn_crafting_row(&mut commands, atlas);
    let floating_stack = spawn_floating_stack(&mut commands, atlas);

    commands
        .entity(dialog)
        .add_children(&[inventory_title, crafting, inventory_grid]);

    commands
        .entity(root)
        .add_children(&[dialog, floating_stack]);
}

/// The crafting grid, an arrow, and the cell of what the grid makes.
fn spawn_crafting_row(commands: &mut Commands, atlas: &AtlasWrapper) -> Entity {
    commands
        .spawn(Node {
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            margin: UiRect::all(Val::Px(10.)),
            column_gap: Val::Px(20.),
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                display: Display::Grid,
                grid_template_columns: RepeatedGridTrack::auto(CRAFTING_GRID_SIZE as u16),
                ..default()
            })
            .with_children(|grid| {
                for row in 0..CRAFTING_GRID_SIZE {
                    for col in 0..CRAFTING_GRID_SIZE {
                        let cell = InventoryCell {
                            id: grid_slot(row, col),
                        };
                        spawn_cell(grid, (cell, CraftingGridCell { row, col }), atlas);
                    }
                }
            });
            row.spawn((
                Text::new("->"),
                TextFont {
                    font_size: 24.0.into(),
                    ..default()
                },
            ));
            spawn_cell(row, CraftResultCell, atlas);
        })
        .id()
}

/// The stack held by the cursor, which it follows over the open inventory.
fn spawn_floating_stack(commands: &mut Commands, atlas: &AtlasWrapper) -> Entity {
    commands
        .spawn((
            FloatingStack,
            Node {
                width: Val::Px(20.),
                height: Val::Px(20.),
                position_type: PositionType::Absolute,
                ..default()
            },
            FocusPolicy::Pass,
        ))
        .with_children(|stack| {
            stack.spawn(stack_count());
            stack.spawn(stack_icon(
                atlas,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(0.),
                    right: Val::Percent(0.),
                    bottom: Val::Percent(0.),
                    top: Val::Percent(0.),
                    ..default()
                },
            ));
        })
        .id()
}
