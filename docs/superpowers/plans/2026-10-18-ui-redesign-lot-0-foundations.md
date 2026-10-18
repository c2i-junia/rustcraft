# UI Redesign Lot 0 (Foundations) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Lay the foundations of the redesign: an e2e harness that drives `bevy_picking` and real logical keys, the theme tokens, `UiScale`, focus (Tab, ring, arrows), a themed button, and an `EditableText` field that replaces `bevy_simple_text_input` everywhere, with Enter submitting.

**Architecture:** A new `client/src/ui/widgets/` module (`widgets_plugin`, added by `app::add_client`, so the game and the harness share it) wraps the headless `bevy::ui_widgets` / `bevy::input_focus` / `bevy::text::EditableText` APIs with our visuals, `TabIndex`, accessibility roles and labels. Each widget is a function returning a bundle or spawning a small tree, plus its own systems and observers. The existing screens keep their legacy `Interaction` buttons; only their text inputs move to the new field.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets`, `bevy::input_focus`, `bevy::text::EditableText`, `bevy::picking`), `accesskit` 0.24, lavapipe for the headless e2e harness.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md` (only "Lot 0 — Foundations"; the "Visual system", "Architecture" and "Testing" sections bind every task). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md` (cited below as "notes §n").

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `test(client): …`, `refactor(client): …`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift (`.cargo/config.toml`), which runs no destructor while a panic unwinds: a failing test's harness is never dropped. Do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`, like the existing ones.
- Never a bare `git stash`; set work aside with a WIP commit.
- `bevy::prelude::Button` is the legacy widget. Import the headless one as `use bevy::ui_widgets::Button as UiButton;`.
- Put the widget component (`UiButton`, or `AccessibilityNode(accesskit::Node::new(Role::TextInput))`) **before** `AccessibleLabel` in a bundle, so the widget's role wins (notes §5).
- The client is a binary crate: widget API that no screen uses before Lot 1 (the themed button, `ArrowNav`, theme tokens not yet read) would trip `dead_code`. Allow it on the item (or with `#![allow(dead_code)]` on `theme.rs` only) with a comment naming why; never crate-wide.
- Widget e2e tests live in a new `client/src/e2e/widgets.rs`, start from `main_menu()`, and spawn their probe nodes under a root with `GlobalZIndex(1)` so picking hits them above the menu. Spawning probes is setup; driving them goes through `click`/`hover`/`press`/`type_text` only.
- The legacy `Interaction` buttons of the existing screens are not migrated in Lot 0.

## Review Focus

1. Escape while the chat field has focus closes the chat only; the pause menu (which reads Escape from `ButtonInput` in `Update`) must not open on the same frame. (Task 7: `typing_in_the_chat_keeps_the_game_keys_quiet`)
2. A character and Enter arriving in the same frame (fast typing) submit the value with that character in it. (Task 5: `a_character_and_enter_in_one_frame_submit_both`)
3. A click elsewhere while the chat field has focus closes the chat, so the game is not left in `UIMode::Opened` with no focused field and no way to close it. (Task 7: `clicking_outside_the_chat_closes_it`)
4. A click on a legacy button clears the focus (`click_to_focus`); the player can click the field again, type, and Enter still submits. (Task 6: `after_create_a_click_on_the_field_types_again`)
5. Window heights between the steps (1079, 1200) or under 720 (600, 400) give the lower step, never a scale under 1. (Task 2: `auto_scale_follows_the_window_height`, and the e2e resize back to 800×600)

---

### Task 1: The harness drives picking and logical keys, and finds widgets by role

**Files:**
- Modify: `client/src/e2e/input.rs` (cursor, buttons, wheel, keys), `client/src/e2e/locator.rs` (`Target::Role`, `Locator::role`), `client/Cargo.toml` (`accesskit = "0.24"`; `bevy = { version = "=0.19.1", features = ["serialize"] }`), `README.md` ("End-to-end UI tests"), `docs/ARCHITECTURE.md` ("End-to-end tests", one phrase)
- Test: `client/src/e2e/input.rs` (tests module), `client/src/e2e/locator.rs` (tests module)

**Interfaces:**
- Produces: `Locator::role(role: accesskit::Role, label: &str) -> Locator` (matches an entity whose `AccessibilityNode` has that role and `label() == Some(label)`; `Debug` prints `Role(Button, "Play")`); `fn logical_key_of(key: KeyCode) -> Key` in `input.rs`; `press`/`key_down`/`key_up` now send `logical_key_of(key)` with `text: None`; `click`/`hover`/`scroll` also write `WindowEvent::{CursorMoved, MouseButtonInput, MouseWheel}`.

- [ ] **Step 1: Write the failing tests**

In `input.rs` tests (e2e; each spawns its probe on `main_menu()`):

```rust
fn clicking_a_widget_button_activates_it()     // (UiButton, AccessibleLabel::new("Probe"), Node 120×40, observer counting On<Activate>)
    h.click(&L::role(Role::Button, "Probe")); assert_eq!(activations, 1);
fn the_wheel_scrolls_a_scroll_area_under_the_cursor() // (ScrollArea, Name "Probe list", Node 200×100, overflow: Overflow::scroll_y()) with a 200×1000 child
    h.hover(&L::name("Probe list")); h.scroll(-1.); h.frames(1);
    assert!(scroll_position.y > 0.0);
fn named_keys_edit_an_editable_text()          // (EditableText::default(), AccessibilityNode(Node::new(Role::TextInput)), AccessibleLabel::new("Probe field"), Node 200×30)
    h.click(&L::role(Role::TextInput, "Probe field")); h.type_text("abcd");
    h.press(KeyCode::Backspace); h.press(KeyCode::ArrowLeft); h.type_text("x");
    assert_eq!(value, "abxc");
```

Plain unit test in `input.rs`: `logical_keys_are_the_named_keys` — `Enter→Key::Enter`, `Escape→Key::Escape`, `Backspace→Key::Backspace`, `Tab→Key::Tab`, `ArrowLeft→Key::ArrowLeft`, `ShiftLeft→Key::Shift`, `ControlRight→Key::Control`, `Space→Key::Space`, `F3→Key::F3`, `KeyE→Key::Character("e".into())`, `Digit1→Key::Character("1".into())`.

In `locator.rs` tests: `a_widget_is_found_by_its_role_and_label` (spawn `UiButton` + `AccessibleLabel::new("Play probe")`; `h.get(&L::role(Role::Button, "Play probe"))`; `L::role(Role::CheckBox, "Play probe")` finds nothing), and `#[should_panic(expected = r#"Role(Button, "Nope")"#)] fn a_missing_role_is_named_in_the_failure`.

- [ ] **Step 2: Run them to verify they fail**

Run: `timeout 300 cargo test -q -p client -- e2e::input e2e::locator`
Expected: compile error on `Locator::role` / `logical_key_of`; once stubbed, the click test fails with 0 activations, the scroll test with `y == 0`, the text test with `""`.

- [ ] **Step 3: Implement**

- Add the two dependencies. `Cargo.lock` must keep `bevy 0.19.1` and `accesskit 0.24.1`.
- `move_cursor` also writes `WindowEvent::CursorMoved(CursorMoved { window, position: physical / scale_factor, delta: None })`; `click_with` writes `WindowEvent::MouseButtonInput(..)` next to the existing `MouseButtonInput`; `scroll` writes `WindowEvent::MouseWheel(..)` next to the existing `MouseWheel`. Keep the existing writes: the legacy `Interaction` and `mouse_scroll` still read them (notes §6).
- `logical_key_of` covers Enter, Escape, Backspace, Delete, Tab, the four arrows, Home, End, PageUp, PageDown, Shift/Control/Alt (left and right), Space, F1–F12, letters and digits; anything else stays `Key::Unidentified`.
- `Target::Role(Role, String)`; `find` queries `(Entity, &AccessibilityNode)` and keeps `is_shown` and `within` as for the other targets.
- README "End-to-end UI tests": in **Locators**, add `L::role(Role::Button, "Play")` (the accessibility role and label every new widget carries); in **Real input**, say that `click`, `hover` and `scroll` also send the window events `bevy_picking` reads, so the `ui_widgets` work headless, and that keys carry their logical key. ARCHITECTURE "End-to-end tests": "by their text, `Name`, marker or accessibility role", "injects the mouse (including the window events of picking) and keyboard as messages".

- [ ] **Step 4: Run the tests, then the whole e2e suite**

Run: `timeout 300 cargo test -q -p client -- e2e::input e2e::locator` → PASS. Then `timeout 600 cargo test -q -p client e2e::` → all existing tests still PASS (57 + the new ones; `every_screen_at_three_resolutions` stays ignored).

- [ ] **Step 5: Commit**

`git commit -m "test(client): drive picking and logical keys from the e2e harness, and find widgets by role"`

---

### Task 2: Theme tokens, the widgets plugin, and `UiScale` from the window height

**Files:**
- Create: `client/src/ui/theme.rs`, `client/src/ui/widgets/mod.rs`, `client/src/ui/widgets/scale.rs`, `client/src/e2e/widgets.rs`
- Modify: `client/src/ui/mod.rs` (`pub mod theme; pub mod widgets;`), `client/src/app.rs` (add `widgets::widgets_plugin` to the plugins of `add_client`), `client/src/e2e/mod.rs` (`mod widgets;`)

**Interfaces:**
- Produces, in `theme.rs` (exact values from the spec table):
  `PANEL = Color::srgba_u8(0x11, 0x13, 0x18, 237)` (93 %), `RAISE = #252A33`, `RAISE_HOVER = #2E3440`, `BEVEL_HI = #48505F`, `BEVEL_LO = #07080A`, `BEVEL_WIDTH: f32 = 2.0`, `SLOT = #0C0E12`, `ACCENT = #F0B54A`, `OK = #78C75F`, `DANGER = #E3593F`, `TEXT = #EFE9DA`, `MUTED = #9AA2AE`, `FOCUS_RING_WIDTH: f32 = 3.0`, `FOCUS_RING_OFFSET: f32 = 2.0`, `DISABLED_ALPHA: f32 = 0.4`, `FONT_S/M/L/XL: f32 = 16/20/24/32`, `SPACE_XS/S/M/L/XL/XXL: f32 = 4/8/12/16/24/32` (colours via `Color::srgb_u8`, which is `const`).
- Produces: `pub fn widgets_plugin(app: &mut App)` in `widgets/mod.rs`, which adds each widget file's `pub(super) fn plugin(app: &mut App)`; `pub fn auto_ui_scale(window_height: f32) -> f32` in `scale.rs`.

- [ ] **Step 1: Write the failing tests**

Unit tests in `scale.rs`:

```rust
fn auto_scale_follows_the_window_height() {
    for (height, scale) in [(400., 1.), (600., 1.), (720., 1.), (900., 1.), (1079., 1.),
                            (1080., 1.5), (1200., 1.5), (1440., 2.), (2160., 3.)] {
        assert_eq!(auto_ui_scale(height), scale, "{height}");
    }
}
fn auto_scale_keeps_the_logical_height_at_least_600() // every height in 600..=2400: height / auto_ui_scale(height) >= 600.
```

E2e in `e2e/widgets.rs`: `the_ui_scale_follows_the_window_height` — `Harness::start(GameFolder::new(), Options { width: 1920, height: 1080, ..default() })`, wait for "Singleplayer", `assert_eq!(UiScale, 1.5)`; `h.resize(800, 600)`, `h.frames(2)`, `assert_eq!(UiScale, 1.0)`.

- [ ] **Step 2: Run them to verify they fail**

Run: `timeout 300 cargo test -q -p client -- ui::widgets::scale e2e::widgets` → compile error, then the e2e test fails with `UiScale(1.0)` at 1080.

- [ ] **Step 3: Implement**

- `auto_ui_scale(h) = max(1, floor(h / 720 × 2) / 2)`. The spec's "logical height ≥ 600" clamp is a no-op for the automatic scale (above 1 the logical height is ≥ 720; below 720 the `max(1, …)` wins): it arrives with the player's forced scale in Lot 1. Do not add that setting.
- `scale::plugin` adds a system in `Update` reading `Single<&Window, With<PrimaryWindow>>` and writing `UiScale` with `set_if_neq` from `window.height()` — the **logical** height, because `UiScale` multiplies the window's own scale factor (notes §4).
- `theme.rs` holds every token of the spec table (`#![allow(dead_code)]` with the reason, per Global Constraints). The legacy `style.rs` stays as it is.

- [ ] **Step 4: Run the tests and the suite**

`timeout 300 cargo test -q -p client -- ui::widgets::scale e2e::widgets` → PASS; full suite green. The goldens (1280×720) do not change; review snapshots at 1920×1080 now render at 1.5 on purpose.

- [ ] **Step 5: Commit**

`git commit -m "feat(client): theme tokens, widgets plugin and a UI scale that follows the window height"`

---

### Task 3: Focus — Tab navigation, the amber ring, arrow navigation

**Files:**
- Create: `client/src/ui/widgets/focus.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/e2e/widgets.rs`

**Interfaces:**
- Consumes: `theme::{ACCENT, FOCUS_RING_WIDTH, FOCUS_RING_OFFSET}`.
- Produces: `#[derive(Component)] #[require(Outline = …)] pub struct FocusRing;` (requires `Outline { width: px(FOCUS_RING_WIDTH), offset: px(FOCUS_RING_OFFSET), color: Color::NONE }`); `#[derive(Component)] pub struct ArrowNav;`. `focus::plugin` adds `TabNavigationPlugin`, `DirectionalNavigationPlugin` (its `AutoNavigationConfig` is read by the arrow observer), the ring system and the `arrow_nav` observer. Screens put `AutoFocus` on their first useful control.

- [ ] **Step 1: Write the failing tests**

Unit test in `focus.rs` (`MinimalPlugins` app, `InputFocus` + `InputFocusVisible` resources, the ring system only):
`the_ring_shows_on_the_focus_or_its_container_only_while_visible` — parent `FocusRing` with a child; focus on the child, visible → parent `Outline.color == ACCENT`; visible false → `Color::NONE`; focus on an unrelated entity → `NONE`.

E2e in `e2e/widgets.rs`, with a helper spawning a `TabGroup::new(0)` root of named `(Node 80×30, TabIndex(0), FocusRing)` probes:
- `tab_and_shift_tab_move_the_focus_in_order` — A (with `AutoFocus`), B, C: focus A at start; Tab → B; Tab → C; Tab → A (wraps); `key_down(ShiftLeft)`, `press(Tab)`, `key_up(ShiftLeft)` → C.
- `the_focus_ring_follows_tab_and_hides_on_click` — after `AutoFocus` no ring anywhere; Tab → only B's `Outline.color == ACCENT`; `click(&L::name("A"))` → focus A, `InputFocusVisible(false)`, no ring.
- `arrows_move_the_focus_inside_an_arrow_nav_container` — a 2×2 `ArrowNav` grid TL, TR, BL, BR plus X outside it, to the right: focus TL; ArrowRight → TR; ArrowDown → BR; ArrowRight → still BR (X is outside); `InputFocusVisible` is true.
- `arrows_do_nothing_outside_an_arrow_nav_container` — focus X by click; ArrowLeft → still X.

- [ ] **Step 2: Run them to verify they fail**

Run: `timeout 300 cargo test -q -p client -- ui::widgets::focus e2e::widgets` → compile error, then Tab does not move the focus (no `TabNavigationPlugin`).

- [ ] **Step 3: Implement**

- Ring: a `PostUpdate` system that sets each `FocusRing`'s `Outline.color` to `ACCENT` when `InputFocusVisible` is true and the focus is that entity or a descendant of it (`IsFocusedHelper::is_focus_within`), else `Color::NONE`; `set_if_neq`-style, so nothing is marked changed needlessly. Toggle the colour, not the component (Bevy's `Outline` docs).
- `arrow_nav`: the observer of notes §1.6, unchanged in substance (candidates are the visible `TabIndex >= 0` descendants of the `ArrowNav` entity being visited; `find_best_candidate`; `propagate(false)` only when it moved). `EditableText` consumes the arrows before they bubble, so a field inside a container keeps them.
- Note for later tasks: with `TabNavigationPlugin`, a picking press focuses the nearest `TabIndex` ancestor and a press elsewhere (legacy buttons included) clears the focus.

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit**

`git commit -m "feat(client): Tab navigation, a focus ring and arrow navigation for the UI"`

---

### Task 4: The themed button

**Files:**
- Create: `client/src/ui/widgets/button.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/e2e/widgets.rs`

**Interfaces:**
- Consumes: `theme::*`, `FocusRing`.
- Produces: `#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)] pub enum ButtonVariant { #[default] Default, Primary, Danger, Ghost }`; `#[derive(Component)] pub struct ThemedButton(pub ButtonVariant);` `pub fn button(label: &str, variant: ButtonVariant, font: Handle<Font>) -> impl Bundle` = `(UiButton, AccessibleLabel::new(label), TabIndex(0), Hovered::default(), FocusRing, ThemedButton(variant), Node { border: BEVEL_WIDTH, padding SPACE_S × SPACE_L, centred }, BorderColor, BackgroundColor, children![(Text::new(label), TextFont FONT_M, TextColor)])`. Callers observe `On<Activate>`; disabling is inserting `InteractionDisabled`.

- [ ] **Step 1: Write the failing tests**

Unit tests in `button.rs` on the private `fn button_colors(variant: ButtonVariant, hovered: bool, pressed: bool, disabled: bool) -> ButtonColors` (`struct ButtonColors { background: Color, border: BorderColor, text: Color }`):
- `a_default_button_is_raised_and_lightens_on_hover` — background `RAISE`, then `RAISE_HOVER`; border top/left `BEVEL_HI`, bottom/right `BEVEL_LO`; text `TEXT`.
- `pressing_inverts_the_bevel` — top/left `BEVEL_LO`, bottom/right `BEVEL_HI`.
- `a_disabled_button_is_at_40_percent` — every colour's alpha is `DISABLED_ALPHA` × its normal alpha; hover and press change nothing.
- `the_variants_have_their_colours` — Primary background `OK`, Danger `DANGER` (both with `PANEL`-coloured text, for contrast on the light fills); Ghost `Color::NONE` background and border at rest, `RAISE_HOVER` background on hover, text `TEXT`.
- `a_button_has_the_button_role_and_its_label` — spawn `button("Play", Default, Handle::default())` in a bare `World`; its `AccessibilityNode` has `Role::Button` and label `"Play"` (the ordering caveat of notes §5).

E2e in `e2e/widgets.rs` (probe = `button("Play", …)` with an observer counting `Activate`):
- `clicking_a_themed_button_activates_it` → 1.
- `enter_and_space_activate_the_focused_button` — insert `AutoFocus`; `press(Enter)` → 1; `press(Space)` → 2.
- `a_disabled_button_does_not_activate` — insert `InteractionDisabled`; click and Enter → 0.
- `hovering_a_themed_button_lightens_it` — `hover` → `BackgroundColor(RAISE_HOVER)` (proves `Hovered` through picking).

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::widgets::button e2e::widgets` → compile error.

- [ ] **Step 3: Implement** the bundle and a style system that applies `button_colors` on `Changed<Hovered>`, `Added<Pressed>`, `Added<InteractionDisabled>`, `Added<ThemedButton>` and the removals of `Pressed` / `InteractionDisabled` (the feathers pattern, notes §7). No screen uses it in Lot 0: allow `dead_code` on `button` and the unused variants with "first used by the main menu (Lot 1)".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a themed button on the ui_widgets button"`

---

### Task 5: The text field

**Files:**
- Create: `client/src/ui/widgets/text_field.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/e2e/widgets.rs`

**Interfaces:**
- Consumes: `theme::*`, `FocusRing`.
- Produces:
  ```rust
  pub struct TextField<'a> { pub label: &'a str, pub placeholder: &'a str,
      pub max_characters: Option<usize>, pub font: Handle<Font>, pub font_size: f32 }
  pub struct TextFieldParts { pub root: Entity, pub input: Entity }
  pub fn spawn_text_field(commands: &mut Commands, field: &TextField) -> TextFieldParts;
  #[derive(EntityEvent)] pub struct Submit { pub entity: Entity, pub value: String }
  #[derive(EntityEvent)] pub struct Cancel { pub entity: Entity }
  #[derive(Component, Default)] pub struct FieldError(pub Option<String>);
  ```
  Tree: `root` (column, `width: 100%`) → frame (`FocusRing`, `SLOT` background, sunken bevel: top/left `BEVEL_LO`, bottom/right `BEVEL_HI`, padding `SPACE_S`) → [`input`, placeholder]; then the error message under the frame. `input` = `EditableText { max_characters, ..default() }`, `TextLayout` `LineBreak::NoWrap`, `TextFont`, `TextColor(TEXT)`, `TextCursorStyle { color: TEXT, selection_color: ACCENT.with_alpha(0.4), unfocused_selection_color: Color::NONE, selected_text_color: None }`, `TabIndex(0)`, `AccessibilityNode(Node::new(Role::TextInput))` then `AccessibleLabel::new(label)`, `FieldError::default()`, `flex_grow: 1`. Placeholder: absolute `Text`, `MUTED`, `Pickable::IGNORE` (a click on it reaches the field). Message: `Text`, `FONT_S`, `DANGER`, `Display::None` while there is no error. Screens put their marker (e.g. `WorldNameInput`) and their `Submit`/`Cancel` observers on `input`, and `AutoFocus` on it when it is the screen's first control.

- [ ] **Step 1: Write the failing tests** (e2e, `e2e/widgets.rs`; a helper spawns a `TextField { label: "Name", placeholder: "Your name", max_characters: Some(5), .. }` with `AutoFocus` on `input` and observers recording `Submit` values and `Cancel` counts)

- `enter_in_a_text_field_submits_its_value` — `type_text("abc")`, `press(Enter)` → submits `["abc"]` (exactly once: the release does not submit again).
- `a_character_and_enter_in_one_frame_submit_both` — write a `KeyboardInput` for `c` and one for Enter in the same frame (a local helper writing both messages, then one frame) → submits `["c"]`.
- `escape_in_a_text_field_cancels_it` — `press(Escape)` → 1 cancel, no submit.
- `the_placeholder_shows_only_while_the_field_is_empty` — `L::text("Your name")` visible; after `type_text("a")` not visible; after Backspace visible again.
- `the_field_keeps_at_most_max_characters` — `type_text("abcdefg")` → value `"abcde"`.
- `an_error_shows_under_the_field_with_a_red_bevel` — set `FieldError(Some("Bad name".into()))` on `input` → `L::text("Bad name")` visible and the frame's four border sides are `DANGER`; set `FieldError(None)` → hidden and the sunken bevel is back.
- `a_click_focuses_the_field_and_shows_its_ring_after_tab` — `click(&L::role(Role::TextInput, "Name"))` → `InputFocus` is `input`; no ring (click); a probe `TabIndex` node before it in the same `TabGroup`, Tab from it → frame `Outline.color == ACCENT`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client e2e::widgets` → compile error.

- [ ] **Step 3: Implement**

- An entity observer on `input` for `FocusedInput<KeyboardInput>`: on a pressed, non-repeat Enter while not composing, or Escape, stop propagation and insert a private request marker; a `PostUpdate` system `.after(EditableTextSystems)` turns it into `Submit { value: text.value().to_string() }` or `Cancel` (notes §3). **Cancel is deferred like Submit** so that systems in `Update` (`set_ui_mode`, the pause menu's Escape) still see the field focused on that frame.
- The placeholder's and the error message's `Display`, and the frame's border, follow `EditableText::value()` and `Changed<FieldError>` in `PostUpdate` after `EditableTextSystems`.

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a text field on EditableText with submit, cancel, placeholder and error"`

---

### Task 6: The world and server forms use the new field

**Files:**
- Modify: `client/src/ui/menus/list_screen.rs`, `client/src/ui/menus/solo.rs`, `client/src/ui/menus/multi.rs` (code and unit tests), `client/src/e2e/helpers.rs` (`value_of`), `client/src/e2e/solo.rs`, `client/src/e2e/multi.rs`, `client/tests/golden/menu_solo.png`, `client/tests/golden/menu_multi.png`

**Interfaces:**
- Consumes: `spawn_text_field`, `TextField`, `Submit`, `FieldError`, `AutoFocus`, `TabGroup`, `Locator::role`.
- Produces: `InputField<'a> { placeholder: &'a str, focused: bool, max_characters: Option<usize> }` (`game_font` is dropped: every field uses the game font); `ListScreenParts.inputs` holds the `input` entities (the markers `WorldNameInput`, `ServerNameInput`, `ServerIpInput` land on them); the list screen root carries `TabGroup::new(0)`. In `solo.rs`: `const INVALID_WORLD_NAME: &str = "Use 1 to 32 letters, digits, spaces, _ or -";`, `const WORLD_EXISTS: &str = "A world with this name already exists";`, `fn new_world_name(typed: &str, list: &WorldList) -> Result<String, &'static str>`. In `multi.rs`: `const INVALID_ADDRESS: &str = "Enter an address like 127.0.0.1:8000";`, `fn new_server(name: &str, ip: &str) -> Result<ServerItem, &'static str>`. `Harness::value_of::<M>()` reads `EditableText::value()`.

- [ ] **Step 1: Write the failing tests**

Unit tests: `solo.rs` `a_new_world_name_is_checked` — `""` → `Ok("new_world_1")` on an empty list; `"bad/name"` → `Err(INVALID_WORLD_NAME)`; an existing name → `Err(WORLD_EXISTS)`; `"Nouveau"` → `Ok`. `multi.rs` `a_new_server_needs_an_address` — `("Home", "10.0.0.2:8000")` → `Ok`; `("Bad", "not an ip")` → `Err(INVALID_ADDRESS)`. The existing `Menu` helper of `multi.rs` spawns `EditableText::new("")` instead of `TextInputValue`, and `add` replaces it with `EditableText::new(value)`.

E2e, `e2e/solo.rs`:
- new `enter_in_the_world_name_creates_the_world` — `type_text("Entered")`, `press(Enter)` → `L::name("Play Entered")` appears; `value_of::<WorldNameInput>() == ""`.
- new `after_create_a_click_on_the_field_types_again` — `type_text("One")`, click "Create world", `click(&L::role(Role::TextInput, "World name"))`, `type_text("Two")`, `press(Enter)` → rows "Play One" and "Play Two".
- `an_invalid_name_adds_no_world` and `a_duplicate_name_adds_no_world` additionally assert `L::text(INVALID_WORLD_NAME)` / `L::text(WORLD_EXISTS)` visible.
- `the_world_list_matches_its_golden` clicks the title `L::text("World list")` before the screenshot: the caret blinks on wall-clock time and covers more than the 50 pixels a golden tolerates; the click clears the focus, so no caret.

E2e, `e2e/multi.rs`:
- `add_server` clicks `L::role(Role::TextInput, "Server name")` and `L::role(Role::TextInput, "Server IP")` (the locators change, the journey does not); `is_inactive::<M>()` becomes `has_focus::<M>()` (`InputFocus` holds the entity marked `M`), and `clicking_a_field_focuses_it_alone` asserts it the other way round.
- new `tab_and_enter_add_a_server` — click the name field, `type_text("Home")`, `press(Tab)`, `type_text("10.0.0.2:8000")`, `press(Enter)` → "Play Home".
- new `enter_in_the_name_field_adds_the_server` — fill both fields by clicks, click the name field again, `press(Enter)` → the row appears.
- `an_invalid_address_adds_no_server` additionally asserts `L::text(INVALID_ADDRESS)` visible.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- menus::solo menus::multi e2e::solo e2e::multi` → compile errors, then the Enter tests fail (Enter does nothing yet).

- [ ] **Step 3: Implement**

- `list_screen.rs`: `spawn_input` places `spawn_text_field(TextField { label: placeholder, placeholder, max_characters, font: load_font, font_size: FONT_M })` in the grid cell (the cell keeps its grid placement, loses its border), with `AutoFocus` on `input` when `focused`. The fields: world name `Some(32)` (`MAX_WORLD_NAME_LENGTH`), server name `Some(32)`, server IP `Some(64)`.
- `solo.rs`: the `Add` branch of `solo_action` and a `Submit` observer on the name field share one function that calls `new_world_name`, adds the row and clears the field on success, and sets `FieldError(Some(msg))` / `FieldError(None)` on the field.
- `multi.rs`: the `Add` branch of `multiplayer_action` and a `Submit` observer on both fields share one function that calls `new_server` and sets `FieldError` on the IP field. The fields are not cleared (unchanged behaviour).
- Remove the `bevy_simple_text_input` imports of these files and of `e2e/helpers.rs` / `e2e/multi.rs`. Escape on these screens still goes back through `escape_button` (it reads `ButtonInput`, which the field's stopped propagation does not affect): `back_and_escape_return_to_the_main_menu` keeps passing with the world name focused.

- [ ] **Step 4: Run the tests; regenerate the two goldens on purpose**

Run the e2e solo and multi tests → PASS except the two goldens. Read `client/tests/golden/menu_solo.actual.png` and `menu_multi.actual.png` (and their `.diff.png`): the fields must show a sunken bevel on the slot colour with the muted placeholder, no caret, no ring, and nothing else may differ. Then `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_world_list_matches_its_golden the_server_list_matches_its_golden`, rerun without it → PASS, and check that no `.actual.png` / `.diff.png` is left. No other golden may change; if one does, it is a bug. Full suite green.

- [ ] **Step 5: Commit** (the report lists the changed locators of `e2e/multi.rs` and the golden click of `e2e/solo.rs`)

`git commit -m "feat(client): the world and server forms use the new text field, and Enter submits them"`

---

### Task 7: The chat and the typing mode use the new field; `bevy_simple_text_input` is removed

**Files:**
- Modify: `client/src/ui/hud/chat.rs`, `client/src/ui/hud/mod.rs` (system list), `client/src/ui/hud/ui_mode.rs`, `client/src/ui/button.rs` (drop the text-input click logic), `client/src/game.rs` (drop `TextInputPlugin`), `client/Cargo.toml` (drop `bevy_simple_text_input`), `client/src/e2e/locator.rs` (drop the `TextInputInner` case of `content_of`), `client/src/e2e/game.rs`, `docs/ARCHITECTURE.md`

**Interfaces:**
- Consumes: `spawn_text_field`, `Submit`, `Cancel`, `InputFocus`, `FocusLost`.
- Produces: `UIMode::Typing` ⇔ `InputFocus` holds an entity with `EditableText`. `ChatInput` marks the chat's `input` entity.

- [ ] **Step 1: Write the failing tests**

Unit test in `ui_mode.rs`: `typing_is_the_focus_on_an_editable_text` — app with `InputFocus` and `set_ui_mode`; focus on an `EditableText` entity → `Typing`; on another entity → `Closed`; cleared, with a visible `UiDialog` → `Opened`.

E2e in `e2e/game.rs`:
- `the_chat_sends_a_message_and_escape_closes_it` unchanged.
- new `typing_in_the_chat_keeps_the_game_keys_quiet` — `press(KeyT)`, `type_text("e")` → `InventoryRoot` not visible; `press(Escape)` → `ChatRoot` hidden and `L::text("Resume")` not visible (the pause menu did not open); `press(KeyE)` → inventory visible (the closed field holds no key).
- new `clicking_outside_the_chat_closes_it` — `press(KeyT)`, `type_text("draft")`, `click(&L::marker::<Hotbar>())` → `ChatRoot` hidden, `UIMode::Closed`; `press(KeyE)` → inventory visible.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::ui_mode e2e::game` → compile error, then the click test leaves the chat open.

- [ ] **Step 3: Implement**

- `setup_chat` spawns the field with `TextField { label: "Chat", placeholder: "Send a message...", max_characters: Some(256), font: load_font(&asset_server), font_size: CHAT_SIZE }` inside the `ChatInput` node's place; the chat root gets `TabGroup::modal()` so Tab stays in the field.
- `toggle_chat_system` opens the chat (on release of the key, as today) and sets `InputFocus` to the field; its Escape branch goes.
- `send_chat_message_system` becomes observers on the field: `Submit` sends a non-blank value; `Submit`, `Cancel` and `FocusLost` all close the chat through one function (hide the root, clear the text, clear `InputFocus` if it is the field). Escape keeps no draft.
- `set_ui_mode` reads `Res<InputFocus>` and `Query<(), With<EditableText>>`.
- Remove the dependency and every remaining use (`rg bevy_simple_text_input` and `rg TextInput` find nothing but `Role::TextInput`).
- ARCHITECTURE: a short "UI widgets" section — `ui/theme.rs` holds the tokens; `ui/widgets/` holds one file per widget plus `focus.rs` and `scale.rs`, added by `widgets_plugin` in `app::add_client`; widgets are headless `bevy::ui_widgets` / `EditableText` with our visuals, `TabIndex`, role and label; a text field reports `Submit` and `Cancel` after `EditableTextSystems`; `UIMode::Typing` is "the focus is on an `EditableText`"; screens still on `Interaction` buttons move over lot by lot.

- [ ] **Step 4: Run the tests and the suite** — `timeout 300 cargo test -q -p client -- hud::ui_mode e2e::game` → PASS; `timeout 900 cargo test --workspace --no-fail-fast` green; fmt and clippy clean; `Cargo.lock` no longer lists `bevy_simple_text_input`.

- [ ] **Step 5: Commit** — `git commit -m "refactor(client): the chat and the typing mode use the new text field, and bevy_simple_text_input is removed"`
