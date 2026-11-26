//! The armour of the local player in survival: ten shields above the hearts, each one full, half
//! or empty, hidden while it wears none, and while the inventory is open, as the hearts.

use crate::i18n::{tr, trf};
use bevy::prelude::*;
use shared::players::armour::MAX_ARMOUR_POINTS;
use shared::players::{Inventory, Player};

use crate::player::CurrentPlayerMarker;
use crate::ui::hud::health::{heart, Heart, HEALTH_BAND};
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::InventoryRoot;
use crate::ui::theme::SPACE_XS;

const EMPTY_PATH: &str = "./graphics/hud/armour_empty.png";
const HALF_PATH: &str = "./graphics/hud/armour_half.png";
const FULL_PATH: &str = "./graphics/hud/armour_full.png";

/// The side of a shield on screen, in logical px: twice its texture, as the hearts.
const SHIELD_SIZE: f32 = 18.0;

/// The shields: one for every 2 points of armour.
const SHIELDS: u32 = MAX_ARMOUR_POINTS / 2;

/// The height the shields take above the hearts, in logical px: their margin and a shield.
pub const ARMOUR_BAND: f32 = SPACE_XS + SHIELD_SIZE;

/// The row of shields.
#[derive(Component)]
pub struct ArmourBar;

/// The shield `index` of the row, from the left.
#[derive(Component)]
pub struct Shield {
    index: u32,
}

/// The shields as the accessibility tree and the tests read them.
pub fn armour_label(points: u32) -> String {
    trf("hud.armour.label", &[("points", &points), ("MAX_ARMOUR_POINTS", &MAX_ARMOUR_POINTS)])
}

/// Spawns the shields over the hearts, hidden until the local player wears armour.
pub fn spawn_armour_bar(
    mut commands: Commands,
    hotbar: Query<Entity, With<Hotbar>>,
    assets: Res<AssetServer>,
) {
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    let row = commands
        .spawn((
            ArmourBar,
            Name::new(tr("hud.armour")),
            AccessibleLabel::new(armour_label(0)),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.),
                bottom: percent(100.),
                margin: UiRect::bottom(px(HEALTH_BAND + SPACE_XS)),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(hotbar),
        ))
        .id();
    for index in 0..SHIELDS {
        commands.spawn((
            Shield { index },
            ImageNode::new(assets.load(EMPTY_PATH)),
            Node {
                width: px(SHIELD_SIZE),
                height: px(SHIELD_SIZE),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ));
    }
}

/// Fills the shields with the points of the armour worn, and shows them in survival while there
/// are some and the inventory is closed. Writes nothing while none of these changes.
pub fn update_armour_bar(
    inventory: Res<Inventory>,
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut bar: Query<(Entity, &mut Visibility), With<ArmourBar>>,
    mut shields: Query<(&Shield, &mut ImageNode)>,
    panel: Query<&Visibility, (With<InventoryRoot>, Without<ArmourBar>)>,
    assets: Res<AssetServer>,
    mut shown: Local<Option<(Entity, u32, bool)>>,
    mut commands: Commands,
) {
    let (Ok(player), Ok((bar, mut visibility))) = (player.single(), bar.single_mut()) else {
        return;
    };
    let open = panel.single().is_ok_and(|v| *v == Visibility::Visible);
    let points = inventory.armour_points();
    let is_shown = points > 0 && player.game_mode.takes_damage() && !open;
    // The bar of another session starts afresh
    let state = (bar, points, is_shown);
    if *shown == Some(state) {
        return;
    }
    *shown = Some(state);

    *visibility = if is_shown {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    commands
        .entity(bar)
        .insert(AccessibleLabel::new(armour_label(points)));
    for (shield, mut image) in &mut shields {
        image.image = assets.load(match heart(points, shield.index) {
            Heart::Empty => EMPTY_PATH,
            Heart::Half => HALF_PATH,
            Heart::Full => FULL_PATH,
        });
    }
}
