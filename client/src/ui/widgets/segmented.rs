//! The segmented control: a row of mutually exclusive options in a sunken frame, on a
//! `RadioGroup`. The tab bar of `tabs.rs` shares its wiring.
//!
//! The group takes the focus and the arrow keys; callers observe `On<SegmentChange>` on it.

use bevy::{
    ecs::entity::EntityHashSet,
    input_focus::tab_navigation::TabIndex,
    picking::hover::Hovered,
    prelude::*,
    ui::Checked,
    ui_widgets::{RadioButton, RadioGroup, ValueChange},
};

use super::{focus::FocusRing, sunken_bevel};
use crate::ui::theme::{
    ACCENT, BEVEL_WIDTH, FONT_M, PANEL, RAISE_HOVER, SLOT, SPACE_M, SPACE_XS, TEXT,
};

/// What `spawn_segmented` builds.
pub struct Segmented<'a> {
    /// The accessible label of the group.
    pub label: &'a str,
    pub options: &'a [&'a str],
    /// The index of the option checked at first.
    pub selected: usize,
}

/// The option `index` of a segmented control.
#[derive(Component)]
pub struct Segment(pub usize);

/// Triggered on the group of a segmented control when the player checks another option.
#[derive(EntityEvent)]
pub struct SegmentChange {
    pub entity: Entity,
    pub index: usize,
}

/// Spawns a segmented control, without a parent, and returns its group.
pub fn spawn_segmented(
    commands: &mut Commands,
    segmented: &Segmented,
    font: Handle<Font>,
) -> Entity {
    let group = commands
        .spawn((
            // the widget before the label: its `Role::RadioGroup` must win over the label's node
            RadioGroup,
            AccessibleLabel::new(segmented.label),
            TabIndex(0),
            FocusRing,
            // as wide as its options, even in a column
            Node {
                align_self: AlignSelf::FlexStart,
                border: UiRect::all(px(BEVEL_WIDTH)),
                padding: UiRect::all(px(SPACE_XS)),
                column_gap: px(SPACE_XS),
                ..default()
            },
            BackgroundColor(SLOT),
            sunken_bevel(),
        ))
        .observe(choose_segment)
        .id();
    for (index, option) in segmented.options.iter().enumerate() {
        let checked = index == segmented.selected;
        let look = Segment::look(checked, false);
        let mut segment = commands.spawn((
            RadioButton,
            AccessibleLabel::new(*option),
            Segment(index),
            Hovered::default(),
            Node {
                padding: UiRect::axes(px(SPACE_M), px(SPACE_XS)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(look.background),
            look.border,
            ChildOf(group),
            children![(
                Text::new(*option),
                TextFont {
                    font: font.clone().into(),
                    font_size: px(FONT_M).into(),
                    ..default()
                },
                TextColor(look.text),
            )],
        ));
        if checked {
            segment.insert(Checked);
        }
    }
    group
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, style_options::<Segment>);
}

/// Checks the chosen segment and tells the callers.
fn choose_segment(
    change: On<ValueChange<Entity>>,
    children: Query<&Children>,
    segments: Query<&Segment>,
    mut commands: Commands,
) {
    if let Some(index) = check_chosen(&change, &children, &segments, &mut commands) {
        commands.trigger(SegmentChange {
            entity: change.source,
            index,
        });
    }
}

/// An option of a `RadioGroup` drawn by this module: a segment or a tab.
pub(super) trait ChoiceOption: Component {
    fn index(&self) -> usize;
    fn look(checked: bool, hovered: bool) -> OptionLook;
}

/// The colours of an option in a given state.
pub(super) struct OptionLook {
    pub background: Color,
    pub border: BorderColor,
    pub text: Color,
}

impl ChoiceOption for Segment {
    fn index(&self) -> usize {
        self.0
    }

    fn look(checked: bool, hovered: bool) -> OptionLook {
        let (background, text) = if checked {
            (ACCENT, PANEL)
        } else if hovered {
            (RAISE_HOVER, TEXT)
        } else {
            (Color::NONE, TEXT)
        };
        OptionLook {
            background,
            border: BorderColor::all(Color::NONE),
            text,
        }
    }
}

/// Checks the option the group of `change` chose, unchecks its other options, and returns the
/// index of the chosen one.
pub(super) fn check_chosen<C: ChoiceOption>(
    change: &ValueChange<Entity>,
    children: &Query<&Children>,
    options: &Query<&C>,
    commands: &mut Commands,
) -> Option<usize> {
    for option in children.iter_descendants(change.source) {
        if !options.contains(option) {
            continue;
        }
        if option == change.value {
            commands.entity(option).insert(Checked);
        } else {
            commands.entity(option).remove::<Checked>();
        }
    }
    options.get(change.value).ok().map(C::index)
}

type OptionChanged = Or<(Changed<Hovered>, Added<Checked>)>;

/// Applies the look of each option of type `C` whose state changed to itself and to its label.
pub(super) fn style_options<C: ChoiceOption>(
    changed: Query<Entity, (With<C>, OptionChanged)>,
    mut unchecked: RemovedComponents<Checked>,
    mut options: Query<
        (
            &Hovered,
            Has<Checked>,
            &mut BackgroundColor,
            &mut BorderColor,
            &Children,
        ),
        With<C>,
    >,
    mut texts: Query<&mut TextColor>,
) {
    let entities: EntityHashSet = changed.iter().chain(unchecked.read()).collect();
    for entity in entities {
        let Ok((hovered, checked, mut background, mut border, children)) = options.get_mut(entity)
        else {
            continue;
        };
        let look = C::look(checked, hovered.0);
        background.0 = look.background;
        *border = look.border;
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.0 = look.text;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use accesskit::Role;
    use bevy::a11y::AccessibilityNode;

    use super::*;

    #[test]
    fn a_segmented_control_is_a_radio_group_of_radio_buttons() {
        let mut world = World::new();
        let segmented = Segmented {
            label: "UI scale",
            options: &["Auto", "1"],
            selected: 0,
        };
        let group = spawn_segmented(&mut world.commands(), &segmented, Handle::default());
        world.flush();
        let node = world.get::<AccessibilityNode>(group).unwrap();
        assert_eq!(node.role(), Role::RadioGroup);
        assert_eq!(node.label(), Some("UI scale"));
        let children = world.get::<Children>(group).unwrap().to_vec();
        let segments: Vec<_> = children
            .iter()
            .map(|segment| {
                let node = world.get::<AccessibilityNode>(*segment).unwrap();
                (node.role(), node.label().map(str::to_string))
            })
            .collect();
        assert_eq!(
            segments,
            [
                (Role::RadioButton, Some("Auto".to_string())),
                (Role::RadioButton, Some("1".to_string()))
            ]
        );
        assert!(world.get::<Checked>(children[0]).is_some());
        assert!(world.get::<Checked>(children[1]).is_none());
    }
}
