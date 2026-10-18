//! Finds the elements of the UI the way a player sees them: by their text, never by where they are.

use std::time::{Duration, Instant};

use accesskit::Role;
use bevy::{a11y::AccessibilityNode, prelude::*, text::EditableText, ui::UiGlobalTransform};

use super::harness::Harness;
use crate::ui::hud::InventoryCell;

/// What a locator looks for.
enum Target {
    Text(String),
    TextContaining(String),
    Name(String),
    /// The accessibility role and label of a widget.
    Role(Role, String),
    /// The name of the component, for the messages, and the query of its entities.
    Marker(&'static str, MarkerQuery),
    Row(String),
    Cell(u32),
}

impl std::fmt::Debug for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Text(content) => f.debug_tuple("Text").field(content).finish(),
            Target::TextContaining(part) => f.debug_tuple("TextContaining").field(part).finish(),
            Target::Name(name) => f.debug_tuple("Name").field(name).finish(),
            Target::Role(role, label) => f.debug_tuple("Role").field(role).field(label).finish(),
            Target::Marker(component, _) => f.debug_tuple("Marker").field(component).finish(),
            Target::Row(content) => f.debug_tuple("Row").field(content).finish(),
            Target::Cell(id) => f.debug_tuple("Cell").field(id).finish(),
        }
    }
}

/// Finds the entities that have a given component.
type MarkerQuery = fn(&mut World) -> Vec<Entity>;

/// Describes elements of the UI. Only visible elements are ever found.
#[derive(Debug)]
pub struct Locator {
    target: Target,
    scope: Option<Box<Locator>>,
}

impl Locator {
    /// A visible `Text` whose content, with the `TextSpan`s under it, equals `content`, or a
    /// visible text field that holds `content`.
    pub fn text(content: &str) -> Locator {
        Self::of(Target::Text(content.to_string()))
    }

    /// A visible `Text` whose content, with the `TextSpan`s under it, contains `part`, or a
    /// visible text field whose value contains it.
    pub fn text_containing(part: &str) -> Locator {
        Self::of(Target::TextContaining(part.to_string()))
    }

    /// An element with this `Name`, for the ones without text.
    pub fn name(name: &str) -> Locator {
        Self::of(Target::Name(name.to_string()))
    }

    /// A widget with the accessibility role `role` and the label `label`, as every widget of the
    /// UI carries them.
    pub fn role(role: Role, label: &str) -> Locator {
        Self::of(Target::Role(role, label.to_string()))
    }

    /// An element with the component `C`.
    pub fn marker<C: Component>() -> Locator {
        Self::of(Target::Marker(std::any::type_name::<C>(), |world| {
            world
                .query_filtered::<Entity, With<C>>()
                .iter(world)
                .collect()
        }))
    }

    /// The cell of inventory slot `id`. Each slot has a single cell: the hotbar holds the
    /// first slots, and the inventory dialog the others and the crafting grid, so the cell of a
    /// hotbar slot is the one in the hotbar, even while the inventory is open.
    pub fn cell(id: u32) -> Locator {
        Self::of(Target::Cell(id))
    }

    /// The nearest ancestor of `text(content)` that holds another `Text` or a `Button`.
    pub fn row(content: &str) -> Locator {
        Self::of(Target::Row(content.to_string()))
    }

    /// Restricts the locator to the elements inside the ones `scope` finds.
    pub fn within(mut self, scope: Locator) -> Locator {
        self.scope = Some(Box::new(scope));
        self
    }

    fn of(target: Target) -> Locator {
        Locator {
            target,
            scope: None,
        }
    }

    fn find(&self, world: &mut World) -> Vec<Entity> {
        let mut found = match &self.target {
            Target::Text(content) => visible_texts(world)
                .into_iter()
                .filter(|(_, text)| text == content)
                .map(|(entity, _)| entity)
                .collect(),
            Target::TextContaining(part) => visible_texts(world)
                .into_iter()
                .filter(|(_, text)| text.contains(part.as_str()))
                .map(|(entity, _)| entity)
                .collect(),
            Target::Name(name) => world
                .query::<(Entity, &Name)>()
                .iter(world)
                .filter(|(_, n)| n.as_str() == name)
                .map(|(entity, _)| entity)
                .collect(),
            Target::Role(role, label) => world
                .query::<(Entity, &AccessibilityNode)>()
                .iter(world)
                .filter(|(_, node)| node.role() == *role && node.label() == Some(label.as_str()))
                .map(|(entity, _)| entity)
                .collect(),
            Target::Marker(_, find) => find(world),
            Target::Row(content) => visible_texts(world)
                .into_iter()
                .filter(|(_, text)| text == content)
                .filter_map(|(entity, _)| row_of(world, entity))
                .collect(),
            Target::Cell(id) => world
                .query::<(Entity, &InventoryCell)>()
                .iter(world)
                .filter(|(_, cell)| cell.id == *id)
                .map(|(entity, _)| entity)
                .collect(),
        };
        found.retain(|&entity| is_shown(world, entity));
        if let Some(scope) = &self.scope {
            let scopes = scope.find(world);
            found.retain(|&entity| {
                scopes
                    .iter()
                    .any(|&s| s == entity || is_inside(world, entity, s))
            });
        }
        found
    }
}

/// Whether `entity` is a descendant of `ancestor`.
pub(super) fn is_inside(world: &World, entity: Entity, ancestor: Entity) -> bool {
    let mut current = entity;
    while let Some(parent) = world.get::<ChildOf>(current) {
        current = parent.parent();
        if current == ancestor {
            return true;
        }
    }
    false
}

/// Whether `entity` is `ancestor` or a descendant of it.
fn is_inside_or_is(world: &World, entity: Entity, ancestor: Entity) -> bool {
    entity == ancestor || is_inside(world, entity, ancestor)
}

/// What a player reads of a text field, or of a `Text` and of the `TextSpan`s under it: the spans
/// drawn in a fully transparent colour are left out.
fn content_of(world: &World, entity: Entity) -> Option<String> {
    if let Some(field) = world.get::<EditableText>(entity) {
        return Some(field.value().to_string());
    }
    let mut content = world.get::<Text>(entity)?.0.clone();
    if let Some(children) = world.get::<Children>(entity) {
        for child in children.iter() {
            let Some(span) = world.get::<TextSpan>(child) else {
                continue;
            };
            let transparent = world
                .get::<TextColor>(child)
                .is_some_and(|c| c.0.alpha() == 0.0);
            if !transparent {
                content.push_str(&span.0);
            }
        }
    }
    Some(content)
}

/// Every visible `Text` and text field with its full content.
fn visible_texts(world: &mut World) -> Vec<(Entity, String)> {
    let texts: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<Text>, With<EditableText>)>>()
        .iter(world)
        .collect();
    texts
        .into_iter()
        .filter(|&entity| is_shown(world, entity))
        .filter_map(|entity| Some((entity, content_of(world, entity)?)))
        .collect()
}

/// Visible means shown, displayed all the way up the tree, and of a size.
pub(super) fn is_shown(world: &World, entity: Entity) -> bool {
    let shown = world
        .get::<InheritedVisibility>(entity)
        .is_some_and(|v| v.get());
    let sized = world
        .get::<ComputedNode>(entity)
        .is_some_and(|n| n.size().x > 0.0 && n.size().y > 0.0);
    let mut displayed = true;
    let mut current = Some(entity);
    while let Some(e) = current {
        if world
            .get::<Node>(e)
            .is_some_and(|n| n.display == Display::None)
        {
            displayed = false;
        }
        current = world.get::<ChildOf>(e).map(|p| p.parent());
    }
    shown && sized && displayed
}

/// The nearest ancestor of a text that holds another `Text` or a `Button`.
fn row_of(world: &World, text: Entity) -> Option<Entity> {
    let mut current = text;
    while let Some(parent) = world.get::<ChildOf>(current) {
        current = parent.parent();
        let holds_more = world.get::<Children>(current).is_some_and(|children| {
            children.iter().any(|child| {
                !is_inside_or_is(world, text, child) && has_text_or_button(world, child)
            })
        });
        if holds_more {
            return Some(current);
        }
    }
    None
}

/// Whether `entity` or something under it is a `Text` or a `Button`.
fn has_text_or_button(world: &World, entity: Entity) -> bool {
    world.get::<Text>(entity).is_some()
        || world.get::<Button>(entity).is_some()
        || world
            .get::<Children>(entity)
            .is_some_and(|c| c.iter().any(|child| has_text_or_button(world, child)))
}

impl Harness {
    /// The visible elements the locator finds.
    pub fn find(&mut self, locator: &Locator) -> Vec<Entity> {
        locator.find(self.world_mut())
    }

    /// The one visible element the locator finds. Panics, naming the locator and listing the
    /// texts on screen, if there is none or more than one.
    #[track_caller]
    pub fn get(&mut self, locator: &Locator) -> Entity {
        let found = self.find(locator);
        if let [entity] = found[..] {
            return entity;
        }
        let texts = self.visible_texts();
        panic!(
            "expected exactly one element for {locator:?}, found {}; the visible texts are {texts:?}",
            found.len()
        );
    }

    /// Runs frames until the locator finds something, and panics, naming the locator and listing
    /// the texts on screen, if it takes longer than `timeout`.
    #[track_caller]
    pub fn wait_for(&mut self, locator: &Locator, timeout: Duration) {
        let start = Instant::now();
        while !self.is_visible(locator) {
            if start.elapsed() >= timeout {
                let texts = self.visible_texts();
                panic!(
                    "timed out after {timeout:?} waiting for {locator:?}; the visible texts are {texts:?}"
                );
            }
            self.frames(1);
        }
    }

    /// The content of every visible text, for the messages of a failure.
    fn visible_texts(&mut self) -> Vec<String> {
        visible_texts(self.world_mut())
            .into_iter()
            .map(|(_, text)| text)
            .collect()
    }

    /// Whether the locator finds something now.
    pub fn is_visible(&mut self, locator: &Locator) -> bool {
        !self.find(locator).is_empty()
    }

    /// The text of the element: its own, or the ones under it, separated by spaces.
    #[track_caller]
    pub fn text_of(&mut self, locator: &Locator) -> String {
        let entity = self.get(locator);
        let world = self.world();
        if let Some(content) = content_of(world, entity) {
            return content;
        }
        let mut texts = Vec::new();
        collect_texts(world, entity, &mut texts);
        texts.join(" ")
    }

    /// The centre of the element, in physical pixels.
    #[track_caller]
    pub fn center_of(&mut self, locator: &Locator) -> Vec2 {
        let entity = self.get(locator);
        self.world()
            .get::<UiGlobalTransform>(entity)
            .unwrap()
            .translation
    }
}

fn collect_texts(world: &World, entity: Entity, texts: &mut Vec<String>) {
    if let Some(content) = content_of(world, entity) {
        texts.push(content);
    }
    if let Some(children) = world.get::<Children>(entity) {
        for &child in children {
            collect_texts(world, child, texts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::e2e::harness::SCREEN;
    use crate::e2e::helpers::main_menu;
    use Locator as L;

    impl Harness {
        /// The entity of a text, visible or not.
        fn get_unsized(&mut self, content: &str) -> Entity {
            self.world_mut()
                .query::<(Entity, &Text)>()
                .iter(self.world())
                .find(|(_, text)| text.0 == content)
                .unwrap()
                .0
        }
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_visible_text_is_found_once() {
        let mut h = main_menu();
        h.get(&L::text("Singleplayer"));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_hidden_text_is_not_found() {
        let mut h = main_menu();
        let size = |width| Node {
            width: Val::Px(width),
            height: Val::Px(20.0),
            ..default()
        };
        // Three ways to hide a text, each shown afterwards so that the case can fail
        let hidden = h
            .world_mut()
            .spawn((size(100.0), Text::new("Invisible"), Visibility::Hidden))
            .id();
        let undisplayed = h
            .world_mut()
            .spawn((
                Node {
                    display: Display::None,
                    ..size(100.0)
                },
                children![(size(100.0), Text::new("Undisplayed"))],
            ))
            .id();
        h.world_mut().spawn((size(0.0), Text::new("Flat")));
        h.frames(3);
        for text in ["Invisible", "Undisplayed", "Flat"] {
            assert!(h.find(&L::text(text)).is_empty(), "{text} is hidden");
        }

        h.world_mut()
            .entity_mut(hidden)
            .insert(Visibility::Inherited);
        h.world_mut().get_mut::<Node>(undisplayed).unwrap().display = Display::Flex;
        let flat = h.get_unsized("Flat");
        h.world_mut().get_mut::<Node>(flat).unwrap().width = Val::Px(100.0);
        h.frames(3);
        for text in ["Invisible", "Undisplayed", "Flat"] {
            assert!(h.is_visible(&L::text(text)), "{text} is shown");
        }
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_text_of_a_closed_screen_is_not_found() {
        let mut h = main_menu();
        h.click(&L::text("Singleplayer"));
        h.wait_for(&L::text("World list"), SCREEN);
        h.click(&L::text("Back to menu"));
        h.wait_for(&L::text("Singleplayer"), SCREEN);
        assert!(h.find(&L::text("World list")).is_empty());
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_text_is_matched_with_its_spans() {
        let mut h = main_menu();
        h.world_mut().spawn((
            Node {
                width: Val::Px(100.0),
                height: Val::Px(20.0),
                ..default()
            },
            Text::new("FPS: "),
            children![TextSpan::new("60")],
        ));
        h.frames(3);
        h.get(&L::text("FPS: 60"));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_transparent_span_is_not_part_of_the_text() {
        let mut h = main_menu();
        h.world_mut().spawn((
            Node {
                width: Val::Px(100.0),
                height: Val::Px(20.0),
                ..default()
            },
            Text::new("Seen"),
            children![(TextSpan::new("Unseen"), TextColor(Color::NONE))],
        ));
        h.frames(3);
        h.get(&L::text("Seen"));
        assert!(h.find(&L::text("SeenUnseen")).is_empty());
    }

    // The panics of these tests are not caught in the test: the Cranelift backend of the dev
    // profile neither runs destructors nor catches while unwinding, so the harness of a
    // panicking test leaves its folder behind.

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    #[should_panic(expected = "Singleplayer")]
    fn a_missing_text_fails_with_the_texts_on_screen() {
        main_menu().get(&L::text("Nope"));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    #[should_panic(
        expected = r#"Marker("client::ui::hud::hotbar::Hotbar"), scope: Some(Locator { target: Text("Quit")"#
    )]
    fn a_missing_marker_is_named_by_its_type_and_scope() {
        let hotbar = L::marker::<crate::ui::hud::hotbar::Hotbar>();
        main_menu().get(&hotbar.within(L::text("Quit")));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    fn a_widget_is_found_by_its_role_and_label() {
        let mut h = main_menu();
        h.world_mut().spawn((
            bevy::ui_widgets::Button,
            bevy::ui::prelude::AccessibleLabel::new("Play probe"),
            Node {
                width: Val::Px(120.0),
                height: Val::Px(40.0),
                ..default()
            },
        ));
        h.frames(3);
        h.get(&L::role(Role::Button, "Play probe"));
        assert!(h.find(&L::role(Role::CheckBox, "Play probe")).is_empty());
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    #[should_panic(expected = r#"Role(Button, "Nope")"#)]
    fn a_missing_role_is_named_in_the_failure() {
        main_menu().get(&L::role(Role::Button, "Nope"));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    #[should_panic(expected = r#"waiting for Locator { target: Text("Nope")"#)]
    fn a_wait_that_times_out_names_its_locator() {
        main_menu().wait_for(&L::text("Nope"), Duration::from_millis(50));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    #[should_panic(expected = "waiting for the moon")]
    fn waiting_for_what_never_happens_fails_with_its_description() {
        main_menu().wait_until("the moon", SCREEN, |_| false);
    }
}
