//! The tab bar: a row of tabs on a `RadioGroup`, wired like the segmented control.
//!
//! The bar takes the focus and the arrow keys; callers observe `On<TabChange>` on it and show the
//! content of the chosen tab.

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    input_focus::tab_navigation::TabIndex,
    picking::hover::Hovered,
    prelude::*,
    ui::Checked,
    ui_widgets::{RadioButton, RadioGroup, ValueChange},
};

use super::{
    focus::FocusRing,
    segmented::{check_chosen, style_options, ChoiceOption, OptionLook},
};
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, FONT_M, MUTED, PANEL, RAISE, RAISE_HOVER, SPACE_L,
    SPACE_S, SPACE_XS, TEXT,
};

/// What `spawn_tabs` builds.
// The settings screen (Lot 1, Task 6) is the first user of the tab bar
#[allow(dead_code)]
pub struct Tabs<'a> {
    /// The accessible label of the bar.
    pub label: &'a str,
    pub tabs: &'a [&'a str],
    /// The index of the tab checked at first.
    pub selected: usize,
}

/// The tab `index` of a tab bar.
#[derive(Component)]
pub struct TabButton(pub usize);

/// Triggered on a tab bar when the player checks another tab.
// The settings screen (Lot 1, Task 6) is the first user of the tab bar
#[allow(dead_code)]
#[derive(EntityEvent)]
pub struct TabChange {
    pub entity: Entity,
    pub index: usize,
}

/// Spawns a tab bar, without a parent, and returns it.
// The settings screen (Lot 1, Task 6) is the first user of the tab bar
#[allow(dead_code)]
pub fn spawn_tabs(commands: &mut Commands, tabs: &Tabs, font: Handle<Font>) -> Entity {
    let bar = commands
        .spawn((
            RadioGroup,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::TabList)),
            AccessibleLabel::new(tabs.label),
            TabIndex(0),
            FocusRing,
            Node {
                column_gap: px(SPACE_XS),
                ..default()
            },
        ))
        .observe(choose_tab)
        .id();
    for (index, label) in tabs.tabs.iter().enumerate() {
        let checked = index == tabs.selected;
        let look = TabButton::look(checked, false);
        let mut tab = commands.spawn((
            RadioButton,
            AccessibilityNode(accesskit::Node::new(Role::Tab)),
            AccessibleLabel::new(*label),
            TabButton(index),
            Hovered::default(),
            Node {
                border: UiRect::all(px(BEVEL_WIDTH)),
                padding: UiRect::axes(px(SPACE_L), px(SPACE_S)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(look.background),
            look.border,
            ChildOf(bar),
            children![(
                Text::new(*label),
                TextFont {
                    font: font.clone().into(),
                    font_size: px(FONT_M).into(),
                    ..default()
                },
                TextColor(look.text),
            )],
        ));
        if checked {
            tab.insert(Checked);
        }
    }
    bar
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, style_options::<TabButton>);
}

/// Checks the chosen tab and tells the callers.
fn choose_tab(
    change: On<ValueChange<Entity>>,
    children: Query<&Children>,
    tabs: Query<&TabButton>,
    mut commands: Commands,
) {
    if let Some(index) = check_chosen(&change, &children, &tabs, &mut commands) {
        commands.trigger(TabChange {
            entity: change.source,
            index,
        });
    }
}

impl ChoiceOption for TabButton {
    fn index(&self) -> usize {
        self.0
    }

    fn look(checked: bool, hovered: bool) -> OptionLook {
        let raised = BorderColor {
            top: BEVEL_HI,
            left: BEVEL_HI,
            bottom: BEVEL_LO,
            right: BEVEL_LO,
        };
        if checked {
            // the checked tab opens onto the panel under it, underlined in the accent
            OptionLook {
                background: PANEL,
                border: BorderColor {
                    bottom: ACCENT,
                    ..raised
                },
                text: TEXT,
            }
        } else {
            OptionLook {
                background: if hovered { RAISE_HOVER } else { RAISE },
                border: raised,
                text: MUTED,
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
    fn the_tab_list_and_its_tabs_have_their_roles() {
        let mut world = World::new();
        let tabs = Tabs {
            label: "Settings",
            tabs: &["Controls", "Audio"],
            selected: 1,
        };
        let list = spawn_tabs(&mut world.commands(), &tabs, Handle::default());
        world.flush();
        let node = world.get::<AccessibilityNode>(list).unwrap();
        assert_eq!(node.role(), Role::TabList);
        assert_eq!(node.label(), Some("Settings"));
        let children = world.get::<Children>(list).unwrap().to_vec();
        let tabs: Vec<_> = children
            .iter()
            .map(|tab| {
                let node = world.get::<AccessibilityNode>(*tab).unwrap();
                (node.role(), node.label().map(str::to_string))
            })
            .collect();
        assert_eq!(
            tabs,
            [
                (Role::Tab, Some("Controls".to_string())),
                (Role::Tab, Some("Audio".to_string()))
            ]
        );
        assert!(world.get::<Checked>(children[1]).is_some());
        assert!(world.get::<Checked>(children[0]).is_none());
    }
}
