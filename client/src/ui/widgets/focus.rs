//! Focus: Tab navigation, the amber focus ring and arrow navigation inside a container.

use bevy::{
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        directional_navigation::{
            AutoNavigationConfig, DirectionalNavigationPlugin, FocusableArea,
        },
        navigator::find_best_candidate,
        tab_navigation::{TabIndex, TabNavigationPlugin},
        FocusCause, FocusedInput, InputFocus, InputFocusVisible, IsFocused, IsFocusedHelper,
    },
    math::CompassOctant,
    prelude::*,
};

use crate::ui::theme::{ACCENT, FOCUS_RING_OFFSET, FOCUS_RING_WIDTH};

/// A ring around the entity while the focus is on it or on one of its descendants and the focus
/// is visible (after a key, not after a click). The ring is the `Outline`, whose colour is
/// toggled, because Bevy asks not to add and remove the component.
#[derive(Component)]
#[require(Outline = Outline {
    width: px(FOCUS_RING_WIDTH),
    offset: px(FOCUS_RING_OFFSET),
    color: Color::NONE,
})]
pub struct FocusRing;

/// A container whose focusable descendants (`TabIndex >= 0`) the arrow keys move between, by
/// their position on screen. It is for grids and lists; Tab still walks them in order.
// Nothing uses it before the first screens with a grid (lot 1).
#[allow(dead_code)]
#[derive(Component)]
pub struct ArrowNav;

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((TabNavigationPlugin, DirectionalNavigationPlugin))
        .add_systems(PostUpdate, show_focus_ring)
        .add_observer(arrow_nav);
}

/// Colours the ring of each `FocusRing` according to the focus.
fn show_focus_ring(
    mut rings: Query<(Entity, &mut Outline), With<FocusRing>>,
    focus: IsFocusedHelper,
) {
    for (entity, mut outline) in &mut rings {
        let color = if focus.is_focus_within_visible(entity) {
            ACCENT
        } else {
            Color::NONE
        };
        if outline.color != color {
            outline.color = color;
        }
    }
}

/// Moves the focus with the arrow keys between the focusable descendants of the `ArrowNav`
/// container being visited by the event. A field inside the container keeps its own arrows,
/// because `EditableText` stops them before they bubble up here.
fn arrow_nav(
    mut event: On<FocusedInput<KeyboardInput>>,
    containers: Query<(), With<ArrowNav>>,
    children: Query<&Children>,
    items: Query<(
        &TabIndex,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
    )>,
    config: Res<AutoNavigationConfig>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    // the event target is the entity being visited by the bubbling, not the focused one
    if !containers.contains(event.focused_entity) {
        return;
    }
    if event.input.state != ButtonState::Pressed {
        return;
    }
    let direction = match event.input.key_code {
        KeyCode::ArrowUp => CompassOctant::North,
        KeyCode::ArrowDown => CompassOctant::South,
        KeyCode::ArrowLeft => CompassOctant::West,
        KeyCode::ArrowRight => CompassOctant::East,
        _ => return,
    };
    let area = |entity: Entity| {
        let (index, node, transform, visibility) = items.get(entity).ok()?;
        (index.0 >= 0 && !node.is_empty() && visibility.get()).then(|| FocusableArea {
            entity,
            position: transform.translation * node.inverse_scale_factor(),
            size: node.size() * node.inverse_scale_factor(),
        })
    };
    let Some(origin) = area(event.original_event_target()) else {
        return;
    };
    let candidates: Vec<_> = children
        .iter_descendants(event.focused_entity)
        .filter_map(area)
        .collect();
    if let Some(next) = find_best_candidate(&origin, direction, &candidates, &config) {
        event.propagate(false);
        focus.set(next, FocusCause::Navigated);
        visible.0 = true;
    }
}

#[cfg(test)]
mod tests {
    use bevy::input_focus::{InputFocus, InputFocusVisible};

    use super::*;
    use crate::ui::theme::ACCENT;

    fn ring_color(app: &App, entity: Entity) -> Color {
        app.world().get::<Outline>(entity).unwrap().color
    }

    #[test]
    fn the_ring_shows_on_the_focus_or_its_container_only_while_visible() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<InputFocus>()
            .insert_resource(InputFocusVisible(true))
            .add_systems(PostUpdate, show_focus_ring);
        let parent = app.world_mut().spawn(FocusRing).id();
        let child = app.world_mut().spawn(ChildOf(parent)).id();
        let other = app.world_mut().spawn_empty().id();

        app.insert_resource(InputFocus::from_entity(child));
        app.update();
        assert_eq!(ring_color(&app, parent), ACCENT);

        app.world_mut().resource_mut::<InputFocusVisible>().0 = false;
        app.update();
        assert_eq!(ring_color(&app, parent), Color::NONE);

        app.world_mut().resource_mut::<InputFocusVisible>().0 = true;
        app.insert_resource(InputFocus::from_entity(other));
        app.update();
        assert_eq!(ring_color(&app, parent), Color::NONE);
    }
}
