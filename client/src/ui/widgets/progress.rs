//! The progress bar: a sunken track filled from the left as far as its value, which its screen
//! sets on `ProgressBar`.

use accesskit::Role;
use bevy::{a11y::AccessibilityNode, prelude::*, ui::UiSystems};

use super::sunken_bevel;
use crate::ui::theme::{BEVEL_WIDTH, OK, SLOT};

/// The height of a progress bar, its bevel included.
const PROGRESS_HEIGHT: f32 = 12.0;

/// The value of a progress bar, 0 to 1, set by its screen.
#[derive(Component)]
pub struct ProgressBar(pub f32);

/// The fill of a progress bar, its only child.
#[derive(Component)]
struct ProgressFill;

/// A 12 px sunken `SLOT` track with an `OK` fill, `Role::ProgressIndicator` labelled `label`.
pub fn progress_bar(label: &str) -> impl Bundle {
    let mut node = accesskit::Node::new(Role::ProgressIndicator);
    node.set_min_numeric_value(0.0);
    node.set_max_numeric_value(100.0);
    (
        ProgressBar(0.0),
        // the role before the label: the label's default node must not win
        AccessibilityNode(node),
        AccessibleLabel::new(label),
        Node {
            height: px(PROGRESS_HEIGHT),
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(SLOT),
        sunken_bevel(),
        children![(
            ProgressFill,
            Node {
                width: percent(0),
                height: percent(100),
                ..default()
            },
            BackgroundColor(OK),
        )],
    )
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(PostUpdate, fill_progress_bars.before(UiSystems::Layout));
}

/// Fills each changed progress bar as far as its value, and tells it as a percentage; a value
/// out of 0 to 1 is clamped.
fn fill_progress_bars(
    mut bars: Query<(&ProgressBar, &mut AccessibilityNode, &Children), Changed<ProgressBar>>,
    mut fills: Query<&mut Node, With<ProgressFill>>,
) {
    for (bar, mut accessible, children) in &mut bars {
        let shown = bar.0.clamp(0.0, 1.0) * 100.0;
        for &child in children {
            if let Ok(mut fill) = fills.get_mut(child) {
                fill.width = percent(shown);
            }
        }
        accessible.set_numeric_value(shown as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fill_follows_the_value() {
        let mut app = App::new();
        app.add_plugins(plugin);
        let bar = app.world_mut().spawn(progress_bar("Progress")).id();
        let fill_width = |app: &mut App| {
            let fill = app.world().get::<Children>(bar).unwrap()[0];
            app.world().get::<Node>(fill).unwrap().width
        };
        let value = |app: &App| {
            let node = app.world().get::<AccessibilityNode>(bar).unwrap();
            assert_eq!(node.role(), Role::ProgressIndicator);
            assert_eq!(node.label(), Some("Progress"));
            node.numeric_value()
        };
        for (set, shown) in [(0.25, 25.0), (1.5, 100.0), (-1.0, 0.0)] {
            app.world_mut().get_mut::<ProgressBar>(bar).unwrap().0 = set;
            app.update();
            assert_eq!(fill_width(&mut app), percent(shown), "{set}");
            assert_eq!(value(&app), Some(shown), "{set}");
        }
    }
}
