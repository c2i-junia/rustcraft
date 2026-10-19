//! The Singleplayer screen: the saved worlds as cards, last played first, and a detail pane for
//! the selected one with Play, Rename and Delete. New world (Ctrl+N) sits in the title bar.
//!
//! The cards are always built again from the disk (`list_worlds`) once a world was created or
//! deleted: the first card is the world of the main menu's Continue card.

pub mod dialogs;

use accesskit::Role;
use bevy::{
    a11y::AccessibilityNode,
    ecs::query::QueryFilter,
    input::{keyboard::KeyboardInput, ButtonState},
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, FocusedInput, InputFocus, InputFocusVisible,
    },
    prelude::*,
    ui::Selected,
    ui_widgets::{observe, Activate},
};
use shared::GameFolderPaths;

use super::background::menu_background;
use super::worlds::{
    elapsed_since, format_size, list_worlds, played_ago, start_world, thumbnail_color, WorldSummary,
};
use super::MenuState;
use crate::constants::SAVE_PATH;
use crate::state::GameState;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::theme::{
    BEVEL_WIDTH, DANGER, FONT_L, FONT_M, FONT_S, FONT_XL, MUTED, PANEL, SPACE_L, SPACE_M, SPACE_S,
    SPACE_XL, SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::dialog::{CloseRequest, DialogRoot};
use crate::ui::widgets::focus::ArrowNav;
use crate::ui::widgets::hints::{hint_bar, Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{
    cards_of, spawn_list_card, spawn_scroll_list, CardSelected, ListCard,
};
use crate::ui::widgets::{raised_bevel, sunken_bevel};
use dialogs::{open_delete_world_dialog, open_new_world_dialog, open_rename_dialog, WorldsChanged};

/// The world the game plays, set when the player chooses one.
#[derive(Resource, Default, Debug, Clone)]
pub struct SelectedWorld {
    pub name: Option<String>,
}

/// The title of the screen.
pub const WORLDS_TITLE: &str = "Worlds";

/// A world card, and what it shows of its world.
#[derive(Component)]
pub struct WorldCard(pub WorldSummary);

/// The list of the world cards: the scroll area they sit in.
#[derive(Component)]
pub struct WorldList;

/// The detail pane of the selected world, hidden while there is no world.
#[derive(Component)]
pub struct WorldDetail;

/// What the selected world shows in the detail pane, above its buttons.
#[derive(Component)]
struct WorldFacts;

/// A button of the detail pane, which acts on the selected world.
#[derive(Component, Clone, Copy)]
pub enum WorldAction {
    Play,
    Rename,
    Delete,
}

/// The line under the buttons of the detail pane saying why an action failed, hidden while it
/// is empty.
#[derive(Component)]
pub struct WorldError;

/// The New world button of the title bar.
#[derive(Component)]
pub struct NewWorldButton;

/// Which key hints the hint bar of the screen shows.
#[derive(Component, PartialEq, Eq)]
enum SoloHints {
    /// A card has the focus.
    Cards,
    /// Anything else has it, or there is no world.
    Elsewhere,
}

const NEW_WORLD_HINT: Hint = Hint {
    keys: "Ctrl+N",
    action: "New world",
};
const BACK_HINT: Hint = Hint {
    keys: "Esc",
    action: "Back",
};

/// The key hints while a card has the focus.
pub const CARD_HINTS: [Hint; 5] = [
    Hint {
        keys: "↑↓",
        action: "Select",
    },
    Hint {
        keys: "Enter",
        action: "Play",
    },
    Hint {
        keys: "Del",
        action: "Delete",
    },
    NEW_WORLD_HINT,
    BACK_HINT,
];

/// The key hints while anything else has the focus.
pub const SCREEN_HINTS: [Hint; 4] = [
    Hint {
        keys: "Tab",
        action: "Move",
    },
    Hint {
        keys: "Enter",
        action: "Select",
    },
    NEW_WORLD_HINT,
    BACK_HINT,
];

/// The width of the detail pane.
const DETAIL_WIDTH: f32 = 280.0;
/// The side of the thumbnail of a card.
const CARD_THUMBNAIL: f32 = 40.0;
/// The side of the thumbnail of the detail pane.
const DETAIL_THUMBNAIL: f32 = 64.0;

/// The Singleplayer screen and its dialogs.
pub fn solo_plugin(app: &mut App) {
    dialogs::plugin(app);
    app.add_systems(OnEnter(MenuState::Solo), solo_setup)
        .add_systems(
            Update,
            follow_focus_with_hints.run_if(in_state(MenuState::Solo)),
        )
        .add_observer(show_changed_worlds)
        .add_observer(play_or_delete_on_key)
        .add_observer(act_on_selected_world);
}

/// Spawns the screen, then lists the worlds.
fn solo_setup(mut commands: Commands, assets: Res<AssetServer>) {
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(MenuState::Solo),
            TabGroup::new(0),
        ))
        .id();
    let content = commands
        .spawn((
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(SPACE_XL),
                    right: px(SPACE_XL),
                    top: px(SPACE_XL),
                    bottom: px(HINT_BAR_HEIGHT + SPACE_XL),
                },
                row_gap: px(SPACE_M),
                ..default()
            },
            ChildOf(root),
        ))
        .id();
    commands.spawn((
        Node {
            align_items: AlignItems::Center,
            ..default()
        },
        ChildOf(content),
        children![
            text(WORLDS_TITLE, FONT_XL, TEXT, &font),
            Node {
                flex_grow: 1.0,
                ..default()
            },
            (
                button("New world", ButtonVariant::Default, font.clone()),
                NewWorldButton,
                observe(open_new_world),
            ),
        ],
    ));
    let body = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_height: px(0),
                column_gap: px(SPACE_L),
                ..default()
            },
            ChildOf(content),
        ))
        .id();
    let scroll = spawn_scroll_list(&mut commands);
    commands.entity(scroll.frame).insert(ChildOf(body));
    commands
        .entity(scroll.list)
        .insert((
            WorldList,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::ListBox)),
            AccessibleLabel::new(WORLDS_TITLE),
        ))
        .observe(show_selected_world);
    spawn_detail_pane(&mut commands, body, &font);
    commands.spawn((
        Node::default(),
        ChildOf(content),
        children![(
            button("Back", ButtonVariant::Default, font.clone()),
            observe(
                |_: On<Activate>, mut menu_state: ResMut<NextState<MenuState>>| {
                    menu_state.set(MenuState::Main);
                }
            ),
        )],
    ));
    commands.spawn((
        hint_bar(&SCREEN_HINTS, font, assets.load(MONO_FONT_PATH)),
        SoloHints::Elsewhere,
        ChildOf(root),
    ));
    commands.trigger(WorldsChanged {
        focus: None,
        error: None,
    });
}

/// Spawns the detail pane in `body`: the facts of the selected world, Play, Rename, Delete, and the line
/// of a failure.
fn spawn_detail_pane(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    let pane = commands
        .spawn((
            WorldDetail,
            Node {
                width: px(DETAIL_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(SPACE_L)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                row_gap: px(SPACE_L),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        WorldFacts,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            min_width: px(0),
            ..default()
        },
        ChildOf(pane),
    ));
    let actions = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(pane),
        ))
        .id();
    let buttons = commands
        .spawn((
            ArrowNav,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(actions),
            children![(
                button("Play", ButtonVariant::Primary, font.clone()),
                WorldAction::Play,
            )],
        ))
        .id();
    // side by side under Play, which leaves a long name two lines of the pane at 800x600
    let pair = commands
        .spawn((
            Node {
                column_gap: px(SPACE_S),
                ..default()
            },
            ChildOf(buttons),
        ))
        .id();
    // red only in the confirmation: Play stays the one strong action
    for (label, action) in [
        ("Rename", WorldAction::Rename),
        ("Delete", WorldAction::Delete),
    ] {
        commands
            .spawn((
                button(label, ButtonVariant::Default, font.clone()),
                action,
                ChildOf(pair),
            ))
            .entry::<Node>()
            .and_modify(|mut node| {
                // halves of the width, whatever their labels
                node.flex_grow = 1.0;
                node.flex_basis = px(0);
            });
    }
    commands.spawn((
        WorldError,
        text("", FONT_S, DANGER, font),
        Node {
            display: Display::None,
            ..default()
        },
        ChildOf(actions),
    ));
}

/// A text of the screen, which breaks a long word rather than leave its box.
fn text(content: &str, size: f32, color: Color, font: &Handle<Font>) -> impl Bundle {
    (
        Text::new(content),
        TextFont {
            font: font.clone().into(),
            font_size: px(size).into(),
            ..default()
        },
        TextColor(color),
        TextLayout::linebreak(LineBreak::WordOrCharacter),
    )
}

/// A thumbnail of `side` pixels in the colour of the world of `seed`.
fn thumbnail(seed: Option<u32>, side: f32) -> impl Bundle {
    (
        Node {
            width: px(side),
            height: px(side),
            flex_shrink: 0.0,
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(thumbnail_color(seed)),
        sunken_bevel(),
    )
}

/// Despawns the cards and spawns one per world, in order; returns them. Shows the empty state when
/// there is none, with `error` under it: the detail pane, which shows it otherwise, is hidden.
fn show_worlds(
    commands: &mut Commands,
    list: Entity,
    worlds: &[WorldSummary],
    error: Option<&str>,
    font: &Handle<Font>,
) -> Vec<Entity> {
    commands.entity(list).despawn_related::<Children>();
    if worlds.is_empty() {
        let empty = commands
            .spawn((
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: px(SPACE_S),
                    ..default()
                },
                ChildOf(list),
                children![
                    text("No worlds yet", FONT_L, TEXT, font),
                    text("Create one with New world (Ctrl+N).", FONT_S, MUTED, font),
                ],
            ))
            .id();
        if let Some(message) = error {
            commands.spawn((text(message, FONT_S, DANGER, font), ChildOf(empty)));
        }
        return Vec::new();
    }
    worlds
        .iter()
        .map(|world| {
            let card = spawn_list_card(commands, list, &world.name);
            let details = match (world.day, world.seed) {
                (Some(day), Some(seed)) => {
                    text(&format!("Day {day} · Seed {seed}"), FONT_S, MUTED, font)
                }
                _ => text("Cannot read world.ron", FONT_S, DANGER, font),
            };
            let played = text(
                &played_ago(elapsed_since(world.played)),
                FONT_S,
                MUTED,
                font,
            );
            let size = text(&format_size(world.size), FONT_S, MUTED, font);
            // two lines, each with its fact on the right: the short size leaves the day and
            // the seed the width the age takes from the name
            commands.entity(card).insert((
                WorldCard(world.clone()),
                children![
                    thumbnail(world.seed, CARD_THUMBNAIL),
                    (
                        Node {
                            flex_direction: FlexDirection::Column,
                            flex_grow: 1.0,
                            min_width: px(0),
                            row_gap: px(SPACE_XS),
                            ..default()
                        },
                        children![
                            card_line(text(&world.name, FONT_M, TEXT, font), played),
                            card_line(details, size),
                        ],
                    ),
                ],
            ));
            card
        })
        .collect()
}

/// A line of a card: `left`, which wraps, and `right`, which keeps its width, at the end.
fn card_line(left: impl Bundle, right: impl Bundle) -> impl Bundle {
    (
        Node {
            column_gap: px(SPACE_M),
            align_items: AlignItems::Center,
            ..default()
        },
        children![
            // in a box of its own: a text that shrinks itself adds its height at the narrowest,
            // a letter a line, to the content size of the list, which then shows its scrollbar
            (
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..default()
                },
                children![left],
            ),
            (
                right,
                Node {
                    flex_shrink: 0.0,
                    ..default()
                },
            ),
        ],
    )
}

/// Shows `world` in the detail pane, in place of what it showed.
fn show_detail(commands: &mut Commands, facts: Entity, world: &WorldSummary, font: &Handle<Font>) {
    commands.entity(facts).despawn_related::<Children>();
    commands
        .spawn((thumbnail(world.seed, DETAIL_THUMBNAIL), ChildOf(facts)))
        .entry::<Node>()
        .and_modify(|mut node| node.margin = UiRect::bottom(px(SPACE_S)));
    let mut line = |content: &str, size: f32, color: Color| {
        commands.spawn((text(content, size, color, font), ChildOf(facts)));
    };
    line(&world.name, FONT_L, TEXT);
    match (world.day, world.seed) {
        (Some(day), Some(seed)) => {
            line(&format!("Day {day}"), FONT_M, TEXT);
            line(&format!("Seed {seed}"), FONT_S, MUTED);
        }
        _ => line("Cannot read world.ron", FONT_S, DANGER),
    }
    line(&played_ago(elapsed_since(world.played)), FONT_S, MUTED);
    line(
        &format!("{} on disk", format_size(world.size)),
        FONT_S,
        MUTED,
    );
}

/// Shows `message` under the buttons of the detail pane, or hides the line for `None`.
fn show_world_error<F: QueryFilter>(
    errors: &mut Query<(&mut Text, &mut Node), F>,
    message: Option<String>,
) {
    for (mut text, mut node) in errors.iter_mut() {
        node.display = if message.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        text.0 = message.clone().unwrap_or_default();
    }
}

/// The names of the world cards, from the top of the list.
fn card_names(
    lists: &Query<Entity, With<WorldList>>,
    children: &Query<&Children>,
    cards: &Query<&WorldCard>,
) -> Vec<String> {
    lists
        .iter()
        .flat_map(|list| cards_of(list, children))
        .filter_map(|card| cards.get(card).ok())
        .map(|card| card.0.name.clone())
        .collect()
}

/// The world whose card takes the focus once `name` is deleted: the next one, else the previous.
fn neighbour(names: &[String], name: &str) -> Option<String> {
    let index = names.iter().position(|other| other == name)?;
    names
        .get(index + 1)
        .or_else(|| {
            index
                .checked_sub(1)
                .and_then(|previous| names.get(previous))
        })
        .cloned()
}

/// Lists the worlds of the disk again, and selects and focuses the card of `focus`, else the
/// first card, and shows `error` under the buttons of the detail pane. Without a world, the
/// detail pane hides, its buttons leave the tab order and New world takes the focus. A dialog
/// still open (not closing) keeps the focus.
#[allow(clippy::too_many_arguments)]
fn show_changed_worlds(
    changed: On<WorldsChanged>,
    lists: Query<Entity, With<WorldList>>,
    open_dialogs: Query<(), (With<DialogRoot>, Without<CloseRequest>)>,
    mut panes: Query<&mut Node, With<WorldDetail>>,
    mut actions: Query<&mut TabIndex, With<WorldAction>>,
    facts: Query<Entity, With<WorldFacts>>,
    new_world: Query<Entity, With<NewWorldButton>>,
    mut errors: Query<(&mut Text, &mut Node), (With<WorldError>, Without<WorldDetail>)>,
    mut focus: ResMut<InputFocus>,
    mut focus_visible: ResMut<InputFocusVisible>,
    keys: Res<ButtonInput<KeyCode>>,
    paths: Res<GameFolderPaths>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(list) = lists.single() else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    let worlds = list_worlds(&paths.game_folder_path.join(SAVE_PATH));
    let cards = show_worlds(&mut commands, list, &worlds, changed.error, &font);
    let mut focus_on = |entity| {
        if open_dialogs.is_empty() {
            focus.set(entity, FocusCause::Navigated);
        }
    };
    let chosen = changed
        .focus
        .as_ref()
        .and_then(|name| worlds.iter().position(|world| &world.name == name))
        .unwrap_or(0);
    let shown = match (cards.get(chosen), facts.single()) {
        // the list sends no `CardSelected` for a card selected here: the pane is filled now
        (Some(&card), Ok(facts)) => {
            commands.entity(card).insert((Selected, TabIndex(0)));
            focus_on(card);
            // reached by the keyboard, the card shows its ring; a click leaves it hidden
            let typed = keys.get_just_pressed().next().is_some()
                || keys.get_just_released().next().is_some();
            if open_dialogs.is_empty() && typed {
                focus_visible.0 = true;
            }
            show_detail(&mut commands, facts, &worlds[chosen], &font);
            true
        }
        _ => {
            if let Ok(button) = new_world.single() {
                focus_on(button);
            }
            false
        }
    };
    for mut pane in &mut panes {
        pane.display = if shown { Display::Flex } else { Display::None };
    }
    // a hidden button would still take Tab
    for mut tab_index in &mut actions {
        tab_index.set_if_neq(TabIndex(if shown { 0 } else { -1 }));
    }
    show_world_error(&mut errors, changed.error.map(str::to_string));
}

/// Shows the world of the card just selected in the detail pane.
fn show_selected_world(
    selected: On<CardSelected>,
    cards: Query<&WorldCard>,
    facts: Query<Entity, With<WorldFacts>>,
    mut errors: Query<(&mut Text, &mut Node), With<WorldError>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Ok(card), Ok(facts)) = (cards.get(selected.card), facts.single()) else {
        return;
    };
    show_detail(&mut commands, facts, &card.0, &assets.load(FONT_PATH));
    show_world_error(&mut errors, None);
}

/// Enter on a focused card plays its world, unless Control is held; Delete asks to delete it.
#[allow(clippy::too_many_arguments)]
fn play_or_delete_on_key(
    mut event: On<FocusedInput<KeyboardInput>>,
    cards: Query<&WorldCard>,
    lists: Query<Entity, With<WorldList>>,
    children: Query<&Children>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<InputFocus>,
    assets: Res<AssetServer>,
    mut selected: ResMut<SelectedWorld>,
    mut game: ResMut<NextState<GameState>>,
    mut menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    // the event bubbles: only the step on the card itself is handled
    let Ok(card) = cards.get(event.focused_entity) else {
        return;
    };
    let input = &event.input;
    if input.state != ButtonState::Pressed || input.repeat {
        return;
    }
    let control = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    match input.key_code {
        KeyCode::Enter if !control => {
            start_world(&card.0.name, &mut selected, &mut game, &mut menu);
        }
        KeyCode::Delete => {
            let next = neighbour(&card_names(&lists, &children, &cards), &card.0.name);
            open_delete_world_dialog(&mut commands, &assets, &focus, &card.0, next);
        }
        _ => return,
    }
    event.propagate(false);
}

/// Carries out the button of the detail pane that was activated, on the selected world.
#[allow(clippy::too_many_arguments)]
fn act_on_selected_world(
    activate: On<Activate>,
    actions: Query<&WorldAction>,
    selected_cards: Query<&WorldCard, With<Selected>>,
    lists: Query<Entity, With<WorldList>>,
    children: Query<&Children>,
    cards: Query<&WorldCard>,
    focus: Res<InputFocus>,
    assets: Res<AssetServer>,
    mut selected: ResMut<SelectedWorld>,
    mut game: ResMut<NextState<GameState>>,
    mut menu: ResMut<NextState<MenuState>>,
    mut commands: Commands,
) {
    let (Ok(action), Ok(card)) = (actions.get(activate.entity), selected_cards.single()) else {
        return;
    };
    match action {
        WorldAction::Play => start_world(&card.0.name, &mut selected, &mut game, &mut menu),
        WorldAction::Rename => {
            let others = cards
                .iter()
                .map(|other| other.0.name.clone())
                .filter(|name| name != &card.0.name)
                .collect();
            open_rename_dialog(&mut commands, &assets, &focus, &card.0.name, others);
        }
        WorldAction::Delete => {
            let next = neighbour(&card_names(&lists, &children, &cards), &card.0.name);
            open_delete_world_dialog(&mut commands, &assets, &focus, &card.0, next);
        }
    }
}

/// Opens the New world dialog from the button of the title bar.
fn open_new_world(
    _: On<Activate>,
    cards: Query<&WorldCard>,
    assets: Res<AssetServer>,
    focus: Res<InputFocus>,
    mut commands: Commands,
) {
    let others = cards.iter().map(|card| card.0.name.clone()).collect();
    open_new_world_dialog(&mut commands, &assets, &focus, others);
}

/// Keeps the hint bar on the hints of the cards while a card has the focus, and on those of the
/// screen otherwise.
fn follow_focus_with_hints(
    focus: Res<InputFocus>,
    cards: Query<(), With<ListCard>>,
    bars: Query<(Entity, &SoloHints, &ChildOf)>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let wanted = if focus.get().is_some_and(|entity| cards.contains(entity)) {
        SoloHints::Cards
    } else {
        SoloHints::Elsewhere
    };
    let Ok((bar, shown, child_of)) = bars.single() else {
        return;
    };
    if *shown == wanted {
        return;
    }
    let hints: &[Hint] = match wanted {
        SoloHints::Cards => &CARD_HINTS,
        SoloHints::Elsewhere => &SCREEN_HINTS,
    };
    commands.entity(bar).despawn();
    commands.spawn((
        hint_bar(hints, assets.load(FONT_PATH), assets.load(MONO_FONT_PATH)),
        wanted,
        ChildOf(child_of.parent()),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_deleted_world_hands_the_focus_to_the_next_else_the_previous() {
        let names: Vec<String> = ["A", "B", "C"].map(String::from).to_vec();
        assert_eq!(neighbour(&names, "A").as_deref(), Some("B"));
        assert_eq!(neighbour(&names, "B").as_deref(), Some("C"));
        assert_eq!(neighbour(&names, "C").as_deref(), Some("B"));
        assert_eq!(neighbour(&names[..1], "A"), None);
        assert_eq!(neighbour(&names, "Z"), None);
    }
}
