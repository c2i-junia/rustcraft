//! The chat: sending, the history of the log with its times, scrolling it, where it sits over
//! the HUD, and the game keys it keeps quiet.

use accesskit::Role;
use bevy::{
    input::keyboard::KeyCode, pbr::wireframe::WireframeConfig, prelude::*, text::FontSource,
    ui::UiGlobalTransform, ui_widgets::ScrollbarThumb,
};
use shared::messages::FullChatMessage;
use shared::utils::unix_time_ms;
use shared::world::ItemId;

use super::game::{enter_world, enter_world_with, slot};
use super::harness::{Harness, Options, SCREEN, SERVER};
use super::locator::{is_inside, Locator as L};
use super::seed::GameFolder;
use crate::network::CachedChatConversation;
use crate::ui::assets::{FONT_PATH, MONO_FONT_PATH};
use crate::ui::hud::chat::{ChatInput, ChatLine, ChatLog, ChatRoot, CHAT_LINE_HEIGHT, CHAT_WIDTH};
use crate::ui::hud::chat_completion::{Ghost, SuggestionPanel};
use crate::ui::hud::debug::{DebugOptions, HudRoot};
use crate::ui::hud::fps_counter::FpsCounter;
use crate::ui::hud::hotbar::{Hotbar, HotbarItemName};
use crate::ui::hud::{InventoryRoot, UIMode};
use crate::ui::theme::{SPACE_L, SPACE_S};

impl Harness {
    /// Whether a visible line of the chat contains `part`. The lines stay on screen for a
    /// while after the chat is closed, while the chat itself is hidden.
    #[track_caller]
    fn chat_shows(&mut self, part: &str) -> bool {
        let lines = self.find(&L::text_containing(part));
        let world = self.world_mut();
        let log = world
            .query_filtered::<Entity, With<ChatLog>>()
            .single(world)
            .unwrap();
        lines.into_iter().any(|line| is_inside(world, line, log))
    }

    /// Whether the text `text` of a line is in view in the open log, which clips the lines
    /// scrolled out of it.
    #[track_caller]
    fn in_log_view(&mut self, text: &str) -> bool {
        let line = L::text(text).within(L::marker::<ChatLog>());
        if !self.is_visible(&line) {
            return false;
        }
        let shown = self.rect_of(&line);
        let log = self.rect_of(&L::marker::<ChatLog>());
        log.contains(shown.min + 0.5) && log.contains(shown.max - 0.5)
    }

    /// The path of the font the chat field draws its text in.
    #[track_caller]
    fn field_font(&mut self) -> String {
        let world = self.world_mut();
        let font = world
            .query_filtered::<&TextFont, With<ChatInput>>()
            .single(world)
            .unwrap();
        let FontSource::Handle(font) = &font.font else {
            panic!("the chat field is drawn in a font of the game");
        };
        let path = world.resource::<AssetServer>().get_path(font).unwrap();
        path.to_string()
    }

    /// Has the server send the messages `line <index>` of `Bot`, one for each index.
    pub(super) fn receive_chat(&mut self, indices: std::ops::Range<usize>) {
        let now = unix_time_ms();
        self.world_mut()
            .resource_mut::<CachedChatConversation>()
            .unread
            .extend(indices.map(|index| FullChatMessage {
                content: format!("line {index}"),
                author: "Bot".into(),
                timestamp: now,
            }));
        self.frames(1);
    }

    /// Asserts that the bottom of the line reading `text` is at the bottom of the log, shown or
    /// not.
    #[track_caller]
    fn assert_at_log_bottom(&mut self, text: &str) {
        let line = self.rect_of(&L::text(text));
        let world = self.world_mut();
        let log = world
            .query_filtered::<(&UiGlobalTransform, &ComputedNode), With<ChatLog>>()
            .single(world)
            .unwrap();
        let log = Rect::from_center_size(log.0.translation, log.1.size());
        assert!(
            (log.max.y - line.max.y).abs() <= 1.0,
            "{line:?} at the bottom of {log:?}"
        );
    }

    /// Opens the chat, sends `message` and waits for the server to send it back.
    #[track_caller]
    fn send_chat(&mut self, message: &str) {
        self.press(KeyCode::KeyT);
        self.type_text(message);
        self.press(KeyCode::Enter);
        self.wait_for(&L::text_containing(message), SERVER);
    }
}

/// Whether `text` reads as a time, `HH:MM`.
fn is_time(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 5 && bytes[2] == b':' && [0, 1, 3, 4].iter().all(|&i| bytes[i].is_ascii_digit())
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_chat_sends_a_message_and_escape_closes_it() {
    let mut h = enter_world(GameFolder::new().with_world("Chat"), "Chat");
    h.press(KeyCode::KeyT);
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    h.type_text("bonjour");
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("bonjour"), SERVER);
    assert!(h.chat_shows("bonjour"));

    // A message dropped with Escape is not sent
    h.press(KeyCode::KeyT);
    h.type_text("brouillon");
    assert_eq!(h.value_of::<ChatInput>(), "brouillon");
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    // The server answers in order: once the next message is shown, the dropped one would be too
    h.press(KeyCode::KeyT);
    h.type_text("ensuite");
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("ensuite"), SERVER);
    assert!(h.chat_shows("ensuite"));
    assert!(!h.is_visible(&L::text_containing("brouillon")));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn up_and_down_recall_sent_messages_and_a_slash_opens_the_chat() {
    let mut h = enter_world(GameFolder::new().with_world("Recall"), "Recall");
    h.send_chat("first");
    h.send_chat("second");

    // Up walks back through the sent messages, Down forward, then back to the draft
    h.press(KeyCode::KeyT);
    h.type_text("dra");
    h.press(KeyCode::ArrowUp);
    assert_eq!(h.value_of::<ChatInput>(), "second");
    h.press(KeyCode::ArrowUp);
    h.press(KeyCode::ArrowUp);
    assert_eq!(h.value_of::<ChatInput>(), "first");
    h.press(KeyCode::ArrowDown);
    assert_eq!(h.value_of::<ChatInput>(), "second");
    h.press(KeyCode::ArrowDown);
    assert_eq!(h.value_of::<ChatInput>(), "dra");
    // Escape keeps no draft, and the recall starts again from the newest
    h.press(KeyCode::Escape);
    h.press(KeyCode::KeyT);
    assert_eq!(h.value_of::<ChatInput>(), "");
    h.press(KeyCode::ArrowUp);
    assert_eq!(h.value_of::<ChatInput>(), "second");
    // the caret is at the end of the recalled message
    h.type_text("!");
    assert_eq!(h.value_of::<ChatInput>(), "second!");
    h.press(KeyCode::Escape);

    // the arrows in the game open nothing
    h.press(KeyCode::ArrowUp);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));

    // a slash opens the chat with the slash typed, and is sent as typed
    h.type_text("/");
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    assert_eq!(h.value_of::<ChatInput>(), "/");
    assert_eq!(h.focus_label().as_deref(), Some("Chat"));
    h.type_text("help");
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("/help"), SERVER);
    assert!(h.chat_shows("/help"));

    // the character opens it, from whichever key of the layout
    h.type_on(KeyCode::Digit7, '/');
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    assert_eq!(h.value_of::<ChatInput>(), "/");
    // the first Escape puts the suggestions of the commands away, the second closes the chat
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    // Shift let go first: AZERTY releases its Shift+`:` as `:`
    h.key_down_on(KeyCode::Period, '/');
    h.key_up_on(KeyCode::Period, ':');
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    assert_eq!(h.value_of::<ChatInput>(), "/");
    h.press(KeyCode::Escape);
    h.press(KeyCode::Escape);
    h.type_on(KeyCode::Slash, '!');
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));

    // a chat key pressed while typing does not open the chat again once released after sending
    h.press(KeyCode::KeyT);
    h.type_text("got it");
    h.key_down(KeyCode::KeyT);
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("got it"), SERVER);
    h.key_up(KeyCode::KeyT);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    h.press(KeyCode::KeyT);
    h.type_text("a");
    h.key_down_on(KeyCode::Period, '/');
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("a/"), SERVER);
    h.key_up_on(KeyCode::Period, '/');
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));

    // only in the game: not over the inventory
    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
    h.type_text("/");
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    h.press(KeyCode::KeyE);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_chat_keeps_its_history_with_times_and_scrolls() {
    let mut h = enter_world(GameFolder::new().with_world("History"), "History");
    // a single line sits at the bottom of the log, over the game and in the open chat
    h.send_chat("one");
    h.assert_at_log_bottom("<Tester> one");
    h.press(KeyCode::KeyT);
    h.frames(2);
    h.assert_at_log_bottom("<Tester> one");
    h.press(KeyCode::Escape);
    for message in ["two", "three"] {
        h.send_chat(message);
    }
    h.press(KeyCode::KeyT);
    for message in ["one", "two", "three"] {
        assert!(h.in_log_view(&format!("<Tester> {message}")), "{message}");
    }
    // each line starts with the time of its message
    let world = h.world_mut();
    let times: Vec<String> = world
        .query_filtered::<&Children, With<ChatLine>>()
        .iter(world)
        .map(|texts| world.get::<Text>(texts[0]).unwrap().0.clone())
        .collect();
    assert_eq!(times.len(), 3);
    assert!(times.iter().all(|time| is_time(time)), "{times:?}");
    h.press(KeyCode::Escape);
    // a click on a line shown over the game leaves the chat closed
    assert!(h.chat_shows("three"));
    h.click(&L::text("<Tester> three"));
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    assert!(*h.world().resource::<UIMode>() == UIMode::Closed);

    // more messages from the server than the log shows at once
    h.receive_chat(0..30);
    h.press(KeyCode::KeyT);
    h.frames(2);
    assert!(h.in_log_view("<Bot> line 29"));
    assert!(!h.in_log_view("<Tester> one"));

    // the wheel over the log scrolls it by three lines a notch
    h.hover(&L::marker::<ChatLog>());
    let before = h.center_of(&L::text("<Bot> line 29"));
    h.scroll(1.);
    h.frames(2);
    let scrolled = h.center_of(&L::text("<Bot> line 29"));
    assert_eq!(
        scrolled.y - before.y,
        3.0 * CHAT_LINE_HEIGHT,
        "{before} then {scrolled}"
    );
    // the field keeps the focus, and PageUp and PageDown scroll by the log less a line
    h.press(KeyCode::PageUp);
    h.frames(2);
    assert!(h.in_log_view("<Bot> line 10"));
    assert!(!h.in_log_view("<Bot> line 29"));
    // a message arriving leaves a log scrolled up where it is
    h.receive_chat(30..31);
    h.frames(2);
    assert!(h.in_log_view("<Bot> line 10"));
    assert!(!h.in_log_view("<Bot> line 30"));
    for _ in 0..3 {
        h.press(KeyCode::PageDown);
    }
    h.frames(2);
    assert!(h.in_log_view("<Bot> line 30"));
    // and a log at its end shows it
    h.receive_chat(31..32);
    h.frames(2);
    assert!(h.in_log_view("<Bot> line 31"));
    // a click in the log keeps the chat open
    h.click(&L::text("<Bot> line 31"));
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    assert!(*h.world().resource::<UIMode>() == UIMode::Typing);

    // dragging the thumb of the scrollbar scrolls the log, and the chat stays open
    let log = h.get(&L::marker::<ChatLog>());
    let at_end = h.world().get::<ScrollPosition>(log).unwrap().y;
    let thumb = L::marker::<ScrollbarThumb>().within(L::marker::<ChatRoot>());
    let grabbed = h.center_of(&thumb);
    h.mouse_down(&thumb);
    for step in 1..=4 {
        h.hover_at(grabbed - Vec2::Y * 15. * step as f32);
    }
    h.mouse_up();
    h.frames(2);
    let dragged = h.world().get::<ScrollPosition>(log).unwrap().y;
    assert!(dragged < at_end, "{at_end} then {dragged}");
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    assert!(*h.world().resource::<UIMode>() == UIMode::Typing);

    // a full log scrolled up keeps its lines in place as the oldest go
    let kept = (0..32)
        .map(|index| format!("<Bot> line {index}"))
        .find(|line| h.in_log_view(line))
        .expect("a line in view");
    let before = h.center_of(&L::text(&kept));
    h.receive_chat(32..100);
    h.frames(2);
    assert!(!h.is_visible(&L::text("<Tester> one")));
    assert_eq!(h.center_of(&L::text(&kept)), before);

    // a letter the pixel font lacks has the text being written drawn in Fira Mono
    assert_eq!(h.field_font(), FONT_PATH);
    h.type_text("été");
    assert_eq!(h.field_font(), MONO_FONT_PATH);
    h.press(KeyCode::Escape);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_chat_sits_above_the_hotbar_and_its_item_name() {
    let folder = GameFolder::new().with_world("Above").with_items(
        "Above",
        "Tester",
        &[(1, ItemId::Stone, 12)],
    );
    let small = Options {
        width: 800,
        height: 600,
        ..default()
    };
    let mut h = enter_world_with(folder, "Above", small);
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 1) == Some((ItemId::Stone, 12))
    });
    for (width, height) in [(800, 600), (1920, 1080)] {
        h.resize(width, height);
        h.frames(2);
        // the name shows when the selection changes
        h.press(KeyCode::Digit1);
        h.press(KeyCode::Digit2);
        h.wait_for(&L::marker::<HotbarItemName>(), SCREEN);
        h.press(KeyCode::KeyT);
        h.frames(2);
        let chat = h.rect_of(&L::marker::<ChatRoot>());
        let name = h.rect_of(&L::marker::<HotbarItemName>());
        let bar = h.rect_of(&L::marker::<Hotbar>());
        for (a, b) in [(chat, name), (chat, bar), (name, bar)] {
            assert!(
                a.intersect(b).is_empty(),
                "{a:?} and {b:?} at {width}x{height}"
            );
        }
        let scale = h.world().resource::<UiScale>().0;
        assert!(
            name.min.y - chat.max.y >= SPACE_S * scale - 0.5,
            "{chat:?} above {name:?} at {width}x{height}"
        );
        let window = Rect::new(0., 0., width as f32, height as f32);
        assert!(
            window.contains(chat.min) && window.contains(chat.max),
            "{chat:?} at {width}x{height}"
        );
        // never as far as the reticle, at the centre; as wide as it may be when there is room
        if width == 800 {
            assert!(
                chat.max.x <= width as f32 / 2.0 - SPACE_L * scale + 0.5,
                "{chat:?} reaches the reticle at {width}x{height}"
            );
        } else {
            assert!(
                (chat.width() - CHAT_WIDTH * scale).abs() < 0.5,
                "{chat:?} at {width}x{height}"
            );
        }
        // the log and the field follow the chat
        for part in [
            h.rect_of(&L::marker::<ChatLog>()),
            h.rect_of(&L::marker::<ChatInput>()),
        ] {
            assert!(
                part.max.x <= chat.max.x + 0.5,
                "{part:?} leaves {chat:?} at {width}x{height}"
            );
        }
        h.press(KeyCode::Escape);
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn typing_in_the_chat_keeps_the_game_keys_quiet() {
    let mut h = enter_world(GameFolder::new().with_world("Quiet"), "Quiet");
    h.press(KeyCode::KeyT);
    // The key that opens the chat is not typed into it
    assert_eq!(h.value_of::<ChatInput>(), "");
    h.type_text("e");
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    // Neither the digit, the debug keys nor the wheel reach the game
    h.type_text("3");
    for key in [
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
    ] {
        h.press(key);
    }
    h.scroll(-1.);
    assert_eq!(
        h.world_mut()
            .query::<&Hotbar>()
            .single(h.world())
            .unwrap()
            .selected,
        0
    );
    assert!(!h.is_visible(&L::marker::<HudRoot>()));
    assert!(!h.world().resource::<WireframeConfig>().global);
    assert!(!h.world().resource::<DebugOptions>().any_enabled());
    assert!(!h.is_visible(&L::marker::<FpsCounter>()));
    assert_eq!(h.value_of::<ChatInput>(), "e3");

    // Escape closes the chat, and the pause menu does not open
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    assert!(!h.is_visible(&L::role(Role::Button, "Back to game")));

    // The closed field holds no key
    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));

    // An open inventory closes with its key, and the debug keys wait for the pause menu too
    h.press(KeyCode::KeyE);
    assert!(!h.is_visible(&L::marker::<InventoryRoot>()));
    h.press(KeyCode::Escape);
    assert!(h.is_visible(&L::role(Role::Button, "Back to game")));
    for key in [
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
    ] {
        h.press(key);
    }
    assert!(!h.is_visible(&L::marker::<HudRoot>()));
    assert!(!h.world().resource::<WireframeConfig>().global);
    assert!(!h.world().resource::<DebugOptions>().any_enabled());
    assert!(!h.is_visible(&L::marker::<FpsCounter>()));
    h.press(KeyCode::Escape);
    h.press(KeyCode::F3);
    assert!(h.is_visible(&L::marker::<HudRoot>()));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn clicking_outside_the_chat_closes_it() {
    let mut h = enter_world(GameFolder::new().with_world("Outside"), "Outside");
    h.press(KeyCode::KeyT);
    h.type_text("draft");
    h.click(&L::marker::<Hotbar>());
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));
    assert!(*h.world().resource::<UIMode>() == UIMode::Closed);

    h.press(KeyCode::KeyE);
    assert!(h.is_visible(&L::marker::<InventoryRoot>()));
}

impl Harness {
    /// The grey text after the caret of the chat field, when it shows.
    fn ghost(&mut self) -> Option<String> {
        if !self.is_visible(&L::marker::<Ghost>()) {
            return None;
        }
        let world = self.world_mut();
        let text = world
            .query_filtered::<&Text, With<Ghost>>()
            .single(world)
            .unwrap();
        Some(text.0.clone())
    }

    /// Whether the panel of the suggestions shows a row reading `row`.
    fn suggests(&mut self, row: &str) -> bool {
        self.is_visible(&L::text(row).within(L::marker::<SuggestionPanel>()))
    }
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn commands_are_completed_as_they_are_typed() {
    let mut h = enter_world(GameFolder::new().with_world("Complete"), "Complete");

    // a slash lists the commands with their arguments
    h.type_text("/");
    assert!(h.is_visible(&L::marker::<SuggestionPanel>()));
    assert!(h.suggests("give <item> <count>"));
    assert!(h.suggests("time set <day|night>"));
    h.frames(10);
    h.snapshot("slash");

    // the rest of the first suggestion shows after the caret, and Tab takes it with a space
    h.type_text("g");
    assert_eq!(h.ghost().as_deref(), Some("ive"));
    h.press(KeyCode::Tab);
    assert_eq!(h.value_of::<ChatInput>(), "/give ");
    assert_eq!(h.focus_label().as_deref(), Some("Chat"));
    assert!(h.is_visible(&L::text("/give <item> <count>")));
    assert_eq!(h.ghost().as_deref(), Some("<item> <count>"));
    h.frames(10);
    h.snapshot("items");

    // Down chooses the next one, which Tab takes
    h.type_text("oa");
    assert!(h.suggests("oak_leaves"));
    h.press(KeyCode::ArrowDown);
    assert_eq!(h.ghost().as_deref(), Some("k_log"));
    h.frames(10);
    h.snapshot("chosen");
    h.press(KeyCode::Tab);
    assert_eq!(h.value_of::<ChatInput>(), "/give oak_log ");
    assert_eq!(h.ghost().as_deref(), Some("<count>"));
    h.type_text("5");
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("Gave 5 OakLog"), SERVER);

    // a command recalled shows no suggestions: the arrows walk the sent messages, Escape closes
    h.press(KeyCode::KeyT);
    h.press(KeyCode::ArrowUp);
    assert_eq!(h.value_of::<ChatInput>(), "/give oak_log 5");
    assert!(!h.is_visible(&L::marker::<SuggestionPanel>()));
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));

    // a wrong line is told before it is sent, and Escape puts the panel away first
    h.type_text("/give diamond");
    assert!(h.is_visible(&L::text("Unknown item: diamond")));
    assert_eq!(h.ghost(), None);
    h.frames(10);
    h.snapshot("error");
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<SuggestionPanel>()));
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    h.press(KeyCode::Escape);
    assert!(!h.is_visible(&L::marker::<ChatRoot>()));

    // a click takes a suggestion
    h.type_text("/t");
    h.click(&L::text("time set <day|night>"));
    assert_eq!(h.value_of::<ChatInput>(), "/time ");
    assert!(h.is_visible(&L::marker::<ChatRoot>()));
    h.press(KeyCode::Tab);
    h.type_text("n");
    h.press(KeyCode::Tab);
    assert_eq!(h.value_of::<ChatInput>(), "/time set night");
    assert!(!h.is_visible(&L::marker::<SuggestionPanel>()) || !h.suggests("night"));
    h.press(KeyCode::Enter);
    h.wait_for(&L::text_containing("Time set to night"), SERVER);
}
