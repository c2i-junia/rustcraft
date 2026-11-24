//! The air of the local player in survival: ten bubbles over the right of the hotbar, mirroring
//! the hearts, shown while the air is not full. They go from the left, the last one bursting
//! before it goes. The textures are Pixel Perfection's `gui/sprites/hud/air*.png`.

use bevy::prelude::*;
use shared::players::air::{bubble, Bubble, BUBBLES, MAX_AIR};
use shared::players::Player;

use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::hud::InventoryRoot;
use crate::ui::theme::SPACE_XS;

pub const AIR_PATH: &str = "./graphics/hud/air.png";
pub const AIR_BURSTING_PATH: &str = "./graphics/hud/air_bursting.png";

/// The side of a bubble on screen, in logical px: twice its texture, like the hearts.
const BUBBLE_SIZE: f32 = 18.0;

/// The row of bubbles. Lot 6's food row goes under it: its bottom margin then grows.
#[derive(Component)]
pub struct AirBar;

/// The bubble `index` of the row, from the right.
#[derive(Component)]
pub struct AirBubble {
    index: u16,
}

/// The bubbles as the accessibility tree and the tests read them.
pub fn air_label(air: u16) -> String {
    let shown = (0..BUBBLES)
        .filter(|&index| bubble(air, index) != Bubble::Empty)
        .count();
    format!("Air: {shown} of {BUBBLES} bubbles")
}

/// Spawns the bubbles over the hotbar, hidden until the local player runs out of breath.
pub fn spawn_air_bar(
    mut commands: Commands,
    hotbar: Query<Entity, With<Hotbar>>,
    assets: Res<AssetServer>,
) {
    let Ok(hotbar) = hotbar.single() else {
        return;
    };
    let full: Handle<Image> = assets.load(AIR_PATH);
    let row = commands
        .spawn((
            AirBar,
            Name::new("Air"),
            AccessibleLabel::new(air_label(MAX_AIR)),
            Node {
                position_type: PositionType::Absolute,
                right: px(0.),
                bottom: percent(100.),
                margin: UiRect::bottom(px(SPACE_XS)),
                // The first bubble on the right
                flex_direction: FlexDirection::RowReverse,
                ..default()
            },
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(hotbar),
        ))
        .id();
    for index in 0..BUBBLES {
        commands.spawn((
            AirBubble { index },
            ImageNode::new(full.clone()),
            Node {
                width: px(BUBBLE_SIZE),
                height: px(BUBBLE_SIZE),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(row),
        ));
    }
}

/// Shows the air of the local player while it is not full, in survival only and while the
/// inventory is closed, as the hearts. Writes nothing while none of these changes.
pub fn update_air_bar(
    player: Query<&Player, With<CurrentPlayerMarker>>,
    mut bar: Query<(Entity, &mut Visibility), With<AirBar>>,
    mut bubbles: Query<(&AirBubble, &mut ImageNode, &mut Visibility), Without<AirBar>>,
    inventory: Query<&Visibility, (With<InventoryRoot>, Without<AirBar>, Without<AirBubble>)>,
    assets: Res<AssetServer>,
    mut shown: Local<Option<(u16, bool)>>,
    mut commands: Commands,
) {
    let (Ok(player), Ok((bar, mut visibility))) = (player.single(), bar.single_mut()) else {
        return;
    };
    let inventory_open = inventory
        .single()
        .is_ok_and(|inventory| *inventory == Visibility::Visible);
    let visible = player.game_mode.takes_damage() && !inventory_open && player.air < MAX_AIR;
    let state = (player.air, visible);
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
        .insert(AccessibleLabel::new(air_label(player.air)));
    for (bubble_node, mut image, mut visibility) in &mut bubbles {
        let path = match bubble(player.air, bubble_node.index) {
            Bubble::Empty => {
                *visibility = Visibility::Hidden;
                continue;
            }
            Bubble::Bursting => AIR_BURSTING_PATH,
            Bubble::Full => AIR_PATH,
        };
        image.image = assets.load(path);
        *visibility = Visibility::Inherited;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_label_counts_the_bubbles_drawn() {
        assert_eq!(air_label(MAX_AIR), "Air: 10 of 10 bubbles");
        assert_eq!(air_label(270), "Air: 9 of 10 bubbles");
        // A bursting bubble still counts
        assert_eq!(air_label(1), "Air: 1 of 10 bubbles");
        assert_eq!(air_label(0), "Air: 0 of 10 bubbles");
    }
}
