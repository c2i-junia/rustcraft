//! The end-to-end tests of the widgets in `ui/widgets/`.

use bevy::{
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        AutoFocus, InputFocus, InputFocusVisible,
    },
    prelude::*,
};

use super::harness::{Harness, Options, SCREEN};
use super::helpers::main_menu;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::ui::theme::ACCENT;
use crate::ui::widgets::focus::{ArrowNav, FocusRing};

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
            TabGroup::new(0),
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
