//! The food of the local player in survival: ten drumsticks over the right of the hotbar,
//! mirroring the hearts and filled from the right, each one full, half or empty. The textures
//! are Pixel Perfection's `gui/sprites/hud/food_*.png`.

use bevy::prelude::*;
use shared::players::hunger::{drumstick, Drumstick, DRUMSTICKS, MAX_FOOD};
use shared::players::Player;

use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::InventoryRoot;
use crate::ui::theme::SPACE_XS;

pub const FOOD_EMPTY_PATH: &str = "./graphics/hud/food_empty.png";
pub const FOOD_FULL_PATH: &str = "./graphics/hud/food_full.png";
pub const FOOD_HALF_PATH: &str = "./graphics/hud/food_half.png";

/// The side of a drumstick on screen, in logical px: twice its texture, like the hearts.
const DRUMSTICK_SIZE: f32 = 18.0;

/// The height the drumsticks take above the hotbar, in logical px: their margin and a
/// drumstick. The air bubbles sit on top of it.
pub const FOOD_BAND: f32 = SPACE_XS + DRUMSTICK_SIZE;

/// The row of drumsticks, shown in survival only.
#[derive(Component)]
pub struct FoodBar;

/// The fill of the drumstick `index` of the row, from the right, over its empty one.
#[derive(Component)]
pub struct DrumstickFill {
    index: u8,
}

/// The drumsticks as the accessibility tree and the tests read them.
pub fn food_label(food: u8) -> String {
    let drumsticks = food as f32 / 2.0;
    format!("Food: {drumsticks} of {DRUMSTICKS} drumsticks")
}

/// Spawns the drumsticks over the hotbar, hidden until the local player is known to be in
/// survival.
pub fn spawn_food_bar(
    mut commands: Commands,
    hotbar: Query<Entity, With<Hotbar>>,
    assets: Res<AssetServer>,
) {
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    let empty: Handle<Image> = assets.load(FOOD_EMPTY_PATH);
    let full: Handle<Image> = assets.load(FOOD_FULL_PATH);
    let row = commands
        .spawn((
            FoodBar,
            Name::new("Food"),
            AccessibleLabel::new(food_label(MAX_FOOD)),
            Node {
                position_type: PositionType::Absolute,
                right: px(0.),
                bottom: percent(100.),
                margin: UiRect::bottom(px(SPACE_XS)),
                // The first drumstick on the right
                flex_direction: FlexDirection::RowReverse,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(hotbar),
        ))
        .id();
    for index in 0..DRUMSTICKS {
        commands.spawn((
            ImageNode::new(empty.clone()),
            Node {
                width: px(DRUMSTICK_SIZE),
                height: px(DRUMSTICK_SIZE),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
            children![(
                DrumstickFill { index },
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

/// Fills the drumsticks with the food of the local player, and shows them as the hearts: in
/// survival only, while the inventory is closed. Writes nothing while none of these changes.
pub fn update_food_bar(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut bar: Query<(Entity, &mut Visibility), With<FoodBar>>,
    mut fills: Query<(&DrumstickFill, &mut ImageNode, &mut Visibility), Without<FoodBar>>,
    inventory: Query<
        &Visibility,
        (
            With<InventoryRoot>,
            Without<FoodBar>,
            Without<DrumstickFill>,
        ),
    >,
    assets: Res<AssetServer>,
    mut shown: Local<Option<(u8, bool)>>,
    mut commands: Commands,
) {
    let (Ok(player), Ok((bar, mut visibility))) = (player.single(), bar.single_mut()) else {
        return;
    };
    let inventory_open = inventory
        .single()
        .is_ok_and(|inventory| *inventory == Visibility::Visible);
    let visible = player.game_mode.takes_damage() && !inventory_open;
    let state = (player.food, visible);
    if *shown == Some(state) {
        return;
    }
    *shown = Some(state);

    *visibility = if visible {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    commands
        .entity(bar)
        .insert(AccessibleLabel::new(food_label(player.food)));
    for (fill, mut image, mut visibility) in &mut fills {
        let path = match drumstick(player.food, fill.index) {
            Drumstick::Empty => {
                *visibility = Visibility::Hidden;
                continue;
            }
            Drumstick::Half => FOOD_HALF_PATH,
            Drumstick::Full => FOOD_FULL_PATH,
        };
        image.image = assets.load(path);
        *visibility = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_counts_the_drumsticks_by_halves() {
        assert_eq!(food_label(MAX_FOOD), "Food: 10 of 10 drumsticks");
        assert_eq!(food_label(7), "Food: 3.5 of 10 drumsticks");
        assert_eq!(food_label(0), "Food: 0 of 10 drumsticks");
    }
}
