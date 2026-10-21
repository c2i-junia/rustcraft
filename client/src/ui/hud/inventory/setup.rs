use super::furnace::spawn_furnace_row;
use super::ghost::{ghost_icon, ghost_tint};
use crate::constants::MAX_HOTBAR_SLOTS;
use crate::state::GameState;
use crate::ui::assets::load_font;
use crate::ui::hud::hotbar::{Hotbar, HotbarItemName};
use crate::ui::hud::{
    ChestGrid, CraftResultCell, CraftingGridCell, CraftingRow, FloatingStack, HudBottom,
    InventoryCell, InventoryDialog, InventoryRoot, InventoryTitle, UiDialog,
};
use crate::ui::theme::{
    BEVEL_LO, BEVEL_WIDTH, FONT_L, FONT_M, MUTED, SPACE_L, SPACE_M, SPACE_S, SPACE_XS, TEXT,
};
use crate::ui::widgets::focus::ArrowNav;
use crate::ui::widgets::slot::{
    slot, slot_count, slot_durability, slot_icon, slot_node, SLOT_SIZE,
};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{AtlasWrapper, MaterialResource};
use bevy::input_focus::tab_navigation::TabGroup;
use bevy::prelude::*;
use shared::players::inventory::chest_slots;
use shared::world::{grid_slot, CRAFTING_GRID_SIZE};
use shared::MAX_INVENTORY_SLOTS;

pub const INVENTORY_TITLE: &str = "Inventory";
pub const CRAFTING_TABLE_TITLE: &str = "Crafting Table";
pub const CHEST_TITLE: &str = "Chest";

/// The height of the hotbar above the bottom of the window, in logical px.
const HOTBAR_BOTTOM: f32 = SPACE_L;
/// The top of the closed panel, the hotbar, above the bottom of the window, in logical px: the
/// bottom and the margin of the panel, its bevel and its padding, then a slot.
pub const HOTBAR_TOP: f32 = HOTBAR_BOTTOM + SPACE_S + BEVEL_WIDTH + SPACE_S + SLOT_SIZE;

/// The icon of a slot. It starts on any sprite of the items atlas, hidden.
pub(super) fn icon(atlas: &AtlasWrapper) -> ImageNode {
    ImageNode::from_atlas_image(
        atlas.texture.clone(),
        atlas.sprite("Dirt").unwrap_or_default(),
    )
}

/// A text of the panel in the game font.
fn panel_text(text: &str, font: &Handle<Font>, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: font.clone().into(),
            font_size: px(FONT_L).into(),
            ..default()
        },
        TextColor(color),
    )
}

/// Builds the panel at the bottom of the screen and the stack held by the cursor. The bottom row
/// of the panel is the hotbar; the inventory above it, hidden until opened, takes its room even
/// then, so that opening it never moves the hotbar.
pub fn setup_inventory(
    mut commands: Commands,
    materials: Res<MaterialResource>,
    asset_server: Res<AssetServer>,
) {
    let Some(atlas) = materials.items.as_ref() else {
        error!("The item textures are not loaded: the hotbar and the inventory cannot be shown");
        return;
    };
    let font = load_font(&asset_server);

    commands
        .spawn((
            HudBottom,
            DespawnOnExit(GameState::Game),
            // Over the whole window: the panel at the bottom, centred, and the item browser
            // beside it
            Node {
                position_type: PositionType::Absolute,
                left: px(0.),
                right: px(0.),
                top: px(0.),
                bottom: px(0.),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                padding: UiRect::bottom(px(HOTBAR_BOTTOM + SPACE_S)),
                ..default()
            },
            Pickable::IGNORE,
            GlobalZIndex(1),
            // Tab stays among the slots of the open panel and the item browser
            TabGroup::modal(),
        ))
        .with_children(|bottom| {
            bottom
                .spawn((
                    InventoryDialog,
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(px(SPACE_S)),
                        row_gap: px(SPACE_M),
                        border: UiRect::all(px(BEVEL_WIDTH)),
                        ..default()
                    },
                    // Drawn by `style_inventory_panel` while the inventory is open
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                    // Closed, its room above the hotbar is empty; the slots stay pickable
                    Pickable::IGNORE,
                    // The arrows stay among its slots
                    ArrowNav,
                ))
                .with_children(|panel| {
                    panel
                        .spawn((
                            UiDialog,
                            InventoryRoot,
                            Visibility::Hidden,
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(SPACE_M),
                                ..default()
                            },
                        ))
                        .with_children(|inventory| {
                            inventory
                                .spawn((InventoryTitle, panel_text(INVENTORY_TITLE, &font, TEXT)));
                            spawn_crafting_row(inventory, atlas, &font);
                            spawn_chest_grid(inventory, atlas, &font);
                            spawn_furnace_row(inventory, atlas, &font, &asset_server);
                            spawn_backpack(inventory, atlas, &font);
                        });
                    spawn_hotbar(panel, atlas, &font);
                });
        });

    commands.spawn((
        FloatingStack,
        DespawnOnExit(GameState::Game),
        Node {
            position_type: PositionType::Absolute,
            width: px(SLOT_SIZE),
            height: px(SLOT_SIZE),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        GlobalZIndex(3),
        Pickable::IGNORE,
        children![slot_icon(icon(atlas)), slot_durability(), slot_count(font)],
    ));
}

/// The 3x9 slots of the open chest, hidden until one is open.
fn spawn_chest_grid(parent: &mut ChildSpawnerCommands, atlas: &AtlasWrapper, font: &Handle<Font>) {
    parent
        .spawn((
            ChestGrid,
            Node {
                display: Display::None,
                grid_template_columns: RepeatedGridTrack::auto(MAX_HOTBAR_SLOTS as u16),
                row_gap: px(SPACE_XS),
                column_gap: px(SPACE_XS),
                ..default()
            },
        ))
        .with_children(|grid| {
            for (index, id) in chest_slots().enumerate() {
                let label = format!("Chest slot {}", index + 1);
                grid.spawn((
                    InventoryCell { id },
                    Tooltip::default(),
                    slot(&label, None, icon(atlas), font.clone()),
                ));
            }
        });
}

/// The 3x9 slots after those of the hotbar.
fn spawn_backpack(parent: &mut ChildSpawnerCommands, atlas: &AtlasWrapper, font: &Handle<Font>) {
    parent
        .spawn(Node {
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::auto(MAX_HOTBAR_SLOTS as u16),
            row_gap: px(SPACE_XS),
            column_gap: px(SPACE_XS),
            ..default()
        })
        .with_children(|grid| {
            for id in MAX_HOTBAR_SLOTS..MAX_INVENTORY_SLOTS {
                let label = format!("Backpack slot {}", id - MAX_HOTBAR_SLOTS + 1);
                grid.spawn((
                    InventoryCell { id },
                    Tooltip::default(),
                    slot(&label, None, icon(atlas), font.clone()),
                ));
            }
        });
}

/// The bottom row of the panel, numbered 1 to 9.
fn spawn_hotbar(parent: &mut ChildSpawnerCommands, atlas: &AtlasWrapper, font: &Handle<Font>) {
    parent
        .spawn((
            Hotbar { selected: 0 },
            Node {
                column_gap: px(SPACE_XS),
                ..default()
            },
        ))
        .with_children(|bar| {
            // Over the row, centred; shown by `show_selected_item_name`
            bar.spawn((
                HotbarItemName::default(),
                Text::new(""),
                TextFont {
                    font: font.clone().into(),
                    font_size: px(FONT_M).into(),
                    ..default()
                },
                TextColor(TEXT),
                TextLayout::justify(Justify::Center),
                TextShadow {
                    offset: Vec2::splat(2.0),
                    color: BEVEL_LO,
                },
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.),
                    right: px(0.),
                    bottom: percent(100.),
                    margin: UiRect::bottom(px(SPACE_S)),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
            for id in 0..MAX_HOTBAR_SLOTS {
                let label = format!("Hotbar slot {}", id + 1);
                bar.spawn((
                    InventoryCell { id },
                    Tooltip::default(),
                    slot(&label, Some(id + 1), icon(atlas), font.clone()),
                ));
            }
        });
}

/// The crafting grid, an arrow, and the cell of what the grid makes.
fn spawn_crafting_row(
    parent: &mut ChildSpawnerCommands,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
) {
    parent
        .spawn((
            CraftingRow,
            Node {
                align_items: AlignItems::Center,
                column_gap: px(SPACE_L),
                ..default()
            },
        ))
        .with_children(|row| {
            // The grid has as many rows and columns as its shown cells reach
            row.spawn(Node {
                display: Display::Grid,
                row_gap: px(SPACE_XS),
                column_gap: px(SPACE_XS),
                ..default()
            })
            .with_children(|grid| {
                for row in 0..CRAFTING_GRID_SIZE {
                    for col in 0..CRAFTING_GRID_SIZE {
                        let label = format!("Crafting grid {}, {}", row + 1, col + 1);
                        // Placed by hand: a hidden cell leaves the grid, which would move
                        // the next ones in its place
                        grid.spawn((
                            InventoryCell {
                                id: grid_slot(row, col),
                            },
                            CraftingGridCell { row, col },
                            Tooltip::default(),
                            slot(&label, None, icon(atlas), font.clone()),
                        ))
                        .insert(Node {
                            grid_row: GridPlacement::start(row as i16 + 1),
                            grid_column: GridPlacement::start(col as i16 + 1),
                            ..slot_node()
                        })
                        // after the icon and the count: drawn over them
                        .with_children(|cell| {
                            cell.spawn(ghost_tint());
                            cell.spawn(ghost_icon(icon(atlas)));
                        });
                    }
                }
            });
            row.spawn(panel_text("->", font, MUTED));
            row.spawn((
                CraftResultCell,
                Tooltip::default(),
                slot("Crafting result", None, icon(atlas), font.clone()),
            ));
        });
}
