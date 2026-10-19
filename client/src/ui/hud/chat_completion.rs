//! The completion of the commands typed in the chat. Over the field, a panel shows the usage of
//! the command, the argument being typed in the accent, or what the server would answer to the
//! line when it is already wrong; under it, what the word being typed may become, the part typed
//! in the accent. After the caret, the field shows in grey the rest of the chosen suggestion, or
//! the arguments left to type.
//!
//! Tab takes the chosen suggestion, Up and Down choose another, a click takes one, and Escape
//! puts the panel away until the text changes.

use bevy::input::{keyboard::KeyboardInput, ButtonState};
use bevy::input_focus::FocusedInput;
use bevy::prelude::*;
use bevy::text::{
    detect_text_needs_rerender, EditableText, EditableTextSystems, LineBreak, TextEdit,
    TextLayoutInfo,
};
use bevy::ui::widget::{scroll_editable_text, update_editable_text_layout, TextScroll};
use bevy::ui::{UiGlobalTransform, UiSystems};
use shared::commands::{complete, Completion, Suggestion};

use super::chat::ChatInput;
use crate::state::GameState;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::theme::{
    ACCENT, BEVEL_HI, BEVEL_LO, BEVEL_WIDTH, DANGER, FONT_S, MUTED, PANEL, RAISE, SPACE_L, SPACE_S,
    SPACE_XS, TEXT,
};
use crate::ui::widgets::hints::key_cap;
use crate::ui::widgets::text_field::TextFieldParts;

/// The suggestions the panel lists at once; Up and Down scroll to the others.
pub const MAX_ROWS: usize = 8;
/// The width of the mark at the left of the chosen suggestion.
const MARK_WIDTH: f32 = 2.0;

/// Follows the chat field and writes the panel before the texts are measured, then places the
/// ghost at the caret once the field is laid out.
pub fn chat_completion_plugin(app: &mut App) {
    app.add_systems(
        PostUpdate,
        (
            // with the edits of the frame, and before Bevy measures the texts written
            (follow_the_chat_field, draw_the_suggestions)
                .chain()
                .after(EditableTextSystems)
                .before(detect_text_needs_rerender),
            place_the_ghost
                .in_set(UiSystems::PostLayout)
                .after(update_editable_text_layout)
                .after(scroll_editable_text),
        )
            .run_if(in_state(GameState::Game)),
    );
}

/// The completion of the text of the chat field, on its input.
#[derive(Component, Default)]
pub struct CommandCompletion {
    /// The text before the caret that `completion` is for; `None` while text is selected or
    /// composed.
    line: Option<String>,
    /// Whether the caret is at the end of the text: only then does the ghost show.
    at_end: bool,
    completion: Option<Completion>,
    /// The chosen suggestion.
    selected: usize,
    /// The suggestion in the first row.
    first: usize,
    /// Whether Up, Down or the pointer chose the suggestion: the ghost shows it before a letter
    /// of the word is typed.
    chosen: bool,
    /// The text of the field for which the panel is put away.
    dismissed: Option<String>,
}

/// The chat commands the mods of the server add, as it told them when the player joined.
#[derive(Resource, Default)]
pub struct ModCommands(pub Vec<String>);

impl CommandCompletion {
    /// Follows the text of the field: `line` before the caret, `None` while text is selected or
    /// composed. A new line starts from its first suggestion; a text other than the one the
    /// panel was put away for brings it back.
    fn update(&mut self, value: &str, line: Option<&str>, mod_commands: &[String]) {
        if self
            .dismissed
            .as_deref()
            .is_some_and(|dismissed| dismissed != value)
        {
            self.dismissed = None;
        }
        self.at_end = line.is_some_and(|line| line.len() == value.len());
        if self.line.as_deref() != line {
            self.line = line.map(str::to_string);
            self.completion = line.and_then(|line| complete(line, mod_commands));
            self.selected = 0;
            self.first = 0;
            self.chosen = false;
        }
    }

    /// What the panel shows, while it shows something.
    fn shown(&self) -> Option<&Completion> {
        self.completion
            .as_ref()
            .filter(|_| self.dismissed.is_none())
            .filter(|completion| {
                !completion.suggestions.is_empty()
                    || !completion.usage.is_empty()
                    || completion.error.is_some()
            })
    }

    /// The suggestions to choose from, while the panel shows.
    pub fn suggestions(&self) -> &[Suggestion] {
        self.shown()
            .map_or(&[], |completion| &completion.suggestions[..])
    }

    /// Whether the panel shows.
    pub fn is_shown(&self) -> bool {
        self.shown().is_some()
    }

    /// Puts the panel away until the text of the field is no longer `value`.
    pub fn dismiss(&mut self, value: &str) {
        self.dismissed = Some(value.to_string());
    }

    /// Chooses the suggestion `step` rows down, round the list, and scrolls the rows to it.
    fn choose(&mut self, step: isize) {
        let count = self.suggestions().len() as isize;
        if count == 0 {
            return;
        }
        self.selected = (self.selected as isize + step).rem_euclid(count) as usize;
        self.chosen = true;
        if self.selected < self.first {
            self.first = self.selected;
        } else if self.selected >= self.first + MAX_ROWS {
            self.first = self.selected + 1 - MAX_ROWS;
        }
    }

    /// The edits that put the suggestion `index` in place of the word before the caret, with a
    /// space when an argument follows.
    fn take(&self, index: usize) -> Option<Vec<TextEdit>> {
        let completion = self.shown()?;
        let suggestion = completion.suggestions.get(index)?;
        let line = self.line.as_deref()?;
        let typed = line[completion.word_start..].chars().count();
        let mut edits = vec![TextEdit::Backspace; typed];
        let mut text = suggestion.text.clone();
        if completion.more {
            text.push(' ');
        }
        edits.push(TextEdit::Insert(text.into()));
        Some(edits)
    }

    /// The grey text after the caret: the rest of the chosen suggestion once a letter of the
    /// word is typed or a suggestion chosen, else the arguments left to type.
    pub fn ghost(&self) -> String {
        let (Some(completion), Some(line)) = (self.shown(), self.line.as_deref()) else {
            return String::new();
        };
        if completion.error.is_some() || !self.at_end {
            return String::new();
        }
        let word = &line[completion.word_start..];
        if !completion.suggestions.is_empty() && (!word.is_empty() || self.chosen) {
            let chosen = &completion.suggestions[self.selected].text;
            return chosen
                .strip_prefix(&word.to_lowercase())
                .unwrap_or_default()
                .to_string();
        }
        let Some(current) = completion.current else {
            return String::new();
        };
        if word.is_empty() {
            completion.usage[current..].join(" ")
        } else if current + 1 < completion.usage.len() {
            format!(" {}", completion.usage[current + 1..].join(" "))
        } else {
            String::new()
        }
    }
}

/// The panel of the suggestions, over the field of the chat.
#[derive(Component)]
pub struct SuggestionPanel;

/// The parts of the panel, spawned once and written as the completion changes: entities spawned
/// later would be laid out a frame late, and the panel would flicker at each letter typed.
#[derive(Component)]
struct PanelParts {
    /// The line at the top: what is wrong, or the usage of the command with spans before, at and
    /// after the argument being typed.
    header: Entity,
    usage: [Entity; 3],
    /// Which suggestion is chosen among how many, when they do not all fit in the rows.
    counter: Entity,
    rows: [RowParts; MAX_ROWS],
    /// The key hints at the bottom.
    hints: Entity,
    ghost: Entity,
}

/// A row of the panel, with spans before, at and after the part of the suggestion that the word
/// typed matches, then the detail.
struct RowParts {
    row: Entity,
    spans: [Entity; 4],
}

/// A row of the panel: the suggestion it shows is this many rows after the first one shown.
#[derive(Component)]
pub struct SuggestionRow(pub usize);

/// The grey text after the caret, in the frame of the field.
#[derive(Component)]
pub struct Ghost;

/// Puts the completion on the chat field `field`: its panel over the field, its ghost in the
/// frame, and its keys on the input.
pub fn spawn_command_completion(
    commands: &mut Commands,
    field: TextFieldParts,
    asset_server: &AssetServer,
) {
    let font: Handle<Font> = asset_server.load(FONT_PATH);
    let text_font = TextFont {
        font: font.clone().into(),
        font_size: px(FONT_S).into(),
        ..default()
    };
    let text = |color: Color| {
        (
            Text::default(),
            text_font.clone(),
            TextColor(color),
            Pickable::IGNORE,
        )
    };
    let spans = |commands: &mut Commands, parent: Entity, colors: [Color; 3]| {
        colors.map(|color| {
            commands
                .spawn((
                    TextSpan::default(),
                    text_font.clone(),
                    TextColor(color),
                    ChildOf(parent),
                ))
                .id()
        })
    };

    let ghost = commands
        .spawn((
            Ghost,
            text(MUTED),
            TextLayout {
                linebreak: LineBreak::NoWrap,
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                left: px(SPACE_S),
                top: px(SPACE_S),
                ..default()
            },
            Visibility::Hidden,
            ChildOf(field.frame),
        ))
        .id();

    let panel = commands
        .spawn((
            SuggestionPanel,
            Node {
                display: Display::None,
                position_type: PositionType::Absolute,
                // just above the field, over the bottom of the log
                bottom: percent(100),
                margin: UiRect::bottom(px(SPACE_XS)),
                left: px(0),
                min_width: px(200),
                max_width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                padding: UiRect::all(px(SPACE_XS)),
                border: UiRect::all(px(BEVEL_WIDTH)),
                ..default()
            },
            BackgroundColor(PANEL),
            BorderColor {
                top: BEVEL_HI,
                left: BEVEL_HI,
                bottom: BEVEL_LO,
                right: BEVEL_LO,
            },
            ChildOf(field.root),
        ))
        .id();
    let top = commands
        .spawn((
            Node {
                justify_content: JustifyContent::SpaceBetween,
                column_gap: px(SPACE_L),
                padding: UiRect::horizontal(px(SPACE_XS)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
        ))
        .id();
    let header = commands.spawn((text(MUTED), ChildOf(top))).id();
    let usage = spans(commands, header, [MUTED, ACCENT, MUTED]);
    let counter = commands.spawn((text(MUTED), ChildOf(top))).id();

    let column = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ChildOf(panel),
        ))
        .id();
    let rows = std::array::from_fn(|slot| {
        let row = commands
            .spawn((
                SuggestionRow(slot),
                Node {
                    display: Display::None,
                    padding: UiRect::horizontal(px(SPACE_XS)),
                    border: UiRect::left(px(MARK_WIDTH)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                BorderColor::all(Color::NONE),
                ChildOf(column),
            ))
            .observe(choose_on_hover)
            .observe(take_on_click)
            .id();
        let label = commands.spawn((text(TEXT), ChildOf(row))).id();
        let [before, matched, after] = spans(commands, label, [TEXT, ACCENT, TEXT]);
        let [detail, ..] = spans(commands, label, [MUTED; 3]);
        RowParts {
            row,
            spans: [before, matched, after, detail],
        }
    });

    let glyph_font: Handle<Font> = asset_server.load(MONO_FONT_PATH);
    let hint = |keys: &'static str, action: &'static str| {
        (
            Node {
                column_gap: px(SPACE_XS),
                align_items: AlignItems::Center,
                ..default()
            },
            Pickable::IGNORE,
            children![
                key_cap(keys, font.clone(), glyph_font.clone()),
                (
                    Text::new(action),
                    text_font.clone(),
                    TextColor(MUTED),
                    Pickable::IGNORE
                ),
            ],
        )
    };
    let hints = commands
        .spawn((
            Node {
                column_gap: px(SPACE_S),
                padding: UiRect::horizontal(px(SPACE_XS)),
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(panel),
            children![
                hint("Tab", "complete"),
                hint("↑↓", "choose"),
                hint("Esc", "hide")
            ],
        ))
        .id();

    commands.entity(panel).insert(PanelParts {
        header,
        usage,
        counter,
        rows,
        hints,
        ghost,
    });
    commands
        .entity(field.input)
        .insert(CommandCompletion::default())
        .observe(complete_on_keys);
}

/// Follows the text of the chat field, once the edits of the frame are applied.
fn follow_the_chat_field(
    mut fields: Query<(&EditableText, &mut CommandCompletion), Changed<EditableText>>,
    mod_commands: Option<Res<ModCommands>>,
) {
    let mod_commands = mod_commands
        .as_ref()
        .map_or(&[][..], |commands| &commands.0[..]);
    for (text, mut completion) in &mut fields {
        let value = text.value().to_string();
        let selection = text.editor().raw_selection();
        let line = (selection.is_collapsed() && !text.is_composing())
            .then(|| value.get(..selection.focus().index()))
            .flatten();
        // written only on a change, which draws the panel again: the caret blinking is none
        let at_end = line.is_some_and(|line| line.len() == value.len());
        let undismissed = completion
            .dismissed
            .as_deref()
            .is_some_and(|dismissed| dismissed != value);
        if completion.line.as_deref() != line || completion.at_end != at_end || undismissed {
            completion.update(&value, line, mod_commands);
        }
    }
}

/// Replaces `text` with `value` unless it reads so already: a text written is measured again.
fn write(text: &mut String, value: &str) {
    if text != value {
        value.clone_into(text);
    }
}

/// Writes the panel and the text of the ghost when the completion changes.
fn draw_the_suggestions(
    completions: Query<&CommandCompletion, Changed<CommandCompletion>>,
    panels: Query<(Entity, &PanelParts)>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
    mut spans: Query<&mut TextSpan>,
    mut colors: Query<&mut TextColor>,
    mut grounds: Query<(&mut BackgroundColor, &mut BorderColor)>,
) {
    let (Ok(completion), Ok((panel, parts))) = (completions.single(), panels.single()) else {
        return;
    };
    let mut display = |entity: Entity, shown: bool| {
        if let Ok(mut node) = nodes.get_mut(entity) {
            let display = if shown { Display::Flex } else { Display::None };
            if node.display != display {
                node.display = display;
            }
        }
    };
    let mut span = |entity: Entity, value: &str| {
        if let Ok(mut span) = spans.get_mut(entity) {
            write(&mut span.0, value);
        }
    };

    if let Ok(mut ghost) = texts.get_mut(parts.ghost) {
        write(&mut ghost.0, &completion.ghost());
    }
    let shown = completion.shown();
    display(panel, shown.is_some());
    // the panel keeps its content while hidden
    let Some(shown) = shown else {
        return;
    };

    // what is wrong, or the usage with the argument being typed in the accent
    let header = shown.error.as_deref().unwrap_or_default();
    if let Ok(mut text) = texts.get_mut(parts.header) {
        write(&mut text.0, header);
    }
    if let Ok(mut color) = colors.get_mut(parts.header) {
        color.0 = if shown.error.is_some() { DANGER } else { MUTED };
    }
    let usage = if shown.error.is_some() {
        &[][..]
    } else {
        &shown.usage[..]
    };
    let current = shown.current.unwrap_or(usage.len()).min(usage.len());
    let words = |words: &[String]| words.join(" ");
    let gap = |before: bool| if before { " " } else { "" };
    span(parts.usage[0], &words(&usage[..current]));
    span(
        parts.usage[1],
        &usage
            .get(current)
            .map_or(String::new(), |word| format!("{}{word}", gap(current > 0))),
    );
    let after = usage.get(current + 1..).unwrap_or_default();
    span(
        parts.usage[2],
        &format!("{}{}", gap(!after.is_empty()), words(after)),
    );

    let count = shown.suggestions.len();
    let counter = if count > MAX_ROWS {
        format!("{}/{count}", completion.selected + 1)
    } else {
        String::new()
    };
    if let Ok(mut text) = texts.get_mut(parts.counter) {
        write(&mut text.0, &counter);
    }

    for (slot, row) in parts.rows.iter().enumerate() {
        let index = completion.first + slot;
        let Some(suggestion) = shown.suggestions.get(index) else {
            display(row.row, false);
            continue;
        };
        display(row.row, true);
        let chosen = index == completion.selected;
        if let Ok((mut ground, mut mark)) = grounds.get_mut(row.row) {
            ground.0 = if chosen { RAISE } else { Color::NONE };
            *mark = BorderColor::all(if chosen { ACCENT } else { Color::NONE });
        }
        let (text, matched) = (&suggestion.text, suggestion.matched.clone());
        span(row.spans[0], &text[..matched.start]);
        span(row.spans[1], &text[matched.clone()]);
        span(row.spans[2], &text[matched.end..]);
        let detail = if suggestion.detail.is_empty() {
            String::new()
        } else {
            format!(" {}", suggestion.detail)
        };
        span(row.spans[3], &detail);
    }
    display(parts.hints, count > 0);
}

/// Places the ghost at the caret, once the field is laid out: on the frame of the edit, the
/// ghost moves with the caret, without waiting for the next layout. It shows while the caret is
/// at the end of the text, and while it fits in the field.
fn place_the_ghost(
    inputs: Query<
        (
            &TextLayoutInfo,
            &TextScroll,
            &ComputedNode,
            &CommandCompletion,
        ),
        With<ChatInput>,
    >,
    mut ghosts: Query<
        (
            &Text,
            &mut Node,
            &ComputedNode,
            &mut UiGlobalTransform,
            &mut Visibility,
        ),
        (With<Ghost>, Without<ChatInput>),
    >,
) {
    let Ok((info, scroll, field, completion)) = inputs.single() else {
        return;
    };
    let Ok((text, mut node, ghost, mut transform, mut visibility)) = ghosts.single_mut() else {
        return;
    };
    // physical px, as the layout
    let caret = info.cursor.map(|(_, rect)| rect.min.x - scroll.0.x);
    let shown = match caret {
        Some(caret) if !text.0.is_empty() && completion.is_shown() => {
            let fits = caret + ghost.size().x <= field.size().x;
            fits.then_some(caret)
        }
        _ => None,
    };
    visibility.set_if_neq(if shown.is_some() {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    });
    let Some(caret) = shown else {
        return;
    };
    let scale = field.inverse_scale_factor;
    let left = SPACE_S + caret * scale;
    let Val::Px(laid_out) = node.left else {
        return;
    };
    if laid_out != left {
        node.left = px(left);
        let mut affine = transform.affine();
        affine.translation.x += (left - laid_out) / scale;
        *transform = affine.into();
    }
}

/// Tab takes the chosen suggestion, and Up and Down choose another, while the panel lists some.
/// Bevy moves the caret on Up and Down first: that move is dropped.
fn complete_on_keys(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut fields: Query<(&mut EditableText, &mut CommandCompletion)>,
) {
    let key = &event.input;
    if key.state != ButtonState::Pressed {
        return;
    }
    let Ok((mut text, mut completion)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    if text.is_composing() || completion.suggestions().is_empty() {
        return;
    }
    match key.key_code {
        KeyCode::Tab if !key.repeat => {
            if let Some(edits) = completion.take(completion.selected) {
                for edit in edits {
                    text.queue_edit(edit);
                }
            }
        }
        KeyCode::ArrowUp | KeyCode::ArrowDown => {
            text.pending_edits
                .retain(|edit| !matches!(edit, TextEdit::Up(_) | TextEdit::Down(_)));
            completion.choose(if key.key_code == KeyCode::ArrowUp {
                -1
            } else {
                1
            });
        }
        _ => return,
    }
    event.propagate(false);
}

/// The pointer over a row chooses its suggestion.
fn choose_on_hover(
    over: On<Pointer<Over>>,
    rows: Query<&SuggestionRow>,
    mut completions: Query<&mut CommandCompletion>,
) {
    let (Ok(row), Ok(mut completion)) = (rows.get(over.entity), completions.single_mut()) else {
        return;
    };
    let index = completion.first + row.0;
    if completion.selected != index {
        completion.selected = index;
        completion.chosen = true;
    }
}

/// A click on a row takes its suggestion.
fn take_on_click(
    click: On<Pointer<Click>>,
    rows: Query<&SuggestionRow>,
    mut fields: Query<(&mut EditableText, &CommandCompletion)>,
) {
    let (Ok(row), Ok((mut text, completion))) = (rows.get(click.entity), fields.single_mut())
    else {
        return;
    };
    if let Some(edits) = completion.take(completion.first + row.0) {
        for edit in edits {
            text.queue_edit(edit);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The completion of `line`, typed with the caret at its end.
    fn typed(line: &str) -> CommandCompletion {
        let mut completion = CommandCompletion::default();
        completion.update(line, Some(line), &[]);
        completion
    }

    #[test]
    fn the_ghost_shows_the_arguments_left_then_the_rest_of_the_chosen_suggestion() {
        assert_eq!(typed("/give").ghost(), " <item> <count>");
        assert_eq!(typed("/give ").ghost(), "<item> <count>");
        assert_eq!(typed("/give oa").ghost(), "k_leaves");
        assert_eq!(typed("/give OA").ghost(), "k_leaves");
        assert_eq!(typed("/give dirt ").ghost(), "<count>");
        assert_eq!(typed("/give dirt 3").ghost(), "");
        // a suggestion that only contains the word has no rest
        assert_eq!(typed("/give log").ghost(), "");
        // a wrong line has none
        assert_eq!(typed("/give diamond ").ghost(), "");
        assert_eq!(typed("hello").ghost(), "");

        // a suggestion chosen shows before a letter is typed
        let mut completion = typed("/time set ");
        assert_eq!(completion.ghost(), "<day|night>");
        completion.choose(1);
        assert_eq!(completion.ghost(), "night");
    }

    #[test]
    fn the_ghost_shows_only_with_the_caret_at_the_end() {
        let mut completion = CommandCompletion::default();
        completion.update("/give dirt", Some("/give"), &[]);
        assert_eq!(completion.ghost(), "");
        // text selected
        completion.update("/give dirt", None, &[]);
        assert!(!completion.is_shown());
    }

    #[test]
    fn up_and_down_choose_round_the_list_and_scroll_the_rows() {
        let mut completion = typed("/give ");
        let count = completion.suggestions().len();
        assert!(count > MAX_ROWS);
        completion.choose(-1);
        assert_eq!(completion.selected, count - 1);
        assert_eq!(completion.first, count - MAX_ROWS);
        completion.choose(1);
        assert_eq!((completion.selected, completion.first), (0, 0));
        for _ in 0..MAX_ROWS {
            completion.choose(1);
        }
        assert_eq!((completion.selected, completion.first), (MAX_ROWS, 1));
    }

    #[test]
    fn a_suggestion_taken_replaces_the_word_with_a_space_when_an_argument_follows() {
        let edits = typed("/ti").take(0).unwrap();
        assert_eq!(
            edits,
            [
                TextEdit::Backspace,
                TextEdit::Backspace,
                TextEdit::Insert("time ".into())
            ]
        );
        let edits = typed("/time set NI").take(0).unwrap();
        assert_eq!(edits.last(), Some(&TextEdit::Insert("night".into())));
        assert_eq!(edits.len(), 3);
    }

    #[test]
    fn escape_puts_the_panel_away_until_the_text_changes() {
        let mut completion = typed("/give ");
        completion.dismiss("/give ");
        assert!(!completion.is_shown());
        completion.update("/give ", Some("/give "), &[]);
        assert!(!completion.is_shown());
        completion.update("/give d", Some("/give d"), &[]);
        assert!(completion.is_shown());
    }
}
