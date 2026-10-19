//! Toasts: short messages at the top-right of the window, which go by themselves after
//! `TOAST_DURATION`. They stack downward, below what holds the top-right corner of the game
//! (`ToastsBelow`), take no click and no focus, and outlive the screen that showed them.

use std::time::Duration;

use accesskit::Role;
use bevy::{a11y::AccessibilityNode, prelude::*, ui::UiSystems};

use super::raised_bevel;
use crate::ui::theme::{BEVEL_WIDTH, DANGER, FONT_S, OK, PANEL, SPACE_L, SPACE_M, SPACE_S, TEXT};

/// How long a toast shows.
pub const TOAST_DURATION: Duration = Duration::from_secs(3);

/// Over the screens and their overlays, under the dialogs (`DIALOG_Z`).
pub const TOAST_Z: i32 = 40;

/// The widest a toast grows before its text wraps, in logical px.
const TOAST_MAX_WIDTH: f32 = 360.0;

/// The width of the coloured left edge of a toast, in logical px.
const TOAST_EDGE: f32 = 4.0;

/// The most toasts shown at once: a new one beyond them takes the place of the oldest.
const MAX_TOASTS: usize = 3;

/// What a toast tells: news, or a failure.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToastKind {
    Info,
    Error,
}

/// A toast, despawned once `Time<Real>` reaches `until`.
#[derive(Component)]
pub struct Toast {
    pub until: Duration,
}

/// The column at the top-right of the window that holds the toasts, spawned with the first one.
#[derive(Component)]
struct ToastStack;

/// A node at the top-right of the window that the toasts stay below.
#[derive(Component)]
pub struct ToastsBelow;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, despawn_old_toasts)
        .add_systems(PostUpdate, keep_toasts_below.before(UiSystems::Layout));
}

/// Shows `text` in a toast under the ones already shown, for `TOAST_DURATION`. The oldest goes
/// when there would be more than `MAX_TOASTS`.
pub fn show_toast(
    commands: &mut Commands,
    text: &str,
    kind: ToastKind,
    font: Handle<Font>,
) -> Entity {
    let (role, edge) = match kind {
        ToastKind::Info => (Role::Status, OK),
        ToastKind::Error => (Role::Alert, DANGER),
    };
    let toast = commands
        .spawn((
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(role)),
            AccessibleLabel::new(text),
            Node {
                max_width: px(TOAST_MAX_WIDTH),
                padding: UiRect::axes(px(SPACE_M), px(SPACE_S)),
                border: UiRect {
                    left: px(TOAST_EDGE),
                    ..UiRect::all(px(BEVEL_WIDTH))
                },
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor {
                left: edge,
                ..raised_bevel()
            },
            Pickable::IGNORE,
            children![(
                Text::new(text),
                TextFont {
                    font: font.into(),
                    font_size: px(FONT_S).into(),
                    ..default()
                },
                TextColor(TEXT),
                TextLayout::linebreak(LineBreak::WordOrCharacter),
                Pickable::IGNORE,
            )],
        ))
        .id();
    commands.queue(move |world: &mut World| {
        let now = world
            .get_resource::<Time<Real>>()
            .map(Time::elapsed)
            .unwrap_or_default();
        let stack = world
            .query_filtered::<Entity, With<ToastStack>>()
            .iter(world)
            .next();
        let stack = stack.unwrap_or_else(|| world.spawn(toast_stack()).id());
        if let Ok(mut toast) = world.get_entity_mut(toast) {
            toast.insert((
                Toast {
                    until: now + TOAST_DURATION,
                },
                ChildOf(stack),
            ));
        }
        let shown: Vec<Entity> = world
            .get::<Children>(stack)
            .map(|toasts| toasts.to_vec())
            .unwrap_or_default();
        for &old in &shown[..shown.len().saturating_sub(MAX_TOASTS)] {
            world.despawn(old);
        }
    });
    toast
}

/// The column of the toasts: top-right, the newest at the bottom.
fn toast_stack() -> impl Bundle {
    (
        ToastStack,
        Node {
            position_type: PositionType::Absolute,
            top: px(SPACE_L),
            right: px(SPACE_L),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: px(SPACE_S),
            ..default()
        },
        GlobalZIndex(TOAST_Z),
        Pickable::IGNORE,
    )
}

/// The top of the toast stack, in logical px: `SPACE_L` from the top of the window, or under
/// the bottoms of the nodes it keeps below, also in logical px, when lower.
fn toast_top(bottoms: impl IntoIterator<Item = f32>) -> f32 {
    bottoms
        .into_iter()
        .map(|bottom| bottom + SPACE_S)
        .fold(SPACE_L, f32::max)
}

/// Moves the toast stack below the nodes it keeps below, as they were laid out last frame.
fn keep_toasts_below(
    below: Query<(&ComputedNode, &UiGlobalTransform, &InheritedVisibility), With<ToastsBelow>>,
    mut stacks: Query<&mut Node, With<ToastStack>>,
) {
    let bottoms = below
        .iter()
        .filter(|(node, _, visibility)| visibility.get() && node.size().y > 0.0)
        .map(|(node, transform, _)| {
            (transform.translation.y + node.size().y / 2.0) * node.inverse_scale_factor()
        });
    let top = px(toast_top(bottoms));
    for mut stack in &mut stacks {
        if stack.top != top {
            stack.top = top;
        }
    }
}

/// Despawns the toasts whose time is up.
fn despawn_old_toasts(
    toasts: Query<(Entity, &Toast)>,
    time: Res<Time<Real>>,
    mut commands: Commands,
) {
    for (toast, Toast { until }) in &toasts {
        if time.elapsed() >= *until {
            commands.entity(toast).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;
    use bevy::state::app::StatesPlugin;

    use super::*;
    use crate::state::GameState;

    #[test]
    fn the_toasts_stay_below_the_top_right_corner_of_the_game() {
        assert_eq!(toast_top([]), SPACE_L);
        // a node that ends higher leaves the toasts where they are
        assert_eq!(toast_top([4.0]), SPACE_L);
        assert_eq!(toast_top([170.0, 40.0]), 170.0 + SPACE_S);
    }

    #[test]
    fn a_toast_has_the_role_of_its_kind() {
        for (kind, role) in [
            (ToastKind::Info, Role::Status),
            (ToastKind::Error, Role::Alert),
        ] {
            let mut world = World::new();
            let toast = world
                .run_system_once(move |mut commands: Commands| {
                    show_toast(&mut commands, "Hello", kind, Handle::default())
                })
                .unwrap();
            let node = world.get::<AccessibilityNode>(toast).unwrap();
            assert_eq!(node.role(), role, "{kind:?}");
            assert_eq!(node.label(), Some("Hello"), "{kind:?}");
        }
    }

    /// A toast shown in the game, as "Could not save the world" as the world is left, is still
    /// read in the menus.
    #[test]
    fn a_toast_outlives_the_game_that_showed_it() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .insert_state(GameState::Game)
            .add_plugins(plugin);
        app.update();
        let toast = app
            .world_mut()
            .run_system_once(|mut commands: Commands| {
                show_toast(&mut commands, "Hello", ToastKind::Error, Handle::default())
            })
            .unwrap();
        app.world_mut()
            .resource_mut::<NextState<GameState>>()
            .set(GameState::Menu);
        app.update();
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Menu
        );
        assert!(app.world().get::<Toast>(toast).is_some());
    }

    #[test]
    fn at_most_three_toasts_show_and_the_oldest_goes() {
        let mut world = World::new();
        let toasts: Vec<Entity> = ["One", "Two", "Three", "Four"]
            .into_iter()
            .map(|text| {
                world
                    .run_system_once(move |mut commands: Commands| {
                        show_toast(&mut commands, text, ToastKind::Info, Handle::default())
                    })
                    .unwrap()
            })
            .collect();

        assert!(world.get_entity(toasts[0]).is_err());
        let stack = world
            .query_filtered::<&Children, With<ToastStack>>()
            .single(&world)
            .unwrap();
        assert_eq!(stack.to_vec(), toasts[1..]);
    }
}
