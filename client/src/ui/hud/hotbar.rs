use super::{item_name, InventoryRoot, UIMode};
use crate::constants::MAX_HOTBAR_SLOTS;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use shared::players::Inventory;
use std::time::Duration;

/// Pixels of trackpad scrolling that select the next hotbar slot.
const SCROLL_PIXELS_PER_SLOT: f32 = 20.0;

/// How long the name of the selected item stays on screen.
pub const ITEM_NAME_DURATION: Duration = Duration::from_secs(2);

/// The keys that select a hotbar slot, and name it over a slot of the inventory: `Digit1` is
/// slot 0. Hard-wired, they are not part of the `KeyMap`.
pub const HOTBAR_KEYS: [KeyCode; 9] = [
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
];

/// The hotbar slot of the first digit key pressed this frame.
pub fn hotbar_key_index(input: &ButtonInput<KeyCode>) -> Option<u32> {
    HOTBAR_KEYS
        .iter()
        .position(|key| input.just_pressed(*key))
        .map(|index| index as u32)
}

#[derive(Component)]
pub struct Hotbar {
    pub selected: u32,
}

/// The name of the selected item, shown over the hotbar for a moment when the selection changes.
#[derive(Component, Default)]
pub struct HotbarItemName {
    /// When the name hides again, on the game clock.
    pub hide_at: Option<Duration>,
}

/// The whole slots a scroll of `y` adds to the selection, going down the hotbar for a positive
/// result. `remainder` keeps the part of a slot not scrolled yet, in slots: it is dropped when
/// the wheel turns the other way, so that a trackpad reversing course answers at once.
fn scroll_steps(remainder: &mut f32, unit: MouseScrollUnit, y: f32) -> i32 {
    let slots = match unit {
        MouseScrollUnit::Line => y,
        MouseScrollUnit::Pixel => y / SCROLL_PIXELS_PER_SLOT,
    };
    if slots * *remainder < 0. {
        *remainder = 0.;
    }
    *remainder += slots;
    let whole = remainder.trunc();
    *remainder -= whole;
    -(whole as i32)
}

/// The number keys select a hotbar slot, during gameplay only.
pub fn select_hotbar_slot_with_keys(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    ui_mode: Res<UIMode>,
    mut hotbar: Query<&mut Hotbar>,
) {
    if *ui_mode != UIMode::Closed {
        return;
    }
    let (Some(index), Ok(mut hotbar)) = (hotbar_key_index(&keyboard_input), hotbar.single_mut())
    else {
        return;
    };
    hotbar.selected = index;
}

/// The mouse wheel and the trackpad change the selected hotbar slot, during gameplay only.
pub fn select_hotbar_slot_with_wheel(
    mut scroll: MessageReader<MouseWheel>,
    ui_mode: Res<UIMode>,
    mut hotbar: Query<&mut Hotbar>,
    mut remainder: Local<f32>,
) {
    if *ui_mode != UIMode::Closed {
        scroll.clear();
        *remainder = 0.;
        return;
    }
    let Ok(mut hotbar) = hotbar.single_mut() else {
        return;
    };
    let mut selected = hotbar.selected as i32;
    for event in scroll.read() {
        selected += scroll_steps(&mut remainder, event.unit, event.y);
    }
    let selected = selected.rem_euclid(MAX_HOTBAR_SLOTS as i32) as u32;
    if hotbar.selected != selected {
        hotbar.selected = selected;
    }
}

/// Shows the name of the item of the slot just selected for `ITEM_NAME_DURATION`, and hides it
/// when the time is up. An empty slot, or the open inventory, shows nothing.
pub fn show_selected_item_name(
    time: Res<Time>,
    inventory: Res<Inventory>,
    hotbar: Query<&Hotbar>,
    inventory_root: Query<&Visibility, (With<InventoryRoot>, Without<HotbarItemName>)>,
    mut name: Query<(&mut HotbarItemName, &mut Text, &mut Visibility)>,
    mut previous: Local<Option<u32>>,
) {
    let (Ok(hotbar), Ok((mut name, mut text, mut visibility))) =
        (hotbar.single(), name.single_mut())
    else {
        return;
    };
    let now = time.elapsed();
    if name.hide_at.is_some_and(|hide_at| now >= hide_at) {
        name.hide_at = None;
        *visibility = Visibility::Hidden;
    }
    // The first frame only records the selection
    let changed = previous.is_some_and(|previous| previous != hotbar.selected);
    *previous = Some(hotbar.selected);
    if !changed {
        return;
    }
    let is_open = matches!(inventory_root.single(), Ok(&Visibility::Visible));
    match inventory.inner.get(&hotbar.selected) {
        Some(stack) if !is_open => {
            text.0 = item_name(stack.item_id);
            name.hide_at = Some(now + ITEM_NAME_DURATION);
            *visibility = Visibility::Inherited;
        }
        _ => {
            name.hide_at = None;
            *visibility = Visibility::Hidden;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::hud::InventoryRoot;
    use shared::players::Inventory;
    use shared::world::{ItemId, ItemStack};
    use std::time::Duration;

    #[test]
    fn line_scrolling_moves_one_slot_per_line() {
        assert_eq!(scroll_steps(&mut 0., MouseScrollUnit::Line, -1.), 1);
        assert_eq!(scroll_steps(&mut 0., MouseScrollUnit::Line, 2.), -2);
    }

    #[test]
    fn pixel_scrolling_accumulates_into_steps() {
        let mut remainder = 0.;
        let steps: Vec<i32> = (0..4)
            .map(|_| scroll_steps(&mut remainder, MouseScrollUnit::Pixel, -5.))
            .collect();
        assert_eq!(steps, [0, 0, 0, 1]);
        assert_eq!(
            scroll_steps(&mut remainder, MouseScrollUnit::Pixel, -45.),
            2
        );
        // 5 px are left over, still going the same way
        assert_eq!(remainder * SCROLL_PIXELS_PER_SLOT, -5.);
    }

    #[test]
    fn reversing_the_wheel_drops_the_remainder() {
        let mut remainder = 0.;
        assert_eq!(
            scroll_steps(&mut remainder, MouseScrollUnit::Pixel, -15.),
            0
        );
        assert_eq!(scroll_steps(&mut remainder, MouseScrollUnit::Pixel, 10.), 0);
        assert_eq!(remainder * SCROLL_PIXELS_PER_SLOT, 10.);
        assert_eq!(
            scroll_steps(&mut remainder, MouseScrollUnit::Pixel, 10.),
            -1
        );
    }

    fn planks_in_slot_2() -> Inventory {
        let mut inventory = Inventory::new();
        inventory.inner.insert(
            2,
            ItemStack {
                item_id: ItemId::OakPlanks,
                item_type: ItemId::OakPlanks.get_default_type(),
                nb: 4,
            },
        );
        inventory
    }

    /// A world with a hotbar and its item name, and an inventory panel in `panel`.
    fn name_world(panel: Visibility) -> (World, Entity, Entity) {
        let mut world = World::new();
        world.insert_resource(Time::<()>::default());
        world.insert_resource(planks_in_slot_2());
        let hotbar = world.spawn(Hotbar { selected: 0 }).id();
        world.spawn((InventoryRoot, panel));
        let name = world
            .spawn((HotbarItemName::default(), Text::new(""), Visibility::Hidden))
            .id();
        (world, hotbar, name)
    }

    fn shown(world: &World, name: Entity) -> (String, Visibility) {
        (
            world.get::<Text>(name).unwrap().0.clone(),
            *world.get::<Visibility>(name).unwrap(),
        )
    }

    #[test]
    fn the_item_name_shows_for_two_seconds_when_the_selection_changes() {
        let (mut world, hotbar, name) = name_world(Visibility::Hidden);
        // `Local` lives in the system state: one system instance, run several times
        let mut run = IntoSystem::into_system(show_selected_item_name);
        run.initialize(&mut world);
        let mut step = |world: &mut World| {
            run.run((), world).unwrap();
        };
        step(&mut world);
        assert_eq!(shown(&world, name).1, Visibility::Hidden);

        world.get_mut::<Hotbar>(hotbar).unwrap().selected = 2;
        step(&mut world);
        assert_eq!(
            shown(&world, name),
            ("Oak Planks".to_string(), Visibility::Inherited)
        );

        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(1900));
        step(&mut world);
        assert_eq!(shown(&world, name).1, Visibility::Inherited);

        world
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(200));
        step(&mut world);
        assert_eq!(shown(&world, name).1, Visibility::Hidden);
    }

    #[test]
    fn the_item_name_stays_hidden_on_an_empty_slot_or_with_the_inventory_open() {
        let (mut world, hotbar, name) = name_world(Visibility::Hidden);
        let mut run = IntoSystem::into_system(show_selected_item_name);
        run.initialize(&mut world);
        run.run((), &mut world).unwrap();
        // Slot 5 is empty
        world.get_mut::<Hotbar>(hotbar).unwrap().selected = 5;
        run.run((), &mut world).unwrap();
        assert_eq!(shown(&world, name).1, Visibility::Hidden);

        let (mut world, hotbar, name) = name_world(Visibility::Visible);
        let mut run = IntoSystem::into_system(show_selected_item_name);
        run.initialize(&mut world);
        run.run((), &mut world).unwrap();
        world.get_mut::<Hotbar>(hotbar).unwrap().selected = 2;
        run.run((), &mut world).unwrap();
        assert_eq!(shown(&world, name).1, Visibility::Hidden);
    }
}
