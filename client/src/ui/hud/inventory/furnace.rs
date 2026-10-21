//! The furnace section of the inventory panel, where the chest section goes, laid out as
//! Minecraft's: the input slot, the flame under it, the fuel slot under the flame, then an arrow
//! pointing to a larger output slot. The flame and the arrow are the pack's
//! `gui/sprites/container/furnace/` sprites, drawn at twice their texels like the item icons,
//! over silhouettes of them.

use accesskit::Role;
use bevy::a11y::AccessibilityNode;
use bevy::prelude::*;
use shared::players::inventory::{FURNACE_FUEL_SLOT, FURNACE_INPUT_SLOT, FURNACE_OUTPUT_SLOT};
use shared::players::Inventory;
use shared::world::{Furnace, COOK_TICKS};

use super::setup::icon;
use super::{FurnaceRow, InventoryCell};
use crate::ui::theme::{BEVEL_HI, SPACE_L, SPACE_S, SPACE_XS};
use crate::ui::widgets::slot::{slot, slot_node, SLOT_SIZE};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::{AtlasWrapper, ClientWorldMap};

pub const FURNACE_TITLE: &str = "Furnace";

pub const FLAME_PATH: &str = "./graphics/hud/furnace_flame.png";
pub const FLAME_EMPTY_PATH: &str = "./graphics/hud/furnace_flame_empty.png";
pub const ARROW_PATH: &str = "./graphics/hud/furnace_arrow.png";
pub const ARROW_EMPTY_PATH: &str = "./graphics/hud/furnace_arrow_empty.png";

/// The side of the flame's sprite, in texels.
const FLAME_TEXELS: u32 = 14;
/// The width and the height of the arrow's sprite, in texels.
const ARROW_TEXELS: UVec2 = UVec2::new(24, 16);
/// The logical px of a texel of the sprites: two, like the item icons.
const TEXEL: f32 = 2.0;

/// The side of the output slot, larger than the others like Minecraft's, in logical px.
pub const OUTPUT_SIZE: f32 = SLOT_SIZE + 2.0 * SPACE_S;

/// A gauge of the open furnace: the flame, which burns down with the fuel, or the arrow, which
/// fills as the input cooks. Its children are the empty sprite, then the lit one, `GaugeFill`.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FurnaceGauge {
    Flame,
    Arrow,
}

/// The lit sprite of a gauge, cut to the part its furnace shows.
#[derive(Component)]
pub struct GaugeFill;

/// The rows of the flame shown from its bottom, as Minecraft draws it: all of them when the fuel
/// is lit, one at its last tick, none once the furnace is out.
pub fn flame_rows(furnace: &Furnace) -> u32 {
    if !furnace.is_lit() {
        return 0;
    }
    // A burn left longer than its total, from a furnace whose total was lost, shows full
    let total = furnace.burn_total.max(furnace.burn_left);
    let left = f32::from(furnace.burn_left) / f32::from(total);
    (left * (FLAME_TEXELS - 1) as f32).ceil() as u32 + 1
}

/// The columns of the arrow shown from its left, as Minecraft draws it: as far as the input has
/// cooked.
pub fn arrow_columns(furnace: &Furnace) -> u32 {
    let cooked = f32::from(furnace.cook.min(COOK_TICKS)) / f32::from(COOK_TICKS);
    (cooked * ARROW_TEXELS.x as f32).ceil() as u32
}

impl FurnaceGauge {
    /// The width and the height of its sprite, in texels.
    fn texels(self) -> UVec2 {
        match self {
            FurnaceGauge::Flame => UVec2::splat(FLAME_TEXELS),
            FurnaceGauge::Arrow => ARROW_TEXELS,
        }
    }

    /// The texels of its sprite `furnace` shows: the flame burns down from its top, the arrow
    /// fills from its left.
    fn shown(self, furnace: &Furnace) -> URect {
        let full = self.texels();
        match self {
            FurnaceGauge::Flame => URect::new(0, full.y - flame_rows(furnace), full.x, full.y),
            FurnaceGauge::Arrow => URect::new(0, 0, arrow_columns(furnace), full.y),
        }
    }

    /// Its accessible name.
    fn label(self) -> &'static str {
        match self {
            FurnaceGauge::Flame => "Fuel",
            FurnaceGauge::Arrow => "Smelting",
        }
    }
}

/// A gauge, `Role::ProgressIndicator` from 0 to 100: the silhouette of its sprite in `BEVEL_HI`,
/// and the lit sprite over it, of which nothing shows yet.
fn gauge(gauge: FurnaceGauge, assets: &AssetServer) -> impl Bundle {
    let (empty, lit) = match gauge {
        FurnaceGauge::Flame => (FLAME_EMPTY_PATH, FLAME_PATH),
        FurnaceGauge::Arrow => (ARROW_EMPTY_PATH, ARROW_PATH),
    };
    let size = gauge.texels().as_vec2() * TEXEL;
    let mut accessible = accesskit::Node::new(Role::ProgressIndicator);
    accessible.set_min_numeric_value(0.0);
    accessible.set_max_numeric_value(100.0);
    accessible.set_numeric_value(0.0);
    (
        gauge,
        // the role before the label: the label's default node must not win
        AccessibilityNode(accessible),
        AccessibleLabel::new(gauge.label()),
        Node {
            width: px(size.x),
            height: px(size.y),
            ..default()
        },
        Pickable::IGNORE,
        children![
            (
                ImageNode {
                    image: assets.load(empty),
                    color: BEVEL_HI,
                    ..default()
                },
                Node {
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                Pickable::IGNORE,
            ),
            (
                GaugeFill,
                ImageNode::new(assets.load(lit)),
                // At the bottom of the flame, at the left of the arrow
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.),
                    bottom: px(0.),
                    ..default()
                },
                Visibility::Hidden,
                Pickable::IGNORE,
            ),
        ],
    )
}

/// The flame lit in full, as the furnace cards of the item browser show it.
pub fn lit_flame(assets: &AssetServer) -> impl Bundle {
    let side = FLAME_TEXELS as f32 * TEXEL;
    (
        ImageNode::new(assets.load(FLAME_PATH)),
        Node {
            width: px(side),
            height: px(side),
            flex_shrink: 0.0,
            ..default()
        },
        Pickable::IGNORE,
    )
}

/// The furnace section, hidden until a furnace is open: a column of the input slot, the flame and
/// the fuel slot, the arrow, and the output slot.
pub(super) fn spawn_furnace_row(
    parent: &mut ChildSpawnerCommands,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    let cell = |id: u32, label: &str| {
        (
            InventoryCell { id },
            Tooltip::default(),
            slot(label, None, icon(atlas), font.clone()),
        )
    };
    parent
        .spawn((
            FurnaceRow,
            Node {
                display: Display::None,
                // In the middle of the panel, Minecraft's place
                align_self: AlignSelf::Center,
                align_items: AlignItems::Center,
                column_gap: px(SPACE_L),
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(SPACE_XS),
                ..default()
            })
            .with_children(|column| {
                column.spawn(cell(FURNACE_INPUT_SLOT, "Furnace input"));
                column.spawn(gauge(FurnaceGauge::Flame, assets));
                column.spawn(cell(FURNACE_FUEL_SLOT, "Furnace fuel"));
            });
            row.spawn(gauge(FurnaceGauge::Arrow, assets));
            row.spawn(cell(FURNACE_OUTPUT_SLOT, "Furnace output"))
                .insert(Node {
                    width: px(OUTPUT_SIZE),
                    height: px(OUTPUT_SIZE),
                    ..slot_node()
                });
        });
}

/// Draws the flame and the arrow of the open furnace as the server last sent it, and tells their
/// values in percent. They show nothing while its state is not known.
pub fn display_furnace_system(
    inventory: Res<Inventory>,
    world_map: Res<ClientWorldMap>,
    mut gauges: Query<(&FurnaceGauge, &mut AccessibilityNode, &Children)>,
    mut fills: Query<(&mut Node, &mut ImageNode, &mut Visibility), With<GaugeFill>>,
) {
    let furnace = inventory
        .open_furnace
        .and_then(|pos| world_map.blocks.furnace(pos))
        .cloned()
        .unwrap_or_default();
    for (&gauge, mut accessible, children) in &mut gauges {
        let shown = gauge.shown(&furnace);
        let full = gauge.texels();
        let value = f64::from(shown.width() * shown.height()) / f64::from(full.x * full.y) * 100.0;
        if accessible.numeric_value() != Some(value) {
            accessible.set_numeric_value(value);
        }
        for child in children.iter() {
            let Ok((mut node, mut image, mut visibility)) = fills.get_mut(child) else {
                continue;
            };
            visibility.set_if_neq(if shown.is_empty() {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            });
            let rect = Some(shown.as_rect());
            if image.rect != rect {
                image.rect = rect;
            }
            let size = shown.size().as_vec2() * TEXEL;
            // A write marks the node changed, which lays the UI out again: only on a change
            if node.width != px(size.x) || node.height != px(size.y) {
                node.width = px(size.x);
                node.height = px(size.y);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;
    use shared::world::{BlockId, ChunkMap};

    fn burning(burn_left: u16, burn_total: u16) -> Furnace {
        Furnace {
            burn_left,
            burn_total,
            ..default()
        }
    }

    fn cooking(cook: u16) -> Furnace {
        Furnace { cook, ..default() }
    }

    #[test]
    fn the_flame_burns_down_by_whole_rows_as_minecraft_s() {
        assert_eq!(flame_rows(&burning(0, 1600)), 0);
        assert_eq!(flame_rows(&burning(1600, 1600)), 14);
        assert_eq!(flame_rows(&burning(800, 1600)), 8);
        // Its last tick still shows a row
        assert_eq!(flame_rows(&burning(1, 1600)), 2);
        assert_eq!(flame_rows(&burning(1, 100)), 2);
        assert_eq!(flame_rows(&burning(100, 0)), 14);
    }

    #[test]
    fn the_arrow_fills_by_whole_columns_as_the_input_cooks() {
        assert_eq!(arrow_columns(&cooking(0)), 0);
        assert_eq!(arrow_columns(&cooking(1)), 1);
        assert_eq!(arrow_columns(&cooking(100)), 12);
        assert_eq!(arrow_columns(&cooking(199)), 24);
        assert_eq!(arrow_columns(&cooking(COOK_TICKS)), 24);
    }

    /// The size, the part of its sprite and the visibility of the lit sprite of `gauge`, and the
    /// value it tells, once displayed.
    fn displayed(world: &mut World, gauge: Entity) -> (Vec2, Option<Rect>, Visibility, f64) {
        world.run_system_once(display_furnace_system).unwrap();
        let fill = world.get::<Children>(gauge).unwrap()[1];
        let node = world.get::<Node>(fill).unwrap();
        let size = match (node.width, node.height) {
            (Val::Px(width), Val::Px(height)) => Vec2::new(width, height),
            other => panic!("{other:?}"),
        };
        let value = world
            .get::<AccessibilityNode>(gauge)
            .unwrap()
            .numeric_value()
            .unwrap();
        (
            size,
            world.get::<ImageNode>(fill).unwrap().rect,
            *world.get::<Visibility>(fill).unwrap(),
            value,
        )
    }

    #[test]
    fn the_gauges_show_the_furnace_the_server_sent() {
        // The gauges load their sprites
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Image>();
        let world = app.world_mut();
        world.init_resource::<Inventory>();
        world.init_resource::<ClientWorldMap>();
        let assets = world.resource::<AssetServer>().clone();
        let flame = world.spawn(gauge(FurnaceGauge::Flame, &assets)).id();
        let arrow = world.spawn(gauge(FurnaceGauge::Arrow, &assets)).id();

        // No furnace known: nothing shows
        let (_, _, visibility, value) = displayed(world, flame);
        assert_eq!((visibility, value), (Visibility::Hidden, 0.0));

        let pos = IVec3::new(1, 2, 3);
        let mut blocks = ChunkMap::with_loaded_region(IVec3::ZERO, IVec3::splat(15));
        blocks.fill_box(pos, pos, BlockId::LitFurnace);
        blocks.insert_furnace(
            pos,
            Furnace {
                burn_left: 800,
                burn_total: 1600,
                cook: 100,
                ..default()
            },
        );
        world.resource_mut::<ClientWorldMap>().blocks = blocks;
        world.resource_mut::<Inventory>().open_furnace = Some(pos);

        // The 8 bottom rows of the flame, twice as large
        assert_eq!(
            displayed(world, flame),
            (
                Vec2::new(28.0, 16.0),
                Some(Rect::new(0.0, 6.0, 14.0, 14.0)),
                Visibility::Inherited,
                8.0 / 14.0 * 100.0
            )
        );
        // Half the arrow
        assert_eq!(
            displayed(world, arrow),
            (
                Vec2::new(24.0, 32.0),
                Some(Rect::new(0.0, 0.0, 12.0, 16.0)),
                Visibility::Inherited,
                50.0
            )
        );
    }
}
