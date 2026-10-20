//! The Mods screen: the mods of the game folder as cards, each with its switch, a search above
//! them with Turn all on and Turn all off, and a detail pane for the selected one.
//!
//! What it shows comes from `shared::mods`, as the server will load the mods: a mod whose
//! dependencies are off waits for them, one whose manifest or script is broken says why. The
//! screen changes `mods.ron` when it is left, and the mods apply when a world starts. Every
//! change of the screen builds the cards again (`ShowMods`), the selected one kept.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use accesskit::{Role, Toggled};
use bevy::{
    a11y::AccessibilityNode,
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    input::{
        keyboard::{Key, KeyboardInput},
        ButtonState,
    },
    input_focus::{
        tab_navigation::{TabGroup, TabIndex},
        FocusCause, FocusedInput, InputFocus, InputFocusVisible,
    },
    prelude::*,
    text::EditableText,
    ui::Selected,
    ui_widgets::{observe, Activate},
};
use shared::mods::{
    find_mods, plan_loading, read_mods_settings, write_mods_settings, FoundMod, ModState,
    ModsSettings, MODS_FOLDER, MODS_SETTINGS_PATH,
};
use shared::GameFolderPaths;

use super::background::menu_background;
use super::worlds::thumbnail_color;
use super::{
    follow_focus_with_hints, list_hint_bar, text, ListEmpty, ListFocus, ListHints, MenuState,
};
use crate::ui::assets::FONT_PATH;
use crate::ui::theme::{
    ACCENT, BEVEL_WIDTH, DANGER, FONT_L, FONT_M, FONT_S, FONT_XL, MUTED, OK, PANEL, SLOT, SPACE_L,
    SPACE_M, SPACE_S, SPACE_XL, SPACE_XS, TEXT,
};
use crate::ui::widgets::button::{button, ButtonVariant};
use crate::ui::widgets::hints::{Hint, HINT_BAR_HEIGHT};
use crate::ui::widgets::list::{spawn_list_card, spawn_scroll_list, CardSelected, WheelStep};
use crate::ui::widgets::text_field::{set_text, spawn_text_field, Cancel, TextField};
use crate::ui::widgets::toast::{show_toast, ToastKind};
use crate::ui::widgets::{raised_bevel, sunken_bevel};

/// The title of the screen, and the label of its button on the main menu.
pub const MODS_TITLE: &str = "Mods";

/// The sides an icon may have, in pixels: a larger or a not square one is not shown.
const ICON_SIDES: std::ops::RangeInclusive<u32> = 16..=128;
const CARD_ICON: f32 = 40.0;
const DETAIL_ICON: f32 = 64.0;
const DETAIL_WIDTH: f32 = 300.0;
const SEARCH_WIDTH: f32 = 260.0;
const SWITCH_WIDTH: f32 = 44.0;
const SWITCH_HEIGHT: f32 = 24.0;
/// The side of the knob of a switch, within its bevel.
const KNOB: f32 = SWITCH_HEIGHT - 2.0 * BEVEL_WIDTH - 4.0;

/// The mods the screen shows, and the settings it changes.
#[derive(Resource)]
pub struct ModsScreen {
    game_folder: PathBuf,
    mods: Vec<ShownMod>,
    /// The settings as the player leaves them.
    pub settings: ModsSettings,
    /// The settings as `mods.ron` holds them.
    saved: ModsSettings,
    /// Why `mods.ron` could not be read, every mod being on then.
    settings_error: Option<String>,
    query: String,
    /// The id of the selected mod.
    selected: Option<String>,
}

/// A mod of the screen.
struct ShownMod {
    found: FoundMod,
    /// Why its script does not parse.
    script_error: Option<String>,
    icon: Option<Handle<Image>>,
}

impl ModsScreen {
    /// Reads the mods of `game_folder` and its `mods.ron`, and loads the icons into `images`.
    fn read(game_folder: &Path, images: &mut Assets<Image>) -> Self {
        let mods = find_mods(&game_folder.join(MODS_FOLDER))
            .into_iter()
            .map(|found| ShownMod {
                script_error: found
                    .problem
                    .is_none()
                    .then(|| server::script_error(&found))
                    .flatten(),
                icon: found
                    .icon()
                    .and_then(|path| read_icon(&path))
                    .map(|image| images.add(image)),
                found,
            })
            .collect();
        let (saved, settings_error) = match read_mods_settings(game_folder) {
            Ok(settings) => (settings, None),
            Err(error) => (ModsSettings::default(), Some(error)),
        };
        ModsScreen {
            game_folder: game_folder.to_path_buf(),
            mods,
            settings: saved.clone(),
            saved,
            settings_error,
            query: String::new(),
            selected: None,
        }
    }

    /// The indices of the mods the search shows.
    fn shown(&self) -> Vec<usize> {
        (0..self.mods.len())
            .filter(|&index| matches_query(&self.query, &self.mods[index].found))
            .collect()
    }

    /// What becomes of each mod at the next start, by index.
    fn states(&self) -> Vec<ModState> {
        let found: Vec<FoundMod> = self.mods.iter().map(|shown| shown.found.clone()).collect();
        let mut states = plan_loading(&found, &self.settings).states;
        for (state, shown) in states.iter_mut().zip(&self.mods) {
            if let (ModState::On, Some(error)) = (&*state, &shown.script_error) {
                *state = ModState::Broken(error.clone());
            }
        }
        states
    }

    fn is_on(&self, index: usize) -> bool {
        self.settings.is_on(&self.mods[index].found.id)
    }

    /// How many mods the player changed since the screen opened.
    fn changes(&self) -> usize {
        self.mods
            .iter()
            .filter(|shown| {
                self.settings.is_on(&shown.found.id) != self.saved.is_on(&shown.found.id)
            })
            .count()
    }

    /// Turns the mods of `indices` on or off; the ids of the mods turned off that no longer
    /// exist stay as they are.
    fn set_on(&mut self, indices: &[usize], on: bool) {
        for &index in indices {
            let id = self.mods[index].found.id.clone();
            self.settings.set_on(&id, on);
        }
    }
}

/// Whether `found` answers the search `query`: each of its words is in the name, the id, an
/// author or the description, whatever the case.
pub fn matches_query(query: &str, found: &FoundMod) -> bool {
    let about = &found.manifest.about;
    let haystack = format!(
        "{} {} {} {}",
        about.name,
        found.id,
        about.authors.join(" "),
        about.description
    )
    .to_lowercase();
    query
        .to_lowercase()
        .split_whitespace()
        .all(|word| haystack.contains(word))
}

/// The icon of `path`, drawn without smoothing, if it is a square PNG of a side the screen
/// shows.
fn read_icon(path: &Path) -> Option<Image> {
    let bytes = std::fs::read(path).ok()?;
    let image = Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::RENDER_WORLD,
    )
    .ok()?;
    let size = image.size();
    (size.x == size.y && ICON_SIDES.contains(&size.x)).then_some(image)
}

/// The card of the mod of this index.
#[derive(Component)]
pub struct ModCard(pub usize);

/// The switch of a card, which turns its mod on or off.
#[derive(Component)]
pub struct ModSwitch(pub usize);

/// The list of the cards.
#[derive(Component)]
pub struct ModList;

/// The search field.
#[derive(Component)]
pub struct ModSearch;

/// The count of the mods on, by the title.
#[derive(Component)]
pub struct ModCount;

/// Turn all on, or Turn all off, for the mods the search shows.
#[derive(Component, Clone, Copy)]
pub enum TurnAll {
    On,
    Off,
}

/// The detail pane, hidden while no mod is shown.
#[derive(Component)]
pub struct ModDetail;

/// What the detail pane shows of the selected mod, above its button.
#[derive(Component)]
struct ModFacts;

/// The button of the detail pane that turns the selected mod on or off.
#[derive(Component)]
pub struct DetailToggle;

/// The line of the footer saying what changed, or why `mods.ron` cannot be read.
#[derive(Component)]
pub struct ModsStatus;

/// Revert, in the footer while something changed.
#[derive(Component)]
pub struct RevertButton;

/// The hint bar of the screen.
#[derive(Component, Default)]
struct ModsHints;

const SEARCH_HINT: Hint = Hint {
    keys: "/",
    action: "Search",
};
const BACK_HINT: Hint = Hint {
    keys: "Esc",
    action: "Back",
};

/// The key hints while a card has the focus.
pub const CARD_HINTS: [Hint; 4] = [
    Hint {
        keys: "↑↓",
        action: "Select",
    },
    Hint {
        keys: "Space",
        action: "Turn on or off",
    },
    SEARCH_HINT,
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
    SEARCH_HINT,
    BACK_HINT,
];

impl ListHints for ModsHints {
    const CARDS: &'static [Hint] = &CARD_HINTS;
    const SCREEN: &'static [Hint] = &SCREEN_HINTS;
}

/// Builds the cards again, and the rest of the screen with them.
#[derive(Event)]
struct ShowMods {
    /// Whether the selected card takes the focus. Otherwise it takes it only from a card
    /// that was built again.
    focus_card: bool,
}

pub fn mods_plugin(app: &mut App) {
    app.add_systems(OnEnter(MenuState::Mods), mods_setup)
        .add_systems(OnExit(MenuState::Mods), save_on_exit)
        .add_systems(
            Update,
            (
                follow_focus_with_hints::<ModsHints>,
                read_search,
                focus_search_on_slash,
            )
                .run_if(in_state(MenuState::Mods)),
        )
        .add_observer(show_mods)
        .add_observer(toggle_on_key)
        .add_observer(act_on_button);
}

/// Reads the mods and spawns the screen, the first card selected.
fn mods_setup(
    paths: Res<GameFolderPaths>,
    mut images: ResMut<Assets<Image>>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let screen = ModsScreen::read(&paths.game_folder_path, &mut images);
    commands.insert_resource(screen);
    let font: Handle<Font> = assets.load(FONT_PATH);
    let root = commands
        .spawn((
            menu_background(&assets),
            DespawnOnExit(MenuState::Mods),
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
    spawn_title_bar(&mut commands, content, &font);
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
    let scroll = spawn_scroll_list(&mut commands, WheelStep::Item);
    commands.entity(scroll.frame).insert(ChildOf(body));
    commands
        .entity(scroll.list)
        .insert((
            ModList,
            // the role before the label: the label's default node must not win
            AccessibilityNode(accesskit::Node::new(Role::ListBox)),
            AccessibleLabel::new(MODS_TITLE),
        ))
        .observe(show_selected_mod);
    spawn_detail_pane(&mut commands, body, &font);
    spawn_footer(&mut commands, content, &font);
    commands.spawn((list_hint_bar::<ModsHints>(&assets), ChildOf(root)));
    commands.trigger(ShowMods { focus_card: true });
}

/// The title, the count of the mods on, the search, Turn all on and Turn all off.
fn spawn_title_bar(commands: &mut Commands, content: Entity, font: &Handle<Font>) {
    let bar = commands
        .spawn((
            Node {
                align_items: AlignItems::Center,
                column_gap: px(SPACE_M),
                ..default()
            },
            ChildOf(content),
            children![
                text(MODS_TITLE, FONT_XL, TEXT, font),
                (ModCount, text("", FONT_S, MUTED, font)),
                Node {
                    flex_grow: 1.0,
                    ..default()
                },
            ],
        ))
        .id();
    let field = spawn_text_field(
        commands,
        &TextField {
            label: "Search mods",
            placeholder: "Search",
            max_characters: Some(64),
            font: font.clone(),
            font_size: FONT_M,
            reserve_error: false,
        },
    );
    commands
        .entity(field.input)
        .insert(ModSearch)
        .observe(clear_or_leave_search)
        .observe(
            |input: On<FocusedInput<KeyboardInput>>, mut commands: Commands| {
                let key = &input.input;
                if key.key_code == KeyCode::ArrowDown && key.state == ButtonState::Pressed {
                    commands.trigger(ShowMods { focus_card: true });
                }
            },
        );
    commands
        .spawn((
            Node {
                width: px(SEARCH_WIDTH),
                flex_shrink: 1.0,
                min_width: px(0),
                ..default()
            },
            ChildOf(bar),
        ))
        .add_child(field.root);
    for (label, action) in [("Turn all on", TurnAll::On), ("Turn all off", TurnAll::Off)] {
        commands.spawn((
            button(label, ButtonVariant::Default, font.clone()),
            action,
            ChildOf(bar),
        ));
    }
}

/// The pane of the selected mod: what it is, and the button that turns it on or off.
fn spawn_detail_pane(commands: &mut Commands, body: Entity, font: &Handle<Font>) {
    let pane = commands
        .spawn((
            ModDetail,
            Node {
                width: px(DETAIL_WIDTH),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(px(SPACE_L)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                row_gap: px(SPACE_L),
                overflow: Overflow::clip_y(),
                ..default()
            },
            BackgroundColor(PANEL),
            raised_bevel(),
            ChildOf(body),
        ))
        .id();
    commands.spawn((
        ModFacts,
        Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(SPACE_XS),
            min_width: px(0),
            ..default()
        },
        ChildOf(pane),
    ));
    commands.spawn((
        button("Turn off", ButtonVariant::Default, font.clone()),
        DetailToggle,
        ChildOf(pane),
    ));
}

/// The line of what changed, Revert, Open folder and Done.
fn spawn_footer(commands: &mut Commands, content: Entity, font: &Handle<Font>) {
    commands.spawn((
        Node {
            align_items: AlignItems::Center,
            column_gap: px(SPACE_S),
            ..default()
        },
        ChildOf(content),
        children![
            (
                ModsStatus,
                text("", FONT_S, ACCENT, font),
                Node {
                    flex_grow: 1.0,
                    min_width: px(0),
                    ..default()
                },
            ),
            (
                button("Revert", ButtonVariant::Ghost, font.clone()),
                RevertButton,
                observe(revert),
            ),
            (
                button("Open folder", ButtonVariant::Default, font.clone()),
                observe(open_mods_folder),
            ),
            (
                button("Done", ButtonVariant::Primary, font.clone()),
                observe(
                    |_: On<Activate>, mut menu_state: ResMut<NextState<MenuState>>| {
                        menu_state.set(MenuState::Main);
                    }
                ),
            ),
        ],
    ));
}

/// Spawns an icon of `side` pixels in `parent`: the mod's own, else a square in a colour of
/// its id.
fn spawn_icon<'a>(
    commands: &'a mut Commands,
    parent: Entity,
    shown: &ShownMod,
    side: f32,
) -> EntityCommands<'a> {
    let mut hasher = DefaultHasher::new();
    shown.found.id.hash(&mut hasher);
    let background = match shown.icon {
        Some(_) => SLOT,
        None => thumbnail_color(Some(hasher.finish() as u32)),
    };
    let mut icon = commands.spawn((
        Node {
            width: px(side),
            height: px(side),
            flex_shrink: 0.0,
            border: UiRect::all(px(BEVEL_WIDTH)),
            ..default()
        },
        BackgroundColor(background),
        sunken_bevel(),
        ChildOf(parent),
    ));
    if let Some(image) = &shown.icon {
        icon.insert(ImageNode::new(image.clone()));
    }
    icon
}

/// The label and the colour of what the card of a mod in `state` says next to its switch.
fn chip(state: &ModState) -> Option<(String, Color)> {
    match state {
        ModState::On | ModState::Off => None,
        ModState::Broken(_) => Some(("Cannot load".into(), DANGER)),
        ModState::Waiting(needs) => {
            let ids: Vec<&str> = needs.iter().map(|need| need.id.as_str()).collect();
            Some((format!("Needs {}", ids.join(", ")), ACCENT))
        }
    }
}

/// A switch, on or off, which turns the mod `index` on or off when clicked. Not a tab stop:
/// Space on the card does the same.
fn switch(index: usize, on: bool) -> impl Bundle {
    let mut node = accesskit::Node::new(Role::Switch);
    node.set_toggled(if on { Toggled::True } else { Toggled::False });
    (
        ModSwitch(index),
        AccessibilityNode(node),
        Node {
            width: px(SWITCH_WIDTH),
            height: px(SWITCH_HEIGHT),
            flex_shrink: 0.0,
            border: UiRect::all(px(BEVEL_WIDTH)),
            padding: UiRect::all(px(2.0)),
            justify_content: if on {
                JustifyContent::FlexEnd
            } else {
                JustifyContent::FlexStart
            },
            ..default()
        },
        BackgroundColor(if on { OK } else { SLOT }),
        sunken_bevel(),
        children![(
            Node {
                width: px(KNOB),
                height: px(KNOB),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(if on { TEXT } else { MUTED }),
            raised_bevel(),
            Pickable::IGNORE,
        )],
    )
}

/// One line of text that never wraps, clipped at the width it is given.
fn one_line(content: &str, size: f32, color: Color, font: &Handle<Font>) -> impl Bundle {
    (
        Node {
            min_width: px(0),
            overflow: Overflow::clip(),
            ..default()
        },
        children![(
            Text::new(content),
            TextFont {
                font: font.clone().into(),
                font_size: px(size).into(),
                ..default()
            },
            TextColor(color),
            TextLayout::no_wrap(),
        )],
    )
}

/// Spawns the card of the mod `index` at the end of `list`, and returns it.
fn spawn_card(
    commands: &mut Commands,
    list: Entity,
    screen: &ModsScreen,
    index: usize,
    state: &ModState,
    font: &Handle<Font>,
) -> Entity {
    let shown = &screen.mods[index];
    let on = screen.is_on(index);
    let label = format!("{}, {}", shown.found.name(), if on { "on" } else { "off" });
    let card = spawn_list_card(commands, list, &label);
    let dim = |color: Color| if on { color } else { color.with_alpha(0.5) };
    let about = &shown.found.manifest.about;
    // a second line on every card, so that they are all as high
    let first_line = match about.description.lines().next() {
        Some(line) => line,
        None if shown.found.problem.is_some() => "Its mod.toml cannot be read",
        None if !shown.found.declared => "No mod.toml: named after its folder",
        None => "No description",
    };
    commands.entity(card).insert(ModCard(index));
    spawn_icon(commands, card, shown, CARD_ICON);
    commands.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: px(0),
            row_gap: px(SPACE_XS),
            ..default()
        },
        ChildOf(card),
        children![
            (
                Node {
                    column_gap: px(SPACE_S),
                    align_items: AlignItems::Baseline,
                    min_width: px(0),
                    ..default()
                },
                children![
                    one_line(shown.found.name(), FONT_M, dim(TEXT), font),
                    text(&shown.found.version().to_string(), FONT_S, dim(MUTED), font),
                ],
            ),
            one_line(first_line, FONT_S, dim(MUTED), font),
        ],
    ));
    if let Some((label, color)) = chip(state) {
        commands.spawn((
            Node {
                flex_shrink: 0.0,
                border: UiRect::all(px(1.0)),
                padding: UiRect::axes(px(SPACE_XS), px(0)),
                ..default()
            },
            BorderColor::all(color),
            ChildOf(card),
            children![text(&label, FONT_S, color, font)],
        ));
    }
    commands
        .spawn((switch(index, on), ChildOf(card)))
        .observe(toggle_on_click);
    card
}

/// Builds the cards of the mods the search shows, and the rest of the screen with them: the
/// count, the detail pane, Turn all on and off, and the footer.
#[allow(clippy::too_many_arguments)]
fn show_mods(
    event: On<ShowMods>,
    mut screen: ResMut<ModsScreen>,
    lists: Query<Entity, With<ModList>>,
    old_cards: Query<(), With<ModCard>>,
    facts: Query<Entity, With<ModFacts>>,
    mut panes: Query<&mut Node, With<ModDetail>>,
    mut texts: Query<&mut Text>,
    counts: Query<Entity, With<ModCount>>,
    turn_all: Query<(Entity, &TurnAll, &Children, &mut TabIndex)>,
    toggles: Query<(Entity, &Children), With<DetailToggle>>,
    mut status: Query<(Entity, &mut TextColor), With<ModsStatus>>,
    mut reverts: Query<&mut Node, (With<RevertButton>, Without<ModDetail>)>,
    mut list_focus: ListFocus,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let Ok(list) = lists.single() else {
        return;
    };
    let font: Handle<Font> = assets.load(FONT_PATH);
    let states = screen.states();
    let shown = screen.shown();
    // the selected mod stays selected while the search shows it, else the first shown is
    let selected = screen
        .selected
        .as_ref()
        .and_then(|id| shown.iter().position(|&i| &screen.mods[i].found.id == id))
        .unwrap_or(0);
    screen.selected = shown
        .get(selected)
        .map(|&i| screen.mods[i].found.id.clone());

    commands.entity(list).despawn_related::<Children>();
    commands.entity(list).remove::<ListEmpty>();
    let cards: Vec<Entity> = shown
        .iter()
        .map(|&index| spawn_card(&mut commands, list, &screen, index, &states[index], &font))
        .collect();
    let focus_was_on_card = list_focus
        .focus
        .get()
        .is_some_and(|entity| old_cards.contains(entity));
    match cards.get(selected) {
        Some(&card) => {
            commands.entity(card).insert((Selected, TabIndex(0)));
            if event.focus_card || focus_was_on_card {
                list_focus.set(card, true);
            }
        }
        None => {
            commands.entity(list).insert(ListEmpty);
            spawn_empty_state(&mut commands, list, &screen, &font);
        }
    }

    // the count by the title
    let on = (0..screen.mods.len()).filter(|&i| screen.is_on(i)).count();
    for count in &counts {
        if let Ok(mut count) = texts.get_mut(count) {
            count.0 = format!("{on} of {} on", screen.mods.len());
        }
    }
    // the detail pane
    for mut pane in &mut panes {
        pane.display = if cards.is_empty() {
            Display::None
        } else {
            Display::Flex
        };
    }
    let selected_index = shown.get(selected).copied();
    if let (Ok(facts), Some(index)) = (facts.single(), selected_index) {
        show_detail(&mut commands, facts, &screen, index, &states[index], &font);
    }
    if let (Ok((toggle, children)), Some(index)) = (toggles.single(), selected_index) {
        let label = if screen.is_on(index) {
            "Turn off"
        } else {
            "Turn on"
        };
        set_button_label(&mut commands, &mut texts, toggle, children, label);
    }
    // Turn all on and off say how many when the search hides some
    let partial = shown.len() < screen.mods.len();
    for (button, action, children, mut tab_index) in turn_all {
        let label = match (action, partial) {
            (TurnAll::On, false) => "Turn all on".to_string(),
            (TurnAll::Off, false) => "Turn all off".to_string(),
            (TurnAll::On, true) => format!("Turn {} on", shown.len()),
            (TurnAll::Off, true) => format!("Turn {} off", shown.len()),
        };
        set_button_label(&mut commands, &mut texts, button, children, &label);
        tab_index.set_if_neq(TabIndex(if shown.is_empty() { -1 } else { 0 }));
    }
    // the footer
    let changes = screen.changes();
    let (line, color) = match (&screen.settings_error, changes) {
        (_, 1) => ("1 change, applied when a world starts".to_string(), ACCENT),
        (_, n) if n > 1 => (format!("{n} changes, applied when a world starts"), ACCENT),
        (Some(error), _) => (format!("{error}: every mod is on"), DANGER),
        (None, _) => (String::new(), ACCENT),
    };
    for (entity, mut text_color) in &mut status {
        text_color.0 = color;
        if let Ok(mut text) = texts.get_mut(entity) {
            text.0 = line.clone();
        }
    }
    for mut revert in &mut reverts {
        revert.display = if changes > 0 {
            Display::Flex
        } else {
            Display::None
        };
    }
}

/// Puts `label` on the text of the button `button`, built by `button`, and on its accessible
/// label.
fn set_button_label(
    commands: &mut Commands,
    texts: &mut Query<&mut Text>,
    button: Entity,
    children: &Children,
    label: &str,
) {
    let Some(child) = children.iter().find(|&child| texts.contains(child)) else {
        return;
    };
    if let Ok(mut text) = texts.get_mut(child) {
        if text.0 != label {
            text.0 = label.to_string();
            commands.entity(button).insert(AccessibleLabel::new(label));
        }
    }
}

/// What the list says when it has no card: no mod in the folder, or none the search shows.
fn spawn_empty_state(
    commands: &mut Commands,
    list: Entity,
    screen: &ModsScreen,
    font: &Handle<Font>,
) {
    let (title, hint) = if screen.mods.is_empty() {
        let folder = screen.game_folder.join(MODS_FOLDER);
        (
            "No mods yet".to_string(),
            format!(
                "Copy the folder of a mod into {}, then open this screen again.",
                folder.display()
            ),
        )
    } else {
        (
            format!("No mod matches \u{201c}{}\u{201d}", screen.query.trim()),
            "Esc clears the search.".to_string(),
        )
    };
    commands.spawn((
        Node {
            flex_grow: 1.0,
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            row_gap: px(SPACE_S),
            padding: UiRect::all(px(SPACE_L)),
            ..default()
        },
        ChildOf(list),
        children![
            text(&title, FONT_L, TEXT, font),
            text(&hint, FONT_S, MUTED, font),
        ],
    ));
}

/// Shows the mod `index` in the detail pane, in place of what it showed.
fn show_detail(
    commands: &mut Commands,
    facts: Entity,
    screen: &ModsScreen,
    index: usize,
    state: &ModState,
    font: &Handle<Font>,
) {
    let shown = &screen.mods[index];
    let found = &shown.found;
    let about = &found.manifest.about;
    commands.entity(facts).despawn_related::<Children>();
    spawn_icon(commands, facts, shown, DETAIL_ICON)
        .entry::<Node>()
        .and_modify(|mut node| node.margin = UiRect::bottom(px(SPACE_S)));
    let mut line = |content: &str, size: f32, color: Color| {
        commands.spawn((text(content, size, color, font), ChildOf(facts)));
    };
    line(found.name(), FONT_L, TEXT);
    let by = match about.authors.is_empty() {
        true => found.version().to_string(),
        false => format!("{} · by {}", found.version(), about.authors.join(", ")),
    };
    line(&by, FONT_S, MUTED);
    if !about.description.is_empty() {
        line(&about.description, FONT_S, TEXT);
    }
    match state {
        ModState::Broken(problem) => line(problem, FONT_S, DANGER),
        ModState::Waiting(needs) => {
            for need in needs {
                line(&format!("Needs {need}"), FONT_S, ACCENT);
            }
        }
        ModState::On | ModState::Off => (),
    }
    line(
        &format!("Folder: {MODS_FOLDER}/{}/", found.id),
        FONT_S,
        MUTED,
    );
    let dependencies: Vec<&str> = found
        .manifest
        .dependencies
        .keys()
        .map(String::as_str)
        .collect();
    if !dependencies.is_empty() {
        line(
            &format!("Needs: {}", dependencies.join(", ")),
            FONT_S,
            MUTED,
        );
    }
    let optional: Vec<&str> = found.manifest.optional.keys().map(String::as_str).collect();
    if !optional.is_empty() {
        line(
            &format!("Works with: {}", optional.join(", ")),
            FONT_S,
            MUTED,
        );
    }
    let needed_by: Vec<&str> = screen
        .mods
        .iter()
        .filter(|other| other.found.manifest.dependencies.contains_key(&found.id))
        .map(|other| other.found.name())
        .collect();
    if !needed_by.is_empty() {
        line(
            &format!("Needed by: {}", needed_by.join(", ")),
            FONT_S,
            MUTED,
        );
    }
    if found.entry().is_none() && found.problem.is_none() {
        line("No code", FONT_S, MUTED);
    }
    if let Some(license) = &about.license {
        line(&format!("License: {license}"), FONT_S, MUTED);
    }
    if let Some(homepage) = &about.homepage {
        line(homepage, FONT_S, MUTED);
    }
}

/// Shows the mod of the card just selected in the detail pane.
#[allow(clippy::too_many_arguments)]
fn show_selected_mod(
    selected: On<CardSelected>,
    cards: Query<&ModCard>,
    mut screen: ResMut<ModsScreen>,
    facts: Query<Entity, With<ModFacts>>,
    toggles: Query<(Entity, &Children), With<DetailToggle>>,
    mut texts: Query<&mut Text>,
    assets: Res<AssetServer>,
    mut commands: Commands,
) {
    let (Ok(&ModCard(index)), Ok(facts)) = (cards.get(selected.card), facts.single()) else {
        return;
    };
    screen.selected = Some(screen.mods[index].found.id.clone());
    let states = screen.states();
    let font = assets.load(FONT_PATH);
    show_detail(&mut commands, facts, &screen, index, &states[index], &font);
    if let Ok((toggle, children)) = toggles.single() {
        let label = if screen.is_on(index) {
            "Turn off"
        } else {
            "Turn on"
        };
        set_button_label(&mut commands, &mut texts, toggle, children, label);
    }
}

/// Turns the mod of the switch clicked on or off, and selects its card.
fn toggle_on_click(
    mut click: On<Pointer<Click>>,
    switches: Query<&ModSwitch>,
    mut screen: ResMut<ModsScreen>,
    mut commands: Commands,
) {
    let Ok(&ModSwitch(index)) = switches.get(click.entity) else {
        return;
    };
    click.propagate(false);
    toggle(&mut screen, index);
    commands.trigger(ShowMods { focus_card: true });
}

/// Turns the mod `index` on or off, and selects it.
fn toggle(screen: &mut ModsScreen, index: usize) {
    let on = screen.is_on(index);
    screen.set_on(&[index], !on);
    screen.selected = Some(screen.mods[index].found.id.clone());
}

/// Space or Enter on a focused card turns its mod on or off.
fn toggle_on_key(
    mut event: On<FocusedInput<KeyboardInput>>,
    cards: Query<&ModCard>,
    screen: Option<ResMut<ModsScreen>>,
    mut commands: Commands,
) {
    // the event bubbles: only the step on the card itself is handled
    let (Ok(&ModCard(index)), Some(mut screen)) = (cards.get(event.focused_entity), screen) else {
        return;
    };
    let input = &event.input;
    if input.state != ButtonState::Pressed || input.repeat {
        return;
    }
    if !matches!(input.key_code, KeyCode::Space | KeyCode::Enter) {
        return;
    }
    event.propagate(false);
    toggle(&mut screen, index);
    commands.trigger(ShowMods { focus_card: true });
}

/// Turn all on or off, and the button of the detail pane.
fn act_on_button(
    activate: On<Activate>,
    turn_all: Query<&TurnAll>,
    toggles: Query<(), With<DetailToggle>>,
    selected_cards: Query<&ModCard, With<Selected>>,
    screen: Option<ResMut<ModsScreen>>,
    mut commands: Commands,
) {
    let Some(mut screen) = screen else {
        return;
    };
    if let Ok(&action) = turn_all.get(activate.entity) {
        let shown = screen.shown();
        screen.set_on(&shown, matches!(action, TurnAll::On));
        commands.trigger(ShowMods { focus_card: false });
    } else if toggles.contains(activate.entity) {
        if let Ok(&ModCard(index)) = selected_cards.single() {
            toggle(&mut screen, index);
            commands.trigger(ShowMods { focus_card: false });
        }
    }
}

/// Revert: the mods as `mods.ron` has them.
fn revert(_: On<Activate>, mut screen: ResMut<ModsScreen>, mut commands: Commands) {
    screen.settings = screen.saved.clone();
    commands.trigger(ShowMods { focus_card: true });
}

/// Opens the folder of the mods in the file manager, made first when it is missing.
fn open_mods_folder(_: On<Activate>, screen: Res<ModsScreen>) {
    let folder = screen.game_folder.join(MODS_FOLDER);
    if let Err(err) = std::fs::create_dir_all(&folder) {
        warn!("Could not make the mods folder {}: {err}", folder.display());
        return;
    }
    let opener = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(err) = std::process::Command::new(opener).arg(&folder).spawn() {
        warn!("Could not open {}: {err}", folder.display());
    }
}

/// Follows the search field.
fn read_search(
    fields: Query<&EditableText, (With<ModSearch>, Changed<EditableText>)>,
    mut screen: ResMut<ModsScreen>,
    mut commands: Commands,
) {
    for field in &fields {
        let value = field.value().to_string();
        if value != screen.query {
            screen.query = value;
            commands.trigger(ShowMods { focus_card: false });
        }
    }
}

/// Escape in the search clears it, or leaves it once it is empty, for the selected card.
fn clear_or_leave_search(
    cancel: On<Cancel>,
    mut texts: Query<&mut EditableText>,
    mut commands: Commands,
) {
    let Ok(mut text) = texts.get_mut(cancel.entity) else {
        return;
    };
    if text.value() == "" {
        commands.trigger(ShowMods { focus_card: true });
    } else {
        set_text(&mut text, "");
    }
}

/// Focuses the search, with its ring, when `/` is typed and released outside a text field, or
/// on Ctrl+F. On the release, so that the slash is not typed in the field.
fn focus_search_on_slash(
    mut key_events: MessageReader<KeyboardInput>,
    keys: Res<ButtonInput<KeyCode>>,
    fields: Query<(), With<EditableText>>,
    search: Query<Entity, With<ModSearch>>,
    mut armed: Local<Option<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut visible: ResMut<InputFocusVisible>,
) {
    let in_field = focus.get().is_some_and(|entity| fields.contains(entity));
    let mut slash = false;
    for key in key_events.read() {
        match key.state {
            ButtonState::Pressed
                if !in_field && !key.repeat && key.logical_key == Key::Character("/".into()) =>
            {
                *armed = Some(key.key_code);
            }
            ButtonState::Released if *armed == Some(key.key_code) => {
                *armed = None;
                slash = true;
            }
            _ => {}
        }
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    if !slash && !(ctrl && keys.just_pressed(KeyCode::KeyF)) {
        return;
    }
    if let Ok(field) = search.single() {
        focus.set(field, FocusCause::Navigated);
        visible.0 = true;
    }
}

/// Writes `mods.ron` when the screen is left with changes, and says so.
fn save_on_exit(screen: Option<Res<ModsScreen>>, assets: Res<AssetServer>, mut commands: Commands) {
    let Some(screen) = screen else {
        return;
    };
    commands.remove_resource::<ModsScreen>();
    if screen.settings == screen.saved {
        return;
    }
    let font = assets.load(FONT_PATH);
    match write_mods_settings(&screen.game_folder, &screen.settings) {
        Ok(()) => {
            show_toast(
                &mut commands,
                "Mods saved: they apply when a world starts",
                ToastKind::Info,
                font,
            );
        }
        Err(err) => {
            warn!("Could not write {MODS_SETTINGS_PATH}: {err}");
            show_toast(
                &mut commands,
                &format!("Could not save {MODS_SETTINGS_PATH}"),
                ToastKind::Error,
                font,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::mods::parse_manifest;

    fn found(text: &str) -> FoundMod {
        let manifest = parse_manifest(text).unwrap();
        FoundMod {
            id: manifest.about.id.clone(),
            folder: PathBuf::from("/mods").join(&manifest.about.id),
            manifest,
            declared: true,
            problem: None,
        }
    }

    #[test]
    fn the_search_matches_each_word_in_the_name_id_authors_or_description() {
        let lanterns = found(
            "[mod]\nid = \"lanterns\"\nname = \"Lanterns\"\nversion = \"1.0.0\"\n\
             description = \"A light brighter than the torch\"\nauthors = [\"Ondine\"]\n[game]\napi = 1\n",
        );
        assert!(matches_query("", &lanterns));
        assert!(matches_query("LANTERN", &lanterns));
        assert!(matches_query("ondine torch", &lanterns));
        assert!(!matches_query("ondine castle", &lanterns));
    }

    #[test]
    fn a_waiting_mod_names_what_it_needs_and_a_broken_one_cannot_load() {
        use shared::mods::{Need, Unmet};
        assert_eq!(chip(&ModState::On), None);
        assert_eq!(chip(&ModState::Off), None);
        let waiting = ModState::Waiting(vec![Need {
            id: "lanterns".into(),
            unmet: Unmet::Off,
        }]);
        assert_eq!(chip(&waiting).unwrap().0, "Needs lanterns");
        assert_eq!(
            chip(&ModState::Broken("x".into())).unwrap().0,
            "Cannot load"
        );
    }
}
