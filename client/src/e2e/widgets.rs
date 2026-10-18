//! The end-to-end tests of the widgets in `ui/widgets/`.

use accesskit::Role;
use bevy::{
    ecs::system::RunSystemOnce,
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        AutoFocus, InputFocus, InputFocusVisible,
    },
    prelude::*,
    text::EditableText,
    ui::InteractionDisabled,
    ui_widgets::Activate,
    window::PrimaryWindow,
};

use super::harness::{Harness, Options, SCREEN};
use super::helpers::main_menu;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::assets::MONO_FONT_PATH;
use crate::ui::menus::QUIT_TITLE;
use crate::ui::theme::{ACCENT, BEVEL_HI, BEVEL_LO, DANGER, RAISE_HOVER};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{spawn_dialog, Dialog, DialogClosed, DialogRoot};
use crate::ui::widgets::focus::{ArrowNav, FocusRing};
use crate::ui::widgets::hints::key_cap;
use crate::ui::widgets::slot::{slot, SlotSelected};
use crate::ui::widgets::text_field::{
    spawn_text_field, Cancel, FieldError, Submit, TextField, TextFieldParts,
};
use crate::ui::widgets::tooltip::{Tooltip, TooltipRoot};

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_ui_scale_follows_the_window_height() {
    let mut h = Harness::start(
        GameFolder::new(),
        Options {
            width: 1920,
            height: 1080,
            ..default()
        },
    );
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.world().resource::<UiScale>().0, 1.5);
    h.resize(800, 600);
    h.frames(2);
    assert_eq!(h.world().resource::<UiScale>().0, 1.0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_forced_ui_scale_is_read_from_the_game_folder() {
    let mut h = Harness::start(
        GameFolder::new().with_file("interface.ron", "(ui_scale: X1)"),
        Options {
            width: 1920,
            height: 1080,
            ..default()
        },
    );
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    assert_eq!(h.world().resource::<UiScale>().0, 1.0);
}

/// Spawns a tab group in a corner of the screen, above the main menu, and lays it out.
fn spawn_group(h: &mut Harness, layout: Node) -> Entity {
    let root = h
        .world_mut()
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(20.0),
                top: Val::Px(20.0),
                column_gap: Val::Px(10.0),
                row_gap: Val::Px(10.0),
                ..layout
            },
            // modal: Tab stays among the probes, not the buttons of the menu
            TabGroup::modal(),
            GlobalZIndex(1),
        ))
        .id();
    h.frames(1);
    root
}

/// A focusable, named 80x30 probe with a focus ring.
fn probe(name: &str) -> impl Bundle {
    (
        Name::new(name.to_string()),
        Node {
            width: Val::Px(80.0),
            height: Val::Px(30.0),
            ..default()
        },
        TabIndex(0),
        FocusRing,
    )
}

/// Spawns a row of probes A, B and C, with the focus on A.
fn abc_row(h: &mut Harness) {
    let root = spawn_group(h, Node::default());
    h.world_mut().spawn((probe("A"), AutoFocus, ChildOf(root)));
    h.world_mut().spawn((probe("B"), ChildOf(root)));
    h.world_mut().spawn((probe("C"), ChildOf(root)));
    h.frames(3);
}

fn focused(h: &mut Harness) -> Entity {
    h.world().resource::<InputFocus>().get().unwrap()
}

fn assert_focus_on(h: &mut Harness, name: &str) {
    let expected = h.get(&L::name(name));
    assert_eq!(focused(h), expected, "the focus should be on {name}");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn tab_and_shift_tab_move_the_focus_in_order() {
    let mut h = main_menu();
    abc_row(&mut h);
    assert_focus_on(&mut h, "A");
    for name in ["B", "C", "A"] {
        h.press(KeyCode::Tab);
        assert_focus_on(&mut h, name);
    }
    h.key_down(KeyCode::ShiftLeft);
    h.press(KeyCode::Tab);
    h.key_up(KeyCode::ShiftLeft);
    assert_focus_on(&mut h, "C");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_focus_ring_follows_tab_and_hides_on_click() {
    let mut h = main_menu();
    abc_row(&mut h);
    let ringed = |h: &mut Harness| -> Vec<Entity> {
        let mut outlines = h
            .world_mut()
            .query_filtered::<(Entity, &Outline), With<FocusRing>>();
        outlines
            .iter(h.world())
            .filter(|(_, o)| o.color == ACCENT)
            .map(|(e, _)| e)
            .collect()
    };
    assert!(ringed(&mut h).is_empty());
    h.press(KeyCode::Tab);
    let b = h.get(&L::name("B"));
    assert_eq!(ringed(&mut h), vec![b]);
    h.click(&L::name("A"));
    h.frames(2);
    assert_focus_on(&mut h, "A");
    assert!(!h.world().resource::<InputFocusVisible>().0);
    assert!(ringed(&mut h).is_empty());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn arrows_move_the_focus_inside_an_arrow_nav_container() {
    let mut h = main_menu();
    let root = spawn_group(&mut h, Node::default());
    let grid = h
        .world_mut()
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                ..default()
            },
            ArrowNav,
            ChildOf(root),
        ))
        .id();
    for names in [["TL", "TR"], ["BL", "BR"]] {
        let row = h
            .world_mut()
            .spawn((
                Node {
                    column_gap: Val::Px(10.0),
                    ..default()
                },
                ChildOf(grid),
            ))
            .id();
        for name in names {
            let mut cell = h.world_mut().spawn((probe(name), ChildOf(row)));
            if name == "TL" {
                cell.insert(AutoFocus);
            }
        }
    }
    h.world_mut().spawn((probe("X"), ChildOf(root)));
    h.frames(3);

    assert_focus_on(&mut h, "TL");
    for (key, name) in [
        (KeyCode::ArrowRight, "TR"),
        (KeyCode::ArrowDown, "BR"),
        (KeyCode::ArrowRight, "BR"),
    ] {
        h.press(key);
        assert_focus_on(&mut h, name);
    }
    assert!(h.world().resource::<InputFocusVisible>().0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn arrows_do_nothing_outside_an_arrow_nav_container() {
    let mut h = main_menu();
    let root = spawn_group(&mut h, Node::default());
    h.world_mut().spawn((probe("W"), ChildOf(root)));
    h.world_mut().spawn((probe("X"), ChildOf(root)));
    h.frames(3);
    h.click(&L::name("X"));
    h.frames(2);
    assert_focus_on(&mut h, "X");
    h.press(KeyCode::ArrowLeft);
    assert_focus_on(&mut h, "X");
}

/// Counts the activations of the buttons of a test.
#[derive(Resource, Default)]
struct Activations(u32);

/// Spawns a "Play" themed button above the main menu and counts its activations.
fn spawn_button(h: &mut Harness, extra: impl Bundle) -> Entity {
    h.world_mut().init_resource::<Activations>();
    let root = spawn_group(h, Node::default());
    let play = h
        .world_mut()
        .spawn((
            button("Play", ButtonVariant::Default, Handle::default()),
            ChildOf(root),
            extra,
        ))
        .observe(|_: On<Activate>, mut count: ResMut<Activations>| count.0 += 1)
        .id();
    h.frames(3);
    play
}

fn activations(h: &mut Harness) -> u32 {
    h.world().resource::<Activations>().0
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn clicking_a_themed_button_activates_it() {
    let mut h = main_menu();
    spawn_button(&mut h, ());
    h.click(&L::role(Role::Button, "Play"));
    h.frames(2);
    assert_eq!(activations(&mut h), 1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn enter_and_space_activate_the_focused_button() {
    let mut h = main_menu();
    spawn_button(&mut h, AutoFocus);
    h.press(KeyCode::Enter);
    assert_eq!(activations(&mut h), 1);
    h.press(KeyCode::Space);
    assert_eq!(activations(&mut h), 2);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_disabled_button_does_not_activate() {
    let mut h = main_menu();
    spawn_button(&mut h, (AutoFocus, InteractionDisabled));
    h.click(&L::role(Role::Button, "Play"));
    h.press(KeyCode::Enter);
    h.frames(2);
    assert_eq!(activations(&mut h), 0);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_themed_button_lightens_it() {
    let mut h = main_menu();
    let play = spawn_button(&mut h, ());
    h.hover(&L::role(Role::Button, "Play"));
    h.frames(2);
    assert_eq!(
        h.world().get::<BackgroundColor>(play).unwrap().0,
        RAISE_HOVER
    );
}

/// What the text field of a test submitted, and how many times it was cancelled.
#[derive(Resource, Default)]
struct FieldEvents {
    submits: Vec<String>,
    cancels: u32,
}

/// Spawns a probe "P", then a "Name" text field of at most 5 characters with the focus, in a
/// 300 px wide column above the main menu, and records its submits and cancels.
fn spawn_field(h: &mut Harness) -> TextFieldParts {
    h.world_mut().init_resource::<FieldEvents>();
    let root = spawn_group(
        h,
        Node {
            width: Val::Px(300.0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
    );
    h.world_mut().spawn((probe("P"), ChildOf(root)));
    let field = TextField {
        label: "Name",
        placeholder: "Your name",
        max_characters: Some(5),
        font: Handle::default(),
        font_size: 20.0,
    };
    let parts = spawn_text_field(&mut h.world_mut().commands(), &field);
    h.world_mut().flush();
    h.world_mut().entity_mut(parts.root).insert(ChildOf(root));
    h.world_mut()
        .entity_mut(parts.input)
        .insert(AutoFocus)
        .observe(|submit: On<Submit>, mut events: ResMut<FieldEvents>| {
            events.submits.push(submit.value.clone());
        })
        .observe(|_: On<Cancel>, mut events: ResMut<FieldEvents>| events.cancels += 1);
    h.frames(3);
    parts
}

fn submits(h: &mut Harness) -> Vec<String> {
    h.world().resource::<FieldEvents>().submits.clone()
}

fn cancels(h: &mut Harness) -> u32 {
    h.world().resource::<FieldEvents>().cancels
}

fn value(h: &mut Harness, input: Entity) -> String {
    h.world()
        .get::<EditableText>(input)
        .unwrap()
        .value()
        .to_string()
}

/// The frame of a text field: the parent of its input.
fn frame_of(h: &mut Harness, input: Entity) -> Entity {
    h.world().get::<ChildOf>(input).unwrap().parent()
}

fn border_of(h: &mut Harness, entity: Entity) -> [Color; 4] {
    let border = h.world().get::<BorderColor>(entity).unwrap();
    [border.top, border.left, border.bottom, border.right]
}

/// Writes a press of C and a press of Enter, then runs a single frame.
fn press_c_and_enter_in_one_frame(h: &mut Harness) {
    let window = h
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    let keys = [
        (KeyCode::KeyC, Key::Character("c".into()), Some("c")),
        (KeyCode::Enter, Key::Enter, None),
    ];
    for (key_code, logical_key, text) in keys {
        h.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window,
        });
    }
    h.frames(1);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn enter_in_a_text_field_submits_its_value() {
    let mut h = main_menu();
    spawn_field(&mut h);
    h.type_text("abc");
    h.press(KeyCode::Enter);
    h.frames(2);
    assert_eq!(submits(&mut h), ["abc"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_character_and_enter_in_one_frame_submit_both() {
    let mut h = main_menu();
    spawn_field(&mut h);
    press_c_and_enter_in_one_frame(&mut h);
    assert_eq!(submits(&mut h), ["c"]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_in_a_text_field_cancels_it() {
    let mut h = main_menu();
    spawn_field(&mut h);
    h.press(KeyCode::Escape);
    h.frames(2);
    assert_eq!(cancels(&mut h), 1);
    assert!(submits(&mut h).is_empty());
    // the field took the Escape: the main menu behind does not also ask to quit
    assert!(!h.is_visible(&L::text(QUIT_TITLE)));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_placeholder_shows_only_while_the_field_is_empty() {
    let mut h = main_menu();
    spawn_field(&mut h);
    assert!(h.is_visible(&L::text("Your name")));
    h.type_text("a");
    assert!(!h.is_visible(&L::text("Your name")));
    h.press(KeyCode::Backspace);
    assert!(h.is_visible(&L::text("Your name")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_field_keeps_at_most_max_characters() {
    let mut h = main_menu();
    let parts = spawn_field(&mut h);
    h.type_text("abcdefg");
    assert_eq!(value(&mut h, parts.input), "abcde");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_error_shows_under_the_field_with_a_red_bevel() {
    let mut h = main_menu();
    let parts = spawn_field(&mut h);
    let frame = frame_of(&mut h, parts.input);
    h.world_mut()
        .entity_mut(parts.input)
        .insert(FieldError(Some("Bad name".into())));
    h.wait_for(&L::text("Bad name"), SCREEN);
    assert_eq!(border_of(&mut h, frame), [DANGER; 4]);
    h.world_mut()
        .entity_mut(parts.input)
        .insert(FieldError(None));
    h.frames(2);
    assert!(!h.is_visible(&L::text("Bad name")));
    assert_eq!(
        border_of(&mut h, frame),
        [BEVEL_LO, BEVEL_LO, BEVEL_HI, BEVEL_HI]
    );
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_click_focuses_the_field_and_shows_its_ring_after_tab() {
    let mut h = main_menu();
    let parts = spawn_field(&mut h);
    let frame = frame_of(&mut h, parts.input);
    let ring = |h: &mut Harness| h.world().get::<Outline>(frame).unwrap().color;
    h.click(&L::name("P"));
    h.frames(2);
    assert_focus_on(&mut h, "P");
    h.click(&L::role(Role::TextInput, "Name"));
    h.frames(2);
    assert_eq!(focused(&mut h), parts.input);
    assert_eq!(ring(&mut h), Color::NONE);
    h.click(&L::name("P"));
    h.press(KeyCode::Tab);
    h.frames(1);
    assert_eq!(focused(&mut h), parts.input);
    assert_eq!(ring(&mut h), ACCENT);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_click_on_the_field_padding_focuses_it() {
    let mut h = main_menu();
    let parts = spawn_field(&mut h);
    let frame = frame_of(&mut h, parts.input);
    h.click(&L::name("P"));
    h.frames(2);
    assert_focus_on(&mut h, "P");
    // 5 logical px inside the left edge of the frame, in its bevel and padding band of 10
    let node = h.world().get::<ComputedNode>(frame).unwrap();
    let (size, inset) = (node.size(), 5.0 / node.inverse_scale_factor());
    let center = h
        .world()
        .get::<UiGlobalTransform>(frame)
        .unwrap()
        .translation;
    h.click_at(Vec2::new(center.x - size.x / 2.0 + inset, center.y));
    h.frames(2);
    assert_eq!(focused(&mut h), parts.input);
}

/// Counts the presses that reach the input of the text field of a test.
#[derive(Resource, Default)]
struct InputPresses(u32);

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_click_on_the_placeholder_reaches_the_field() {
    let mut h = main_menu();
    let parts = spawn_field(&mut h);
    h.world_mut().init_resource::<InputPresses>();
    h.world_mut()
        .entity_mut(parts.input)
        .observe(|_: On<Pointer<Press>>, mut presses: ResMut<InputPresses>| presses.0 += 1);
    h.click(&L::name("P"));
    h.frames(2);
    assert_focus_on(&mut h, "P");
    h.click(&L::text("Your name"));
    h.frames(2);
    assert_eq!(focused(&mut h), parts.input);
    // the input itself was pressed, so the caret was placed
    assert_eq!(h.world().resource::<InputPresses>().0, 1);
}

/// Spawns a "Probe slot", a 100x40 "Tooltip probe" with a tooltip, and an "Elsewhere" probe, in
/// a row above the main menu.
fn spawn_slot_and_tooltip_probes(h: &mut Harness, tooltip: Tooltip) -> Entity {
    let root = spawn_group(h, Node::default());
    let probe_slot = h
        .world_mut()
        .spawn((
            slot(
                "Probe slot",
                Some(3),
                ImageNode::default(),
                Handle::default(),
            ),
            ChildOf(root),
        ))
        .id();
    h.world_mut().spawn((
        Name::new("Tooltip probe"),
        Node {
            width: Val::Px(100.0),
            height: Val::Px(40.0),
            ..default()
        },
        tooltip,
        ChildOf(root),
    ));
    h.world_mut().spawn((probe("Elsewhere"), ChildOf(root)));
    h.frames(3);
    probe_slot
}

fn dirt_tooltip() -> Tooltip {
    Tooltip {
        title: "Dirt".into(),
        detail: "5 items".into(),
        hint: "Shift+click: move to the hotbar".into(),
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_a_slot_lightens_it() {
    let mut h = main_menu();
    let slot = spawn_slot_and_tooltip_probes(&mut h, dirt_tooltip());
    h.hover(&L::role(Role::GridCell, "Probe slot"));
    h.frames(2);
    assert_eq!(
        h.world().get::<BackgroundColor>(slot).unwrap().0,
        RAISE_HOVER
    );
    h.world_mut().entity_mut(slot).insert(SlotSelected(true));
    h.frames(1);
    assert_eq!(border_of(&mut h, slot), [ACCENT; 4]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn hovering_an_element_with_a_tooltip_shows_it_by_the_cursor() {
    let mut h = main_menu();
    spawn_slot_and_tooltip_probes(&mut h, dirt_tooltip());
    let probe_center = h.center_of(&L::name("Tooltip probe"));
    h.hover(&L::name("Tooltip probe"));
    h.frames(3);
    let panel = L::marker::<TooltipRoot>;
    for text in ["Dirt", "5 items", "Shift+click: move to the hotbar"] {
        assert!(h.is_visible(&L::text(text).within(panel())), "{text}");
    }
    let root = h.get(&panel());
    let size = h.world().get::<ComputedNode>(root).unwrap().size();
    let center = h
        .world()
        .get::<UiGlobalTransform>(root)
        .unwrap()
        .translation;
    let top_left = center - size / 2.0;
    // 12 logical px right of and below the cursor, which is at the centre of the probe
    let offset = 12.0
        / h.world()
            .get::<ComputedNode>(root)
            .unwrap()
            .inverse_scale_factor();
    assert!(
        (top_left.x - probe_center.x - offset).abs() <= 1.0,
        "{top_left} {probe_center}"
    );
    assert!(
        (top_left.y - probe_center.y - offset).abs() <= 1.0,
        "{top_left} {probe_center}"
    );
    h.hover(&L::name("Elsewhere"));
    h.frames(2);
    assert!(!h.is_visible(&panel()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_screenshot_shows_the_tooltip_of_the_hovered_element() {
    let mut h = main_menu();
    spawn_slot_and_tooltip_probes(&mut h, dirt_tooltip());
    h.hover(&L::name("Tooltip probe"));
    h.frames(3);
    let root = h.get(&L::marker::<TooltipRoot>());
    let size = h.world().get::<ComputedNode>(root).unwrap().size();
    let center = h
        .world()
        .get::<UiGlobalTransform>(root)
        .unwrap()
        .translation;
    let shot = h.screenshot();
    // The middle of the top bevel of the tooltip
    let pixel = shot.get_pixel(center.x as u32, (center.y - size.y / 2.0) as u32 + 1);
    assert_eq!(pixel.0, BEVEL_HI.to_srgba().to_u8_array_no_alpha());
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_empty_tooltip_shows_nothing() {
    let mut h = main_menu();
    spawn_slot_and_tooltip_probes(&mut h, Tooltip::default());
    h.hover(&L::name("Tooltip probe"));
    h.frames(3);
    assert!(!h.is_visible(&L::marker::<TooltipRoot>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_tooltip_is_inside_the_window_on_the_first_frame_it_shows() {
    let mut h = main_menu();
    h.world_mut().spawn((
        Name::new("Corner probe"),
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(0.0),
            bottom: Val::Px(0.0),
            width: Val::Px(100.0),
            height: Val::Px(40.0),
            ..default()
        },
        dirt_tooltip(),
        GlobalZIndex(1),
    ));
    h.frames(3);
    let center = h.center_of(&L::name("Corner probe"));
    h.move_cursor(center);
    let panel = L::marker::<TooltipRoot>;
    let mut shown = false;
    for _ in 0..10 {
        h.frames(1);
        if h.is_visible(&panel()) {
            shown = true;
            break;
        }
    }
    assert!(shown, "the tooltip never showed");
    let root = h.get(&panel());
    let size = h.world().get::<ComputedNode>(root).unwrap().size();
    let middle = h
        .world()
        .get::<UiGlobalTransform>(root)
        .unwrap()
        .translation;
    let window = h
        .world_mut()
        .query_filtered::<&Window, With<PrimaryWindow>>()
        .single(h.world())
        .unwrap();
    let window = Vec2::new(
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    let (min, max) = (middle - size / 2.0, middle + size / 2.0);
    assert!(
        min.x >= -0.5 && min.y >= -0.5 && max.x <= window.x + 0.5 && max.y <= window.y + 0.5,
        "{min} {max} is outside {window}"
    );
}

/// The actions the dialogs of a test were closed with.
#[derive(Resource, Default)]
struct Closes(Vec<Option<usize>>);

/// Spawns a "Before" button with the focus, then a probe dialog "Probe?" whose Cancel is focused,
/// and records how its dialogs close.
fn dialog_probe() -> Harness {
    let mut h = main_menu();
    h.world_mut().init_resource::<Closes>();
    h.world_mut()
        .add_observer(|closed: On<DialogClosed>, mut closes: ResMut<Closes>| {
            closes.0.push(closed.action)
        });
    let root = spawn_group(&mut h, Node::default());
    h.world_mut().spawn((
        button("Before", ButtonVariant::Default, Handle::default()),
        AutoFocus,
        ChildOf(root),
    ));
    h.frames(3);
    open_probe_dialog(&mut h);
    h
}

fn open_probe_dialog(h: &mut Harness) {
    h.world_mut()
        .run_system_once(|mut commands: Commands, focus: Res<InputFocus>| {
            let dialog = Dialog {
                title: "Probe?",
                body: "Sure?",
                actions: &[
                    ("Cancel", ButtonVariant::Default),
                    ("Quit", ButtonVariant::Danger),
                ],
                focused: 0,
            };
            spawn_dialog(&mut commands, &dialog, Handle::default(), &focus);
        })
        .unwrap();
    h.frames(3);
}

fn closes(h: &mut Harness) -> Vec<Option<usize>> {
    h.world().resource::<Closes>().0.clone()
}

/// Whether the focus is on the dialog or inside it.
fn focus_in_dialog(h: &mut Harness) -> bool {
    let root = h.get(&L::marker::<DialogRoot>());
    let mut at = Some(focused(h));
    while let Some(entity) = at {
        if entity == root {
            return true;
        }
        at = h.world().get::<ChildOf>(entity).map(ChildOf::parent);
    }
    false
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_dialog_focuses_its_chosen_button_and_keeps_the_focus_inside() {
    let mut h = dialog_probe();
    assert_eq!(h.focus_label().as_deref(), Some("Cancel"));
    for _ in 0..4 {
        h.press(KeyCode::Tab);
        assert!(focus_in_dialog(&mut h), "Tab left the dialog");
    }
    // a click anywhere that is not a button leaves the focus on the same button, which still
    // reacts to Tab
    let title = h.center_of(&L::text("Probe?"));
    let title_height = {
        let entity = h.get(&L::text("Probe?"));
        h.world().get::<ComputedNode>(entity).unwrap().size().y
    };
    let clicks = [
        ("the backdrop", Vec2::new(5.0, 5.0)),
        (
            "the panel padding",
            title - Vec2::new(0.0, title_height / 2.0 + 8.0),
        ),
        ("the title", title),
        ("the body", h.center_of(&L::text("Sure?"))),
    ];
    for (what, position) in clicks {
        h.click_at(position);
        h.frames(2);
        assert!(h.is_visible(&L::text("Probe?")));
        assert_eq!(
            h.focus_label().as_deref(),
            Some("Cancel"),
            "a click on {what}"
        );
        h.press(KeyCode::Tab);
        assert_eq!(h.focus_label().as_deref(), Some("Quit"), "Tab after {what}");
        h.press(KeyCode::Tab);
        assert_eq!(
            h.focus_label().as_deref(),
            Some("Cancel"),
            "Tab after {what}"
        );
    }
    assert!(closes(&mut h).is_empty());
    h.press(KeyCode::Escape);
    h.frames(2);
    assert_eq!(closes(&mut h), [None]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn escape_closes_a_dialog_and_gives_the_focus_back() {
    let mut h = dialog_probe();
    h.press(KeyCode::Escape);
    h.frames(2);
    assert_eq!(closes(&mut h), [None]);
    assert!(h.find(&L::marker::<DialogRoot>()).is_empty());
    assert_eq!(h.focus_label().as_deref(), Some("Before"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_dialog_button_closes_it_with_its_action() {
    let mut h = dialog_probe();
    h.click(&L::role(Role::Button, "Quit").within(L::marker::<DialogRoot>()));
    h.frames(2);
    assert_eq!(closes(&mut h), [Some(1)]);
    assert!(h.find(&L::marker::<DialogRoot>()).is_empty());
    open_probe_dialog(&mut h);
    h.press(KeyCode::Enter);
    h.frames(2);
    assert_eq!(closes(&mut h), [Some(1), Some(0)]);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_key_cap_draws_arrow_glyphs() {
    let mut h = main_menu();
    let mono: Handle<Font> = h.world().resource::<AssetServer>().load(MONO_FONT_PATH);
    let root = spawn_group(&mut h, Node::default());
    h.world_mut()
        .spawn((key_cap("←", Handle::default(), mono.clone()), ChildOf(root)));
    h.world_mut()
        .spawn((key_cap("Enter", Handle::default(), mono), ChildOf(root)));
    h.frames(10);
    assert!(h.bright_pixels_in(&L::text("←")) > 20);
}
