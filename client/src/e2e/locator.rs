//! Finds the elements of the UI the way a player sees them: by their text, never by where they are.

use bevy::{prelude::*, ui::UiGlobalTransform};

use super::harness::Harness;

/// What a locator looks for.
enum Target {
    Text(String),
    Name(String),
    Marker(fn(&mut World) -> Vec<Entity>),
    Row(String),
}

/// Describes elements of the UI. Only visible elements are ever found.
pub struct Locator {
    target: Target,
    scope: Option<Box<Locator>>,
}

impl Locator {
    /// A visible `Text` whose content, with the `TextSpan`s under it, equals `content`.
    pub fn text(content: &str) -> Locator {
        Self::of(Target::Text(content.to_string()))
    }

    /// An element with this `Name`, for the ones without text.
    pub fn name(name: &str) -> Locator {
        Self::of(Target::Name(name.to_string()))
    }

    /// An element with the component `C`.
    pub fn marker<C: Component>() -> Locator {
        Self::of(Target::Marker(|world| {
            world
                .query_filtered::<Entity, With<C>>()
                .iter(world)
                .collect()
        }))
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
            Target::Name(name) => world
                .query::<(Entity, &Name)>()
                .iter(world)
                .filter(|(_, n)| n.as_str() == name)
                .map(|(entity, _)| entity)
                .collect(),
            Target::Marker(find) => find(world),
            Target::Row(content) => visible_texts(world)
                .into_iter()
                .filter(|(_, text)| text == content)
                .filter_map(|(entity, _)| row_of(world, entity))
                .collect(),
        };
        found.retain(|&entity| is_visible(world, entity));
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
fn is_inside(world: &World, entity: Entity, ancestor: Entity) -> bool {
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

/// The content of a `Text` and of the `TextSpan`s under it.
fn content_of(world: &World, entity: Entity) -> Option<String> {
    let mut content = world.get::<Text>(entity)?.0.clone();
    if let Some(children) = world.get::<Children>(entity) {
        for &child in children {
            if let Some(span) = world.get::<TextSpan>(child) {
                content.push_str(&span.0);
            }
        }
    }
    Some(content)
}

/// Every visible `Text` with its full content.
fn visible_texts(world: &mut World) -> Vec<(Entity, String)> {
    let texts: Vec<Entity> = world
        .query_filtered::<Entity, With<Text>>()
        .iter(world)
        .collect();
    texts
        .into_iter()
        .filter(|&entity| is_visible(world, entity))
        .filter_map(|entity| Some((entity, content_of(world, entity)?)))
        .collect()
}

/// Visible means shown, displayed all the way up the tree, and of a size.
fn is_visible(world: &World, entity: Entity) -> bool {
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

    /// The one visible element the locator finds. Panics, listing the texts on screen, if there
    /// is none or more than one.
    pub fn get(&mut self, locator: &Locator) -> Entity {
        let found = self.find(locator);
        if let [entity] = found[..] {
            return entity;
        }
        let texts: Vec<String> = visible_texts(self.world_mut())
            .into_iter()
            .map(|(_, text)| text)
            .collect();
        panic!(
            "expected exactly one element, found {}; the visible texts are {texts:?}",
            found.len()
        );
    }

    pub fn is_visible(&mut self, locator: &Locator) -> bool {
        !self.find(locator).is_empty()
    }

    /// The text of the element: its own, or the ones under it, separated by spaces.
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
    use crate::ui::button::MenuState;
    use Locator as L;

    fn main_menu() -> Harness {
        let mut h = Harness::new();
        h.wait_until("the main menu", SCREEN, |w| {
            *w.resource::<State<MenuState>>().get() == MenuState::Main
        });
        h.frames(3);
        h
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
    #[should_panic(expected = "Singleplayer")]
    fn a_missing_text_fails_with_the_texts_on_screen() {
        let mut h = main_menu();
        h.get(&L::text("Nope"));
    }

    #[test]
    #[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
    #[should_panic(expected = "waiting for the moon")]
    fn waiting_for_what_never_happens_fails_with_its_description() {
        let mut h = main_menu();
        h.wait_until("the moon", SCREEN, |_| false);
    }
}
