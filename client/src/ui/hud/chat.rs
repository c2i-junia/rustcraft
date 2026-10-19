//! The chat: a log of the last messages with their times, over the field to write in, at the
//! bottom-left above the hotbar. Closed, the recent lines stay over the game a few seconds.

use std::time::{Duration, Instant};

use bevy::input::{
    keyboard::{Key, KeyboardInput},
    ButtonState,
};
use bevy::input_focus::{
    tab_navigation::TabGroup, AcquireFocus, FocusCause, FocusLost, FocusedInput, InputFocus,
};
use bevy::prelude::*;
use bevy::text::{EditableText, FontSource, LineBreak, LineHeight, TextEdit};
use bevy_renet::RenetClient;
use chrono::{DateTime, FixedOffset, Local, TimeZone};
use shared::messages::{ChatMessageRequest, ClientToServerMessage, FullChatMessage};

use super::hotbar::ITEM_NAME_BAND;
use super::inventory::HOTBAR_TOP;
use super::UIMode;
use crate::input::data::GameAction;
use crate::network::CachedChatConversation;
use crate::network::SendGameMessageExtension;
use crate::state::KeyMap;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::UiDialog;
use crate::ui::theme::{BEVEL_WIDTH, FONT_S, MUTED, PANEL, SPACE_L, SPACE_S, SPACE_XS, TEXT};
use crate::ui::widgets::list::{spawn_scroll_list, WheelStep};
use crate::ui::widgets::text_field::{set_text, spawn_text_field, Cancel, Submit, TextField};

/// The messages the log keeps; the oldest go first.
pub const CHAT_MAX_MESSAGES: usize = 100;
pub const CHAT_WIDTH: f32 = 420.0;
/// The lines the open log shows, and the most recent lines the closed chat shows.
pub const CHAT_LOG_LINES: usize = 10;
/// How long a line stays whole over the game once received, before it fades.
pub const CHAT_FADE_AFTER: Duration = Duration::from_secs(7);
/// How long a line takes to fade out over the game.
pub const CHAT_FADE: Duration = Duration::from_secs(1);

/// The height of a line of the log: whole pixels at every `UiScale`.
pub const CHAT_LINE_HEIGHT: f32 = FONT_S + SPACE_XS;
/// The opacity of the well of the open log: the game stays readable through it.
const LOG_ALPHA: f32 = 0.75;
/// The opacity of the strip under a line shown over the game.
const STRIP_ALPHA: f32 = 0.6;

/// Every character the pixel font of `FONT_PATH` draws, read from its character map: some
/// accented letters, such as `ç` and `à`, but no `é` or `è`.
const PIXEL_FONT_CHARS: &str = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_\
abcdefghijklmnopqrstuvwxyz{|}~¡¢£¤§«¬\u{ad}®±¶·¸»¿ÄÅÆÇÑØÜàáâåæçêëíîñòóôö÷øùúûüÿƒΩ—•∑√∞≈⌂☺☻☼♀♂♠♣♥♦";

#[derive(Component)]
pub struct ChatRoot;

/// The `ScrollArea` of the chat, holding its `ChatLines`.
#[derive(Component)]
pub struct ChatLog;

/// The column of the lines in the log, oldest first, at the bottom of the log while they are
/// few.
#[derive(Component)]
pub struct ChatLines;

/// A line of the log: the time of its message, then its author and its text.
#[derive(Component)]
pub struct ChatLine {
    /// When the client received the message, which it fades from over the game.
    pub received: Instant,
}

#[derive(Component)]
pub struct ChatInput;

/// The messages sent in this game, oldest first, and where Up/Down are among them. On the chat
/// root.
#[derive(Component, Default)]
pub struct SentMessages {
    sent: Vec<String>,
    /// The message the field shows, while Up/Down walk them.
    recalled: Option<usize>,
    /// The text of the field before the first Up, given back past the newest message.
    draft: String,
}

impl SentMessages {
    /// The messages kept; the oldest go first.
    pub const MAX: usize = 50;

    /// Keeps `message`, unless it repeats the last one, and ends the recall.
    pub fn record(&mut self, message: &str) {
        if self.sent.last().map(String::as_str) != Some(message) {
            self.sent.push(message.to_string());
            let dropped = self.sent.len().saturating_sub(Self::MAX);
            self.sent.drain(..dropped);
        }
        self.end_recall();
    }

    /// The message before the one recalled, or the newest one, keeping `current` as the draft
    /// then; the oldest stays. `None` when nothing was sent.
    pub fn older(&mut self, current: &str) -> Option<&str> {
        let index = match self.recalled {
            Some(index) => index.saturating_sub(1),
            None => {
                let newest = self.sent.len().checked_sub(1)?;
                self.draft = current.to_string();
                newest
            }
        };
        self.recalled = Some(index);
        Some(&self.sent[index])
    }

    /// The message after the one recalled, or past the newest the draft, which ends the recall.
    /// `None` when nothing is recalled.
    pub fn newer(&mut self) -> Option<&str> {
        let index = self.recalled? + 1;
        if index < self.sent.len() {
            self.recalled = Some(index);
            Some(&self.sent[index])
        } else {
            self.recalled = None;
            Some(&self.draft)
        }
    }

    /// Forgets the draft: the next Up starts again from the newest message.
    pub fn end_recall(&mut self) {
        self.recalled = None;
        self.draft.clear();
    }
}

pub fn setup_chat(mut commands: Commands, asset_server: Res<AssetServer>) {
    let root = commands
        .spawn((
            Name::new("ChatRoot"),
            DespawnOnExit(crate::state::GameState::Game),
            ChatRoot,
            SentMessages::default(),
            UiDialog,
            // Tab stays in the field
            TabGroup::modal(),
            Node {
                position_type: PositionType::Absolute,
                left: px(SPACE_L),
                // above the hotbar and the name of its item, at every size
                bottom: px(HOTBAR_TOP + ITEM_NAME_BAND + SPACE_S),
                width: px(CHAT_WIDTH),
                flex_direction: FlexDirection::Column,
                row_gap: px(SPACE_XS),
                ..default()
            },
            Visibility::Hidden,
        ))
        .observe(keep_the_focus_in_the_chat)
        .id();

    // a well of `CHAT_LOG_LINES` lines, translucent over the game, with its scrollbar
    // three lines a notch of the wheel
    let log = spawn_scroll_list(&mut commands, WheelStep::Px(3.0 * CHAT_LINE_HEIGHT));
    commands
        .entity(log.well)
        .insert(BackgroundColor(PANEL.with_alpha(LOG_ALPHA)));
    commands.entity(log.list).insert(ChatLog);
    // the lines sit at the bottom of the log while they are few
    commands.spawn((
        ChatLines,
        Node {
            flex_direction: FlexDirection::Column,
            justify_content: JustifyContent::End,
            min_height: percent(100),
            // grows past the log as the lines come, which the log scrolls
            flex_shrink: 0.0,
            ..default()
        },
        ChildOf(log.list),
    ));
    commands
        .spawn((
            Node {
                height: px(CHAT_LOG_LINES as f32 * CHAT_LINE_HEIGHT + 2.0 * BEVEL_WIDTH),
                ..default()
            },
            ChildOf(root),
        ))
        .add_child(log.frame);

    let field = spawn_text_field(
        &mut commands,
        &TextField {
            label: "Chat",
            placeholder: "Send a message...",
            max_characters: Some(256),
            font: asset_server.load(FONT_PATH),
            font_size: FONT_S,
            reserve_error: false,
        },
    );
    commands.entity(root).add_child(field.root);
    commands
        .entity(field.input)
        .insert(ChatInput)
        .observe(send_chat_message)
        .observe(page_the_log)
        .observe(recall_on_arrows)
        .observe(close_chat_on::<Submit>)
        .observe(close_chat_on::<Cancel>)
        .observe(close_chat_on::<FocusLost>);
}

/// `HH:MM` of a UTC timestamp in milliseconds, at `offset`.
pub fn chat_time(timestamp_ms: u64, offset: FixedOffset) -> String {
    utc(timestamp_ms)
        .with_timezone(&offset)
        .format("%H:%M")
        .to_string()
}

/// The UTC time of a timestamp in milliseconds; the epoch for one out of range.
fn utc(timestamp_ms: u64) -> DateTime<chrono::Utc> {
    i64::try_from(timestamp_ms)
        .ok()
        .and_then(DateTime::from_timestamp_millis)
        .unwrap_or_default()
}

/// The offset of the local time zone at a UTC timestamp in milliseconds.
fn local_offset(timestamp_ms: u64) -> FixedOffset {
    Local.offset_from_utc_datetime(&utc(timestamp_ms).naive_utc())
}

/// The opacity of a line `age` old over the game: 1 for `CHAT_FADE_AFTER`, then down to 0 over
/// `CHAT_FADE`.
pub fn line_alpha(age: Duration) -> f32 {
    let left = (CHAT_FADE_AFTER + CHAT_FADE).saturating_sub(age);
    (left.as_secs_f32() / CHAT_FADE.as_secs_f32()).min(1.0)
}

/// The font that draws all of `text`: the pixel font, or Fira Mono when the pixel font lacks one
/// of its characters. A message is drawn in a single font, without a missing letter.
fn font_for(text: &str) -> &'static str {
    if text.chars().all(|c| PIXEL_FONT_CHARS.contains(c)) {
        FONT_PATH
    } else {
        MONO_FONT_PATH
    }
}

/// Scrolls `log` to its end, once laid out: the layout clamps a position past the end.
fn scroll_to_bottom(log: &mut ScrollPosition) {
    log.y = f32::MAX;
}

/// Opens the chat on its key when no dialog is open, with the focus on its field and the log at
/// its last line. On release, so that the key that opens the chat is not typed into it, and only
/// the release of a key pressed in the game: a chat key still held as a message is sent, typed
/// into it, does not open the chat again.
pub fn toggle_chat_system(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    key_map: Res<KeyMap>,
    mut armed: bevy::ecs::system::Local<Option<KeyCode>>,
    ui_mode: Res<UIMode>,
    mut chat_root: Query<&mut Visibility, With<ChatRoot>>,
    chat_input: Query<Entity, With<ChatInput>>,
    mut logs: Query<&mut ScrollPosition, With<ChatLog>>,
    mut focus: ResMut<InputFocus>,
) {
    if *ui_mode != UIMode::Closed {
        *armed = None;
        return;
    }
    let keys = key_map.map.get(&GameAction::OpenChat);
    if let Some(&pressed) = keys
        .into_iter()
        .flatten()
        .find(|&&key| keyboard_input.just_pressed(key))
    {
        *armed = Some(pressed);
    }
    let Some(key) = *armed else {
        return;
    };
    if !keyboard_input.just_released(key) {
        return;
    }
    *armed = None;
    let (Ok(mut visibility), Ok(input)) = (chat_root.single_mut(), chat_input.single()) else {
        return;
    };
    open_chat(&mut visibility, input, &mut focus, &mut logs);
}

/// Opens the chat on the character `/` in the game, with `/` typed in its field and the caret
/// after it. The character, not a key, so that it works on every layout: a press giving `/`
/// arms its physical key, whose release opens the chat whatever it gives then (AZERTY's
/// Shift+`:` released Shift first gives `:`). On release, as the chat key, so that neither the
/// key nor its repeat is typed in the field; a release whose press was not in the game, such as
/// the end of a `/` typed in the chat, opens nothing.
pub fn open_chat_on_slash(
    mut keys: MessageReader<KeyboardInput>,
    mut armed: bevy::ecs::system::Local<Option<KeyCode>>,
    ui_mode: Res<UIMode>,
    mut chat_root: Query<&mut Visibility, With<ChatRoot>>,
    mut chat_input: Query<(Entity, &mut EditableText), With<ChatInput>>,
    mut logs: Query<&mut ScrollPosition, With<ChatLog>>,
    mut focus: ResMut<InputFocus>,
) {
    // read on every frame, so that a slash typed over a dialog never opens the chat later
    let in_game = *ui_mode == UIMode::Closed;
    if !in_game {
        *armed = None;
    }
    let mut open = false;
    for key in keys.read() {
        match key.state {
            ButtonState::Pressed
                if in_game && !key.repeat && key.logical_key == Key::Character("/".into()) =>
            {
                *armed = Some(key.key_code);
            }
            ButtonState::Released if *armed == Some(key.key_code) => {
                *armed = None;
                open = true;
            }
            _ => {}
        }
    }
    if !open {
        return;
    }
    let (Ok(mut visibility), Ok((input, mut text))) =
        (chat_root.single_mut(), chat_input.single_mut())
    else {
        return;
    };
    open_chat(&mut visibility, input, &mut focus, &mut logs);
    set_text(&mut text, "/");
}

/// Shows the chat, with the focus on its field `input` and the log at its last line.
fn open_chat(
    visibility: &mut Visibility,
    input: Entity,
    focus: &mut InputFocus,
    logs: &mut Query<&mut ScrollPosition, With<ChatLog>>,
) {
    *visibility = Visibility::Visible;
    focus.set(input, FocusCause::Navigated);
    for mut log in logs {
        scroll_to_bottom(&mut log);
    }
}

/// Shows the recent lines over the game while the chat is closed: the last `CHAT_LOG_LINES`
/// lines younger than `CHAT_FADE_AFTER + CHAT_FADE`, each on its strip, fading with
/// `line_alpha`. The open chat shows its lines without strips; another dialog open hides them,
/// from the frame it shows, as the HUD.
pub fn fade_chat_messages_system(
    ui_mode: Res<UIMode>,
    chat_root: Query<&Visibility, (With<ChatRoot>, Without<ChatLine>)>,
    dialogs: Query<&Visibility, (With<UiDialog>, Without<ChatRoot>, Without<ChatLine>)>,
    logs: Query<&Children, With<ChatLines>>,
    mut lines: Query<(&ChatLine, &Children, &mut Visibility, &mut BackgroundColor)>,
    mut colors: Query<&mut TextColor>,
) {
    let open = chat_root
        .iter()
        .any(|visibility| *visibility == Visibility::Visible);
    let dialog_open = *ui_mode == UIMode::Opened
        || dialogs
            .iter()
            .any(|visibility| *visibility == Visibility::Visible);
    let now = Instant::now();
    for log in &logs {
        let recent = log.len().saturating_sub(CHAT_LOG_LINES);
        for (index, entity) in log.iter().enumerate() {
            let Ok((line, texts, mut visibility, mut strip)) = lines.get_mut(entity) else {
                continue;
            };
            let age = now.saturating_duration_since(line.received);
            let over_the_game =
                !open && !dialog_open && index >= recent && age < CHAT_FADE_AFTER + CHAT_FADE;
            let (shown, alpha, ground) = if over_the_game {
                let alpha = line_alpha(age);
                let ground = PANEL.with_alpha(STRIP_ALPHA * alpha);
                (Visibility::Visible, alpha, ground)
            } else {
                (Visibility::Inherited, 1.0, Color::NONE)
            };
            visibility.set_if_neq(shown);
            strip.set_if_neq(BackgroundColor(ground));
            for text in texts.iter() {
                if let Ok(mut color) = colors.get_mut(text) {
                    let faded = TextColor(color.0.with_alpha(alpha));
                    color.set_if_neq(faded);
                }
            }
        }
    }
}

/// Adds the messages received since the last frame to the log, and drops the oldest lines beyond
/// `CHAT_MAX_MESSAGES`. A log scrolled to its end stays there; a log scrolled up keeps its lines
/// in place.
pub fn display_chat_messages_system(
    mut commands: Commands,
    mut conversation: ResMut<CachedChatConversation>,
    asset_server: Res<AssetServer>,
    mut logs: Query<(&mut ScrollPosition, &ComputedNode), With<ChatLog>>,
    columns: Query<(Entity, Option<&Children>), With<ChatLines>>,
    laid_out: Query<&ComputedNode, With<ChatLine>>,
) {
    let (Ok((mut position, node)), Ok((column, lines))) = (logs.single_mut(), columns.single())
    else {
        return;
    };
    if conversation.unread.is_empty() {
        return;
    }
    let incoming: Vec<FullChatMessage> = std::mem::take(&mut conversation.unread);
    let lines = lines.map(|lines| &lines[..]).unwrap_or_default();
    // the messages that would be dropped at once are never spawned
    let skipped = incoming.len().saturating_sub(CHAT_MAX_MESSAGES);
    let dropped = (lines.len() + incoming.len() - skipped).saturating_sub(CHAT_MAX_MESSAGES);
    for &oldest in &lines[..dropped] {
        commands.entity(oldest).despawn();
    }
    // the height the dropped lines took, as laid out on the last frame
    let gone: f32 = lines[..dropped]
        .iter()
        .filter_map(|&line| laid_out.get(line).ok())
        .map(|line| line.size().y * line.inverse_scale_factor)
        .sum();

    let now = Instant::now();
    for message in &incoming[skipped..] {
        let said = format!("<{}> {}", message.author, message.content);
        let time = chat_time(message.timestamp, local_offset(message.timestamp));
        commands.spawn((
            ChatLine { received: now },
            Node {
                column_gap: px(SPACE_S),
                padding: UiRect::horizontal(px(SPACE_XS)),
                ..default()
            },
            // the strip under the line over the game, drawn by `fade_chat_messages_system`
            BackgroundColor(Color::NONE),
            // shown over the game, a line takes no click; in the open chat, the log under it
            // takes the wheel and the clicks
            Pickable::IGNORE,
            ChildOf(column),
            children![
                (
                    line_text(time, asset_server.load(FONT_PATH), MUTED),
                    Node {
                        flex_shrink: 0.0,
                        ..default()
                    },
                ),
                (
                    line_text(said.clone(), asset_server.load(font_for(&said)), TEXT),
                    TextLayout {
                        linebreak: LineBreak::WordOrCharacter,
                        ..default()
                    },
                ),
            ],
        ));
    }

    // physical pixels, as the layout left them on the last frame
    let end = (node.content_size().y - node.size().y + node.scrollbar_size.y).max(0.0);
    if node.scroll_position.y >= end - 1.0 {
        scroll_to_bottom(&mut position);
    } else {
        // a log scrolled up keeps its lines in place as the oldest go
        position.y = (position.y - gone).max(0.0);
    }
}

/// A text of a line of the log.
fn line_text(text: String, font: Handle<Font>, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont {
            font: FontSource::Handle(font),
            font_size: px(FONT_S).into(),
            ..default()
        },
        LineHeight::Px(CHAT_LINE_HEIGHT),
        TextColor(color),
        Pickable::IGNORE,
    )
}

/// Writes back where the log is scrolled, once laid out: the layout clamps a position past the
/// end, which `scroll_to_bottom` relies on, and the drag of the scrollbar and the pages of
/// `page_the_log` start from this value.
pub fn settle_chat_scroll(mut logs: Query<(&mut ScrollPosition, &ComputedNode), With<ChatLog>>) {
    for (mut position, node) in &mut logs {
        let shown = node.scroll_position.y * node.inverse_scale_factor;
        if position.y != shown {
            position.y = shown;
        }
    }
}

/// Draws the text being written in Fira Mono while the pixel font lacks one of its characters,
/// as the lines of the log. Bevy reads the font of a field before it applies the edits of the
/// frame, so the characters about to be inserted count too: the frame that draws a new letter
/// already has its font. Edits that remove one are seen on the next frame.
pub fn match_the_field_font(
    mut fields: Query<(&EditableText, &mut TextFont), (With<ChatInput>, Changed<EditableText>)>,
    asset_server: Res<AssetServer>,
) {
    for (text, mut font) in &mut fields {
        let mut written = text.value().to_string();
        for edit in &text.pending_edits {
            if let TextEdit::Insert(inserted) = edit {
                written.push_str(inserted);
            }
        }
        let wanted = FontSource::Handle(asset_server.load(font_for(&written)));
        if font.font != wanted {
            font.font = wanted;
        }
    }
}

/// PageUp and PageDown in the field scroll the log by its height less a line, which stays in
/// view.
fn page_the_log(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut logs: Query<&mut ScrollPosition, With<ChatLog>>,
) {
    let key = &event.input;
    if key.state != ButtonState::Pressed {
        return;
    }
    let page = (CHAT_LOG_LINES - 1) as f32 * CHAT_LINE_HEIGHT;
    let step = match key.key_code {
        KeyCode::PageUp => -page,
        KeyCode::PageDown => page,
        _ => return,
    };
    event.propagate(false);
    for mut log in &mut logs {
        log.y = (log.y + step).max(0.0);
    }
}

/// Up and Down in the field put the sent messages in it, older and newer, with the caret at the
/// end; past the newest, the text typed before. Bevy moves the caret on these keys first: the
/// recalled text replaces the field after that move.
fn recall_on_arrows(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut texts: Query<&mut EditableText>,
    mut sent: Query<&mut SentMessages>,
) {
    let key = &event.input;
    if key.state != ButtonState::Pressed {
        return;
    }
    let field = event.focused_entity;
    let (Ok(mut text), Ok(mut sent)) = (texts.get_mut(field), sent.single_mut()) else {
        return;
    };
    // while the IME composes, the arrows belong to it
    if text.is_composing() {
        return;
    }
    let recalled = match key.key_code {
        KeyCode::ArrowUp => sent.older(&text.value().to_string()),
        KeyCode::ArrowDown => sent.newer(),
        _ => return,
    };
    event.propagate(false);
    if let Some(recalled) = recalled {
        // replacing the whole text leaves the caret at its end
        set_text(&mut text, recalled);
    }
}

/// A press in the chat, on its log or its scrollbar, keeps the focus in the field, and with it
/// the chat open. A press anywhere else clears the focus, which closes the chat.
fn keep_the_focus_in_the_chat(
    mut acquire: On<AcquireFocus>,
    input: Query<Entity, With<ChatInput>>,
    mut focus: ResMut<InputFocus>,
) {
    acquire.propagate(false);
    if let Ok(input) = input.single() {
        if focus.get() != Some(input) {
            focus.set(input, FocusCause::Pressed);
        }
    }
}

/// Sends the message submitted in the chat field, unless it is blank, and keeps it for Up.
fn send_chat_message(
    submit: On<Submit>,
    mut client: ResMut<RenetClient>,
    mut sent: Query<&mut SentMessages>,
) {
    if !submit.value.trim().is_empty() {
        client.send_game_message(ClientToServerMessage::ChatMessage(ChatMessageRequest {
            content: submit.value.clone(),
        }));
        for mut sent in &mut sent {
            sent.record(&submit.value);
        }
    }
}

/// Closes the chat when its field is submitted, cancelled or loses the focus: the chat is
/// hidden, its field emptied and unfocused, and its log back at its last line. Nothing typed is
/// kept for the next time, and the recall ends.
fn close_chat_on<E: EntityEvent>(
    event: On<E>,
    mut chat_root: Query<(&mut Visibility, &mut SentMessages), With<ChatRoot>>,
    mut texts: Query<&mut EditableText>,
    mut logs: Query<&mut ScrollPosition, With<ChatLog>>,
    mut focus: ResMut<InputFocus>,
) {
    let input = event.event_target();
    if let Ok((mut visibility, mut sent)) = chat_root.single_mut() {
        *visibility = Visibility::Hidden;
        sent.end_recall();
    }
    if let Ok(mut text) = texts.get_mut(input) {
        text.clear();
    }
    for mut log in &mut logs {
        scroll_to_bottom(&mut log);
    }
    if focus.get() == Some(input) {
        focus.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
    use chrono::FixedOffset;
    use shared::messages::FullChatMessage;

    fn message(author: &str, content: &str) -> FullChatMessage {
        FullChatMessage {
            content: content.into(),
            author: author.into(),
            timestamp: 0,
        }
    }

    /// An app that shows the messages received in a log, under a closed chat.
    fn chat_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .init_resource::<CachedChatConversation>()
            .insert_resource(UIMode::Closed)
            .add_systems(
                Update,
                (display_chat_messages_system, fade_chat_messages_system).chain(),
            );
        let lines = app.world_mut().spawn((ChatLines, Node::default())).id();
        let log = app.world_mut().spawn((ChatLog, Node::default())).id();
        app.world_mut().entity_mut(log).add_child(lines);
        app.world_mut()
            .spawn((ChatRoot, UiDialog, Visibility::Hidden))
            .add_child(log);
        (app, lines)
    }

    fn receive(app: &mut App, messages: impl IntoIterator<Item = FullChatMessage>) {
        app.world_mut()
            .resource_mut::<CachedChatConversation>()
            .unread
            .extend(messages);
        app.update();
    }

    /// The lines of the log, in its column `log`, oldest first.
    fn lines(app: &App, log: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(log)
            .map(|lines| lines.to_vec())
            .unwrap_or_default()
    }

    /// The text of `line` after its time.
    fn said(app: &App, line: Entity) -> String {
        let texts = app.world().get::<Children>(line).unwrap();
        app.world().get::<Text>(texts[1]).unwrap().0.clone()
    }

    fn east(seconds: i32) -> FixedOffset {
        FixedOffset::east_opt(seconds).unwrap()
    }

    #[test]
    fn the_chat_keeps_the_last_hundred_messages() {
        let (mut app, log) = chat_app();
        receive(&mut app, (0..100).map(|i| message("steve", &i.to_string())));
        receive(
            &mut app,
            (100..102).map(|i| message("steve", &i.to_string())),
        );
        let shown = lines(&app, log);
        assert_eq!(shown.len(), CHAT_MAX_MESSAGES);
        assert_eq!(said(&app, shown[0]), "<steve> 2");
        assert_eq!(said(&app, shown[CHAT_MAX_MESSAGES - 1]), "<steve> 101");
        assert!(app
            .world()
            .resource::<CachedChatConversation>()
            .unread
            .is_empty());
    }

    #[test]
    fn up_and_down_walk_the_sent_messages() {
        let mut sent = SentMessages::default();
        for message in ["a", "b", "b"] {
            sent.record(message);
        }
        assert_eq!(sent.older("dr"), Some("b"));
        assert_eq!(sent.older("b"), Some("a"));
        // the oldest stays
        assert_eq!(sent.older("a"), Some("a"));
        assert_eq!(sent.newer(), Some("b"));
        // past the newest, the text typed before recalling
        assert_eq!(sent.newer(), Some("dr"));
        assert_eq!(sent.newer(), None);

        let mut sent = SentMessages::default();
        for index in 1..=60 {
            sent.record(&index.to_string());
        }
        let mut oldest = None;
        for _ in 0..60 {
            oldest = sent.older("").map(str::to_string);
        }
        assert_eq!(oldest.as_deref(), Some("11"));
    }

    #[test]
    fn a_line_shows_the_local_time_of_its_message() {
        assert_eq!(chat_time(0, east(3600)), "01:00");
        let afternoon = ((13 * 60 + 7) * 60 + 59) * 1000 + 999;
        assert_eq!(chat_time(afternoon, east(0)), "13:07");
        let late = (23 * 60 + 30) * 60 * 1000;
        assert_eq!(chat_time(late, east(3600)), "00:30");
    }

    #[test]
    fn a_line_fades_after_seven_seconds() {
        let alpha = |millis| line_alpha(Duration::from_millis(millis));
        assert_eq!(alpha(0), 1.0);
        assert_eq!(alpha(6_900), 1.0);
        assert_eq!(alpha(7_500), 0.5);
        assert_eq!(alpha(8_000), 0.0);
        assert_eq!(alpha(60_000), 0.0);
    }

    #[test]
    fn a_closed_chat_shows_only_its_recent_lines() {
        let (mut app, log) = chat_app();
        receive(&mut app, (0..12).map(|i| message("steve", &i.to_string())));
        let visibility = |app: &App, line: Entity| *app.world().get::<Visibility>(line).unwrap();
        let shown = lines(&app, log);
        let visible: Vec<bool> = shown
            .iter()
            .map(|&line| visibility(&app, line) == Visibility::Visible)
            .collect();
        assert_eq!(visible, [[false; 2].as_slice(), &[true; 10]].concat());

        // a line older than its fade is left to the closed chat
        let old = shown[5];
        app.world_mut().get_mut::<ChatLine>(old).unwrap().received =
            Instant::now() - Duration::from_secs(9);
        app.update();
        assert_eq!(visibility(&app, old), Visibility::Inherited);
        assert_eq!(visibility(&app, shown[11]), Visibility::Visible);

        // another dialog hides them all
        app.insert_resource(UIMode::Opened);
        app.update();
        assert!(shown
            .iter()
            .all(|&line| visibility(&app, line) == Visibility::Inherited));
    }

    #[test]
    fn a_message_the_pixel_font_cannot_draw_is_drawn_in_fira_mono() {
        let (mut app, log) = chat_app();
        receive(
            &mut app,
            [
                message("steve", "hello"),
                // the pixel font has a few accented letters, `ç` among them, but no `é`
                message("steve", "ça va"),
                message("steve", "j'ai été"),
                message("Zoé", "hello"),
            ],
        );
        let assets = app.world().resource::<AssetServer>().clone();
        let fonts: Vec<String> = lines(&app, log)
            .into_iter()
            .map(|line| {
                let texts = app.world().get::<Children>(line).unwrap();
                let FontSource::Handle(font) = &app.world().get::<TextFont>(texts[1]).unwrap().font
                else {
                    panic!("a line is drawn in a font of the game");
                };
                assets.get_path(font).unwrap().to_string()
            })
            .collect();
        let (pixel, mono) = (
            assets.load::<Font>(FONT_PATH),
            assets.load::<Font>(MONO_FONT_PATH),
        );
        let (pixel, mono) = (
            assets.get_path(&pixel).unwrap().to_string(),
            assets.get_path(&mono).unwrap().to_string(),
        );
        assert_eq!(fonts, [pixel.clone(), pixel, mono.clone(), mono]);
    }

    #[test]
    fn a_letter_being_typed_switches_the_field_font_before_it_is_drawn() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .add_systems(Update, match_the_field_font);
        let mut text = EditableText::new("ca");
        // typed this frame, applied by Bevy after the style of the field is read
        text.queue_edit(TextEdit::Insert("é".into()));
        let field = app.world_mut().spawn((ChatInput, text)).id();
        app.update();
        let assets = app.world().resource::<AssetServer>().clone();
        let mono = assets.load::<Font>(MONO_FONT_PATH);
        assert_eq!(
            app.world().get::<TextFont>(field).unwrap().font,
            FontSource::Handle(mono)
        );
    }

    #[test]
    fn a_recalled_message_has_its_font_on_its_first_frame() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Font>()
            .add_systems(Update, match_the_field_font);
        let mut text = EditableText::new("ca");
        // what Up puts in the field, applied by Bevy after the style of the field is read
        set_text(&mut text, "été");
        let field = app.world_mut().spawn((ChatInput, text)).id();
        app.update();
        let assets = app.world().resource::<AssetServer>().clone();
        let mono = assets.load::<Font>(MONO_FONT_PATH);
        assert_eq!(
            app.world().get::<TextFont>(field).unwrap().font,
            FontSource::Handle(mono)
        );
    }

    /// The characters a font maps to a glyph, read from the Windows Unicode BMP subtable
    /// (platform 3, encoding 1, format 4) of its `cmap` table.
    fn mapped_characters(font: &[u8]) -> std::collections::BTreeSet<char> {
        let u16_at = |at: usize| u16::from_be_bytes([font[at], font[at + 1]]);
        let u32_at = |at: usize| u32::from_be_bytes(font[at..at + 4].try_into().unwrap()) as usize;
        let tables = u16_at(4) as usize;
        let cmap = (0..tables)
            .map(|i| 12 + 16 * i)
            .find(|&record| &font[record..record + 4] == b"cmap")
            .map(|record| u32_at(record + 8))
            .expect("a cmap table");
        let subtables = u16_at(cmap + 2) as usize;
        let subtable = (0..subtables)
            .map(|i| cmap + 4 + 8 * i)
            .find(|&record| u16_at(record) == 3 && u16_at(record + 2) == 1)
            .map(|record| cmap + u32_at(record + 4))
            .expect("a Unicode BMP subtable");
        assert_eq!(u16_at(subtable), 4, "a subtable of format 4");
        let segments = u16_at(subtable + 6) as usize / 2;
        let ends = subtable + 14;
        let starts = ends + 2 * segments + 2;
        let deltas = starts + 2 * segments;
        let range_offsets = deltas + 2 * segments;
        let mut mapped = std::collections::BTreeSet::new();
        for segment in 0..segments {
            let (start, end) = (u16_at(starts + 2 * segment), u16_at(ends + 2 * segment));
            let delta = u16_at(deltas + 2 * segment);
            let range_offset = range_offsets + 2 * segment;
            for code in start..=end {
                if code == 0xFFFF {
                    continue;
                }
                let glyph = match u16_at(range_offset) as usize {
                    0 => code.wrapping_add(delta),
                    offset => match u16_at(range_offset + offset + 2 * (code - start) as usize) {
                        0 => 0,
                        glyph => glyph.wrapping_add(delta),
                    },
                };
                if glyph != 0 {
                    mapped.extend(char::from_u32(code.into()));
                }
            }
        }
        mapped
    }

    #[test]
    fn the_characters_of_the_pixel_font_are_those_of_its_file() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../data/fonts/RustCraftRegular-Bmg3.otf"
        );
        assert!(path.ends_with(FONT_PATH.trim_start_matches("./")));
        let font = std::fs::read(path).unwrap();
        let listed: std::collections::BTreeSet<char> = PIXEL_FONT_CHARS.chars().collect();
        assert_eq!(
            listed.len(),
            PIXEL_FONT_CHARS.chars().count(),
            "listed once each"
        );
        assert_eq!(listed, mapped_characters(&font));
    }
}
