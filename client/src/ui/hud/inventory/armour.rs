//! The armour slots of the inventory, at the right of the crafting row: two rows of two, the
//! outline of their piece showing in those that are empty.

use bevy::prelude::*;
use shared::players::armour::ArmourPiece;
use shared::players::Inventory;

use super::setup::icon;
use crate::ui::hud::InventoryCell;
use crate::ui::theme::{BEVEL_WIDTH, SPACE_XS};
use crate::ui::widgets::slot::{slot, ICON_SIZE, SLOT_SIZE};
use crate::ui::widgets::tooltip::Tooltip;
use crate::world::AtlasWrapper;

/// The outline in the empty slot of a piece.
#[derive(Component)]
pub struct ArmourOutline(ArmourPiece);

/// Where the outline of `piece` is.
fn outline_path(piece: ArmourPiece) -> String {
    format!("./graphics/hud/slot_{}.png", piece.name().to_lowercase())
}

pub(super) fn spawn_armour_slots(
    parent: &mut ChildSpawnerCommands,
    atlas: &AtlasWrapper,
    font: &Handle<Font>,
    assets: &AssetServer,
) {
    let inset = px((SLOT_SIZE - 2.0 * BEVEL_WIDTH - ICON_SIZE) / 2.0);
    parent
        .spawn(Node {
            display: Display::Grid,
            grid_template_columns: RepeatedGridTrack::auto(2),
            row_gap: px(SPACE_XS),
            column_gap: px(SPACE_XS),
            margin: UiRect::left(auto()),
            ..default()
        })
        .with_children(|grid| {
            for piece in ArmourPiece::ALL {
                grid.spawn((
                    InventoryCell { id: piece.slot() },
                    Tooltip::default(),
                    slot(piece.name(), None, icon(atlas), font.clone()),
                ))
                .with_child((
                    ArmourOutline(piece),
                    ImageNode::new(assets.load(outline_path(piece))),
                    Node {
                        position_type: PositionType::Absolute,
                        left: inset,
                        top: inset,
                        width: px(ICON_SIZE),
                        height: px(ICON_SIZE),
                        ..default()
                    },
                    Pickable::IGNORE,
                ));
            }
        });
}

/// Shows the outlines of the pieces that are not worn.
pub fn show_armour_outlines(
    inventory: Res<Inventory>,
    mut outlines: Query<(&ArmourOutline, &mut Visibility)>,
) {
    for (outline, mut visibility) in &mut outlines {
        visibility.set_if_neq(if inventory.inner.contains_key(&outline.0.slot()) {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        });
    }
}
