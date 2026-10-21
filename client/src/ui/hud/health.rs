//! The health of the local player in survival: ten hearts over the left of the hotbar, each one
//! full, half or empty, hidden while the inventory is open over them. The textures are Pixel
//! Perfection's `gui/sprites/hud/heart/`.

use bevy::prelude::*;
use shared::players::game_mode::{GameMode, MAX_HEALTH};
use shared::players::Player;

use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::{Hotbar, HotbarItemName};
use crate::ui::hud::InventoryRoot;
use crate::ui::theme::{SPACE_S, SPACE_XS};

pub const HEART_CONTAINER_PATH: &str = "./graphics/hud/heart_container.png";
pub const HEART_FULL_PATH: &str = "./graphics/hud/heart_full.png";
pub const HEART_HALF_PATH: &str = "./graphics/hud/heart_half.png";

/// The side of a heart on screen, in logical px: twice its texture, like the item icons.
const HEART_SIZE: f32 = 18.0;

/// The hearts: one for every 2 points of health.
const HEARTS: u32 = MAX_HEALTH / 2;

/// The height the hearts take above the hotbar, in logical px: their margin and a heart.
pub const HEALTH_BAND: f32 = SPACE_XS + HEART_SIZE;

/// The row of hearts, shown in survival only.
#[derive(Component)]
pub struct HealthBar;

/// The fill of the heart `index` of the row, over its empty container.
#[derive(Component)]
pub struct HeartFill {
    index: u32,
}

/// How much of a heart is filled.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Heart {
    Empty,
    Half,
    Full,
}

/// The heart `index` of the row for `health` points, from the left.
pub fn heart(health: u32, index: u32) -> Heart {
    match health.saturating_sub(index * 2) {
        0 => Heart::Empty,
        1 => Heart::Half,
        _ => Heart::Full,
    }
}

/// The hearts as the accessibility tree and the tests read them.
pub fn health_label(health: u32) -> String {
    let hearts = health as f32 / 2.0;
    format!("Health: {hearts} of {HEARTS} hearts")
}

/// Spawns the hearts over the hotbar, hidden until the local player is known to be in survival.
pub fn spawn_health_bar(
    mut commands: Commands,
    hotbar: Query<Entity, With<Hotbar>>,
    assets: Res<AssetServer>,
) {
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    let container: Handle<Image> = assets.load(HEART_CONTAINER_PATH);
    let full: Handle<Image> = assets.load(HEART_FULL_PATH);
    let row = commands
        .spawn((
            HealthBar,
            Name::new("Health"),
            AccessibleLabel::new(health_label(MAX_HEALTH)),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.),
                bottom: percent(100.),
                margin: UiRect::bottom(px(SPACE_XS)),
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(hotbar),
        ))
        .id();
    for index in 0..HEARTS {
        commands.spawn((
            ImageNode::new(container.clone()),
            Node {
                width: px(HEART_SIZE),
                height: px(HEART_SIZE),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
            children![(
                HeartFill { index },
                ImageNode::new(full.clone()),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.),
                    height: percent(100.),
                    ..default()
                },
                Pickable::IGNORE,
            )],
        ));
    }
}

/// Fills the hearts with the health of the local player, and shows them in survival only, while
/// the inventory is closed: open, its backpack takes their place, as in Minecraft. Writes nothing
/// while none of these changes.
pub fn update_health_bar(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut bar: Query<(Entity, &mut Visibility), With<HealthBar>>,
    mut fills: Query<(&HeartFill, &mut ImageNode, &mut Visibility), Without<HealthBar>>,
    inventory: Query<&Visibility, (With<InventoryRoot>, Without<HealthBar>, Without<HeartFill>)>,
    assets: Res<AssetServer>,
    mut shown: Local<Option<(u32, GameMode, bool)>>,
    mut commands: Commands,
) {
    let (Ok(player), Ok((bar, mut visibility))) = (player.single(), bar.single_mut()) else {
        return;
    };
    let inventory_open = inventory
        .single()
        .is_ok_and(|inventory| *inventory == Visibility::Visible);
    let state = (player.health, player.game_mode, inventory_open);
    if *shown == Some(state) {
        return;
    }
    *shown = Some(state);

    *visibility = if player.game_mode.takes_damage() && !inventory_open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    commands
        .entity(bar)
        .insert(AccessibleLabel::new(health_label(player.health)));
    for (fill, mut image, mut visibility) in &mut fills {
        let path = match heart(player.health, fill.index) {
            Heart::Empty => {
                *visibility = Visibility::Hidden;
                continue;
            }
            Heart::Half => HEART_HALF_PATH,
            Heart::Full => HEART_FULL_PATH,
        };
        image.image = assets.load(path);
        *visibility = Visibility::Inherited;
    }
}

/// Raises the name of the selected item over the hearts while they show.
pub fn raise_item_name_over_the_hearts(
    bar: Query<&Visibility, (With<HealthBar>, Changed<Visibility>)>,
    mut name: Query<&mut Node, With<HotbarItemName>>,
) {
    let (Ok(visibility), Ok(mut node)) = (bar.single(), name.single_mut()) else {
        return;
    };
    let band = if *visibility == Visibility::Hidden {
        0.0
    } else {
        HEALTH_BAND
    };
    node.margin = UiRect::bottom(px(SPACE_S + band));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_heart_holds_two_points_from_the_left() {
        let row = |health| {
            (0..HEARTS)
                .map(|index| heart(health, index))
                .collect::<Vec<_>>()
        };
        assert!(row(MAX_HEALTH).iter().all(|&h| h == Heart::Full));
        assert!(row(0).iter().all(|&h| h == Heart::Empty));
        let seven = row(7);
        assert_eq!(
            &seven[..4],
            [Heart::Full, Heart::Full, Heart::Full, Heart::Half]
        );
        assert!(seven[4..].iter().all(|&h| h == Heart::Empty));
        assert_eq!(health_label(7), "Health: 3.5 of 10 hearts");
        assert_eq!(health_label(MAX_HEALTH), "Health: 10 of 10 hearts");
    }
}
