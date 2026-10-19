use bevy::prelude::*;

use super::InventoryRoot;
use crate::state::GameState;
use crate::ui::menus::pause::{pause_open, PauseMenu};
use crate::ui::menus::settings::{settings_open, SettingsRoot};
use crate::ui::theme::{BEVEL_LO, RETICLE};

/// The length of a bar of the reticle, in logical px.
const BAR_LENGTH: f32 = 20.0;
/// The thickness of a bar of the reticle.
const BAR_THICKNESS: f32 = 2.0;
/// The width of the dark outline around each bar, on every side.
const OUTLINE: f32 = 1.0;

/// The root of the reticle.
#[derive(Component)]
pub struct Reticle;

/// A bar of the reticle, `width` by `height` px, centred on the middle of the screen.
fn bar(width: f32, height: f32, color: Color) -> impl Bundle {
    (
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(width),
            height: Val::Px(height),
            left: Val::Px(-width / 2.0),
            top: Val::Px(-height / 2.0),
            ..default()
        },
        BackgroundColor(color),
        Pickable::IGNORE,
    )
}

/// The cross in the middle of the screen: white bars on a dark outline, to read over any
/// terrain. It lets the clicks through.
pub fn spawn_reticle(mut commands: Commands) {
    // Main container for the reticle
    commands
        .spawn((
            Reticle,
            DespawnOnExit(GameState::Game), // Link the reticle to the Game state
            Node {
                position_type: PositionType::Absolute,
                margin: UiRect {
                    left: Val::Auto,
                    right: Val::Auto,
                    top: Val::Auto,
                    bottom: Val::Auto,
                },
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|parent| {
            // The outline first, so that the bars are drawn over it
            let (long, thick) = (BAR_LENGTH + 2.0 * OUTLINE, BAR_THICKNESS + 2.0 * OUTLINE);
            parent.spawn(bar(long, thick, BEVEL_LO));
            parent.spawn(bar(thick, long, BEVEL_LO));
            parent.spawn(bar(BAR_LENGTH, BAR_THICKNESS, RETICLE));
            parent.spawn(bar(BAR_THICKNESS, BAR_LENGTH, RETICLE));
        });
}

/// Hides the reticle while the inventory, the pause menu or the settings are open, from the
/// frame they show to the frame they are gone: there is nothing to aim at, and it would show
/// through the panel. The chat leaves it.
pub fn show_reticle(
    inventory: Query<&Visibility, (With<InventoryRoot>, Without<Reticle>)>,
    menus: Query<(), With<PauseMenu>>,
    settings: Query<(), With<SettingsRoot>>,
    mut reticle: Query<&mut Visibility, With<Reticle>>,
) {
    let inventory_open = inventory.iter().any(|v| *v == Visibility::Visible);
    let shown = if inventory_open || pause_open(menus) || settings_open(settings) {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    };
    for mut visibility in &mut reticle {
        visibility.set_if_neq(shown);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::BEVEL_LO;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn the_reticle_has_a_dark_outline_and_lets_clicks_through() {
        let mut world = World::new();
        world.run_system_once(spawn_reticle).unwrap();

        let mut container = world.query_filtered::<&Children, Without<ChildOf>>();
        let bars: Vec<Entity> = container.single(&world).unwrap().iter().collect();
        let layout: Vec<((f32, f32), bool)> = bars
            .iter()
            .map(|&bar| {
                let node = world.get::<Node>(bar).unwrap();
                let (Val::Px(width), Val::Px(height)) = (node.width, node.height) else {
                    panic!("a bar has a size in px");
                };
                let color = world.get::<BackgroundColor>(bar).unwrap().0;
                ((width, height), color == BEVEL_LO)
            })
            .collect();
        assert_eq!(
            layout,
            [
                ((22., 4.), true),
                ((4., 22.), true),
                ((20., 2.), false),
                ((2., 20.), false)
            ]
        );

        let mut nodes = world.query::<(&Node, Option<&Pickable>)>();
        for (_, pickable) in nodes.iter(&world) {
            assert_eq!(pickable, Some(&Pickable::IGNORE));
        }
    }
}
