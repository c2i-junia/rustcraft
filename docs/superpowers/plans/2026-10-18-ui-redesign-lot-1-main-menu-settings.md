# UI Redesign Lot 1 (Main Menu and Settings) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the main menu (logo, stacked buttons, quit confirmation, a "Continue" card, version, key hints, on the darkened dirt background) and turn Settings into one tabbed screen (Controls, Audio, Interface) whose changes apply and save at once, reachable from the main menu and from the pause menu.

**Architecture:** Five new widgets in `client/src/ui/widgets/` (`dialog`, `hints`, `tabs`, `segmented`, `slider`), each a function spawning a small tree with its own observers, on the headless `bevy::ui_widgets` (`Button`, `RadioGroup`, `Slider`). The main menu and the settings screen only assemble widgets; what is not drawing (the last played world, "played … ago", key-map conflicts, the forced UI scale, loading and writing the settings files) lives in plain functions with unit tests. The settings screen is one `MenuState::Settings` in the menus and an overlay over the pause menu in the game, built by the same `spawn_settings`; its tabs are not states. Escape follows the Lot 0 rule: whatever consumes it (a dialog, a key capture, the settings closing) finishes in `PostUpdate`, so the Escape handlers in `Update` still see it open on that frame.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate, RadioGroup, RadioButton, Slider, SliderValue, SliderRange, SliderStep, SliderThumb, SetSliderValue, ValueChange, ScrollArea}`, `bevy::input_focus`, `bevy::ui::{BackgroundGradient, RadialGradient, NodeImageMode::Tiled}`), `accesskit` 0.24, RON 0.6, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md` — only "Lot 1 — Main menu and settings", the "Main menu" and "Settings" paragraphs of "Screens", decision 2 (background), decision 4 ("Continue"), and the forced scale of the "UiScale" paragraph; "Visual system", "Architecture", "Testing" and the anti-goals bind every task. Defects to fix: `docs/superpowers/specs/2026-10-18-ui-e2e-tests-design.md`, "Non-goals" (main menu, settings, controls, audio lines). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md` ("notes §n"). Conventions and products: the Lot 0 and Lot 5 plans next to this file.

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `fix(client): …`, `test(client): …`, `refactor(client): …`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::settings`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift, which runs no destructor while a panic unwinds: a failing test's harness is never dropped. Do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`. Widget probes live in `client/src/e2e/widgets.rs` (start from `main_menu()`, probes under a `GlobalZIndex(1)` root, driven only through `click`/`hover`/`press`/`type_text`/`scroll`). Menu tests go in `e2e/main_menu.rs` and `e2e/settings.rs`; every `enter_world` starts an integrated server, so **one journey per behaviour group**, chaining its checks.
- Never a bare `git stash`; set work aside with a WIP commit.
- `bevy::prelude::Button` is the legacy widget. Import the headless one as `use bevy::ui_widgets::Button as UiButton;`. New screens use no `Interaction`; a node that sits over legacy buttons (a dialog backdrop, the in-game settings overlay) also carries `FocusPolicy::Block`.
- Put the widget component or the explicit `AccessibilityNode(accesskit::Node::new(Role::…))` **before** `AccessibleLabel` in a bundle (notes §5).
- A widget no screen uses yet allows `dead_code` on the item with a comment naming its first user; the allowance goes in the task that uses it. Never crate-wide. Task 3 removes the Lot 0 allowances on `ButtonVariant` and `button`.
- **Escape.** A dialog, a key capture and the settings screen consume Escape through `FocusedInput<KeyboardInput>` (or `Activate`) and finish in `PostUpdate`; the Escape handlers that read `ButtonInput<KeyCode>` in `Update` (`escape_button`, the main menu's, `settings_escape`, `render_pause_menu`) skip while `dialog_open`, `text_field_focused` (the focus is in an `EditableText`, which takes Escape for itself), `KeyCapture::is_active()` or (for the pause menu) `settings_open` holds.
- **Settings files** (game folder, written at once by a plain `write_*` function after each change, never "on leaving"; a missing or unreadable file gives the defaults; an older file always loads, proved by a test reading literal old content):
  - `keybindings.ron` — format unchanged, `(map: { Action: [Key, …] })`. On load, each action keeps its first two keys (primary, secondary) and `Escape` is always `[Escape]`.
  - `audio.ron` — format unchanged, `(master: f32, effects: f32, interface: f32)` in `0..=1`, `#[serde(default)]`.
  - `interface.ron` — new, `(ui_scale: Auto)` with `Auto | X1 | X1_5 | X2 | X3`, `#[serde(default)]`.
- **Fonts.** The RustCraft font has no `← ↑ → ↓` (U+2190–2193), `…`, `×` or `−`: write `...`, `x`, `-`. Arrows only appear in key caps, drawn in Fira Mono (`MONO_FONT_PATH`), which has them.
- **Pixel art.** The title image is shown at its own size (601×94 logical px); the dirt background is tiled at a whole number of physical px per texel (`NodeImageMode::Tiled`, `stretch_value = BACKGROUND_TEXEL_PX × UiScale`). A node placed from `Window::cursor_position()` or `world_to_viewport` divides by `UiScale`.
- **Lots 2 and 3 own** `solo.rs`, `multi.rs`, `list_screen.rs` and their legacy buttons, `ScrollingList`, `mouse_scroll`, `button_system`, `escape_button` (Solo and Multi arms) and `MenuButtonAction::BackToMainMenu`: they stay. This lot touches `solo.rs` only to share `start_world`.
- **Goldens.** `menu_main` changes in Tasks 3 and 4; `menu_settings` and `menu_audio` in Task 6; `menu_controls` in Task 6 (interim) and Task 7. Each is regenerated with `UPDATE_GOLDEN=1` only after reading the `.actual.png`. Goldens hold nothing time-dependent (the "played … ago" world is seeded with a fixed age; no caret, no ring, no hover: move the cursor to a corner with `hover_at` before the shot). Any other golden that changes is a bug.

## Review Focus

1. Escape that closes the quit dialog, or cancels a key capture, must not also reopen the dialog, leave the settings, or close the pause menu on the same frame. (Task 3: `escape_asks_before_quitting_with_cancel_focused`; Task 7: `the_keyboard_alone_rebinds_cancels_and_clears`; Task 9: `escape_with_the_settings_open_leaves_the_pause_menu_open`)
2. A forced UI scale that does not fit the window is clamped, and re-clamped when the window shrinks after the choice: 3 at 1920×1080 gives 1.5, then 1 at 800×600. (Task 1: `a_forced_scale_keeps_the_logical_height_at_least_600`; Task 6: `the_ui_scale_is_chosen_clamped_and_saved`)
3. Ctrl+Enter while Singleplayer has the focus plays the last world and does not open the world list. (Task 4: `continue_plays_the_last_world_by_ctrl_enter_and_by_click`)
4. Capturing a game key (E) in the in-game settings binds it without opening the inventory, and clicks on the overlay never reach the legacy pause buttons beneath. (Task 9: the pause journey)
5. An old `keybindings.ron` with three keys for an action, or with Escape rebound, still loads. (Task 1: `an_old_keybindings_file_still_loads`, `escape_always_pauses`)
6. A click on a dialog's backdrop closes nothing and keeps the focus inside the dialog. (Task 2: `a_dialog_focuses_its_chosen_button_and_keeps_the_focus_inside`)

## Defects of the e2e spec fixed here

Escape on the main menu quits at once (Task 3); no Tab or keyboard activation (Tasks 3, 6); the controls screen cannot be left with the keyboard (Task 7); no hover feedback on the settings buttons, the controls' Back and Cancel, pressed look shows the normal image (Tasks 3, 6, 7); the wheel scrolls every list at once (Tasks 5, 7, for the settings); the same key bound twice silently, the recorder cannot be cancelled, arrow keys show as empty labels, actions named by their code name (Tasks 7, 8).

## Files

- `client/src/ui/settings.rs` (new), `client/src/ui/widgets/scale.rs`, `client/src/input/{data,keyboard}.rs`, `client/src/audio/{mod,settings}.rs`, `client/src/constants.rs`, `client/src/app.rs` — the settings files and the model (Tasks 1, 7, 8).
- `client/src/ui/widgets/{dialog,hints,tabs,segmented,slider}.rs` (new) — the widgets (Tasks 2, 5).
- `client/src/ui/menus/{home,mod,background,worlds}.rs` (`background`, `worlds` new), `client/src/ui/button.rs`, `client/src/ui/style.rs`, `server/src/lib.rs` — the main menu (Tasks 3, 4).
- `client/src/ui/menus/settings/{mod,controls,audio,interface}.rs` (`interface` new, `menu.rs` deleted), `client/src/ui/menus/pause.rs`, `client/src/ui/hud/mod.rs` — the settings (Tasks 6–9).
- `client/src/e2e/{main_menu,settings,widgets,capture,helpers,seed,game,review}.rs`, `client/tests/golden/menu_{main,settings,controls,audio}.png`, `README.md`, `docs/ARCHITECTURE.md`.

---

### Task 1: The settings files — interface scale, key bindings and volumes

**Files:**
- Create: `client/src/ui/settings.rs`
- Modify: `client/src/ui/mod.rs`, `client/src/ui/widgets/scale.rs`, `client/src/ui/widgets/mod.rs` (`init_resource::<InterfaceSettings>()`), `client/src/app.rs`, `client/src/constants.rs`, `client/src/input/keyboard.rs`, `client/src/audio/settings.rs`, `client/src/e2e/seed.rs`, `client/src/e2e/widgets.rs`

**Interfaces:**
- Produces, in `ui/settings.rs`:
  ```rust
  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
  pub enum UiScaleChoice { #[default] Auto, X1, X1_5, X2, X3 }
  impl UiScaleChoice {
      pub const ALL: [UiScaleChoice; 5];
      pub fn label(self) -> &'static str;      // "Auto", "1", "1.5", "2", "3"
      pub fn factor(self) -> Option<f32>;      // None for Auto
  }
  #[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)] #[serde(default)]
  pub struct InterfaceSettings { pub ui_scale: UiScaleChoice }
  pub fn load_interface_settings(paths: &GameFolderPaths) -> InterfaceSettings;
  pub fn write_interface_settings(settings: &InterfaceSettings, paths: &GameFolderPaths);
  ```
  `INTERFACE_SETTINGS_PATH = "interface.ron"` in `constants.rs`; `add_client` inserts `load_interface_settings(&game_folder_paths)`.
- Produces, in `scale.rs`: `pub const UI_SCALE_STEPS: [f32; 4] = [1.0, 1.5, 2.0, 3.0];`, `pub const MIN_LOGICAL_HEIGHT: f32 = 600.0;`, `pub fn ui_scale_for(choice: UiScaleChoice, window_height: f32) -> f32` — `Auto` is `auto_ui_scale`; a forced factor becomes the largest step ≤ it whose logical height `window_height / step` is ≥ 600, never under 1. `update_ui_scale` reads `Res<InterfaceSettings>`.
- Produces, in `input/keyboard.rs`: `pub const MAX_KEYS_PER_ACTION: usize = 2;`, `pub fn default_bindings() -> KeyMap` (the map now built inline in `get_bindings`), `pub fn write_keybindings(key_map: &KeyMap, paths: &GameFolderPaths)` (the body of `save_keybindings`, which now calls it). `get_bindings` keeps the first two keys of each saved action, drops repeated keys within an action, and forces `Escape → [Escape]`.
- Produces, in `audio/settings.rs`: `pub fn set(&mut self, channel: Channel, volume: f32)` (clamped to `0..=1`), `pub fn write_audio_settings(settings: &AudioSettings, paths: &GameFolderPaths)` (the body of `save_audio_settings`, which calls it until Task 6).
- Produces, in `e2e/seed.rs`: `GameFolder::with_file(self, name: &str, contents: &str) -> Self`.

- [ ] **Step 1: Write the failing tests**

Unit, `ui/settings.rs`: `the_interface_settings_are_saved_and_loaded_back` (`X1_5` round-trips through a temp game folder); `a_missing_or_unreadable_interface_file_gives_auto` — no file → `Auto`; `"garbage"` → `Auto`; `"()"` → `Auto`; `"(ui_scale: X2)"` → `X2`.

Unit, `scale.rs`:
```rust
fn a_forced_scale_keeps_the_logical_height_at_least_600() {
    for (choice, height, scale) in [(X3, 2160., 3.), (X3, 1440., 2.), (X3, 1080., 1.5), (X2, 1080., 1.5),
        (X2, 1200., 2.), (X1_5, 900., 1.5), (X1_5, 720., 1.), (X1, 2160., 1.), (X3, 500., 1.), (Auto, 1080., 1.5)] {
        assert_eq!(ui_scale_for(choice, height), scale, "{choice:?} at {height}");
    }
    // and for every choice and every height in 600..=2400:
    //   UI_SCALE_STEPS.contains(&s) && height / s >= MIN_LOGICAL_HEIGHT
}
```

Unit, `keyboard.rs` (temp game folder as in the existing test):
- `an_old_keybindings_file_still_loads` — the literal content `save_keybindings` wrote before this lot, with its `/*[0]*/` comments, holding `Jump: [KeyJ]` and `MoveForward: [KeyW, ArrowUp, KeyZ]` → `Jump == [KeyJ]`, `MoveForward == [KeyW, ArrowUp]`, `Escape == [Escape]`, every other action as in `default_bindings()`.
- `escape_always_pauses` — a file with `Escape: [KeyP]` → `[Escape]`.
- `an_unreadable_keybindings_file_gives_the_defaults` — `"garbage"` → `default_bindings()`.

Unit, `audio/settings.rs`: `setting_a_volume_clamps_it` (`1.4 → 1.0`, `-0.1 → 0.0`, `0.55 → 0.55`); `an_old_audio_file_still_loads` (`"(master: 0.7)"` → master 0.7, the others 1.0); the existing save/load test goes through `write_audio_settings`.

E2e, `e2e/widgets.rs`: `a_forced_ui_scale_is_read_from_the_game_folder` — `GameFolder::new().with_file("interface.ron", "(ui_scale: X1)")`, 1920×1080 → `UiScale == 1.0`.

- [ ] **Step 2: Run them to verify they fail**

Run: `timeout 300 cargo test -q -p client -- ui::settings ui::widgets::scale input::keyboard audio::settings e2e::widgets::a_forced`
Expected: compile errors, then `UiScale` 1.5 and three keys kept for `MoveForward`.

- [ ] **Step 3: Implement** as in Interfaces. `load_interface_settings` and the writers follow `audio/settings.rs` (pretty RON, `error!` on failure, nothing else).

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green; no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): an interface settings file with a forced UI scale, and settings files that keep loading"`

---

### Task 2: The dialog and key-hint widgets

**Files:**
- Create: `client/src/ui/widgets/dialog.rs`, `client/src/ui/widgets/hints.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/e2e/widgets.rs`, `client/src/e2e/helpers.rs`

**Interfaces:**
- Consumes: `button`, `ButtonVariant`, `theme::*`, `InputFocus`, `TabGroup`, `AutoFocus`.
- Produces, in `dialog.rs`:
  ```rust
  pub const DIALOG_Z: i32 = 50;
  pub struct Dialog<'a> {
      pub title: &'a str,
      pub body: &'a str,                              // "" shows no body
      pub actions: &'a [(&'a str, ButtonVariant)],    // left to right; the primary action last
      pub focused: usize,                             // the button focused on open, by convention Cancel
  }
  #[derive(Component)] pub struct DialogRoot { restore_focus: Option<Entity> }
  /// Triggered on the root when the dialog closes: `Some(i)` for the action `i`, `None` for Escape.
  #[derive(EntityEvent)] pub struct DialogClosed { pub entity: Entity, pub action: Option<usize> }
  pub fn spawn_dialog(commands: &mut Commands, dialog: &Dialog, font: Handle<Font>, focus: &InputFocus) -> Entity;
  pub fn dialog_open(dialogs: Query<(), With<DialogRoot>>) -> bool;
  ```
  Tree: root (absolute, full window, `GlobalZIndex(DIALOG_Z)`, `TabGroup::modal()`, `AccessibilityNode(Role::Dialog)` then `AccessibleLabel(title)`, centred) → [backdrop (absolute fill, `BackgroundColor(BEVEL_LO.with_alpha(0.6))`, `TabIndex(-1)`, `FocusPolicy::Block`), panel (`PANEL`, raised bevel, padding `SPACE_XL`, `row_gap: SPACE_L`, `min_width: 360`, `max_width: 90%`) → [title `FONT_L` `TEXT`, body `FONT_M` `MUTED`, actions row (`justify_content: FlexEnd`, gap `SPACE_S`) of `button(..)`s, `AutoFocus` on `focused`]]. An action's `Activate`, or Escape on the root (`FocusedInput<KeyboardInput>`, pressed, not repeat, `propagate(false)`), records the close; a `PostUpdate` system triggers `DialogClosed`, despawns the root and puts the focus back on `restore_focus` if it still exists.
- Produces, in `hints.rs`:
  ```rust
  pub struct Hint { pub keys: &'static str, pub action: &'static str }
  #[derive(Component)] pub struct HintBar;
  pub fn hint_bar(hints: &[Hint], font: Handle<Font>, glyph_font: Handle<Font>) -> impl Bundle;
  pub fn key_cap(keys: &str, font: Handle<Font>, glyph_font: Handle<Font>) -> impl Bundle;
  fn uses_glyph_font(keys: &str) -> bool;   // any char in U+2190..=U+2193
  ```
  `hint_bar`: absolute, `bottom/left/right: 0`, row, `column_gap: SPACE_L`, padding `SPACE_S` × `SPACE_L`, `PANEL` background, `Pickable::IGNORE`; each hint is a row (gap `SPACE_XS`) of `key_cap(keys)` and the action in `FONT_S`, `MUTED`. `key_cap`: `SLOT` background, sunken bevel, padding 2 × `SPACE_XS`, the keys in `FONT_S`, `TEXT`, in `glyph_font` when `uses_glyph_font`.
- Produces, in `e2e/helpers.rs`: `Harness::focus_label(&mut self) -> Option<String>` (the `AccessibilityNode` label of the focused entity); `Harness::bright_pixels_in(&mut self, locator: &L) -> usize` (pixels of a fresh screenshot inside the node's physical rect whose luminance is above 128).

- [ ] **Step 1: Write the failing tests**

Unit, `dialog.rs`: `a_dialog_has_the_dialog_role_and_its_title` (spawn in a bare `World` via a `Commands` queue; root `Role::Dialog`, label `"Quit Rustcraft?"`).
Unit, `hints.rs`: `arrow_keys_use_the_glyph_font` (`"←→"`, `"↑"` true; `"Enter"`, `"Ctrl+Enter"` false); `a_hint_bar_lists_its_hints` (two hints → texts `"Enter"`, `"Select"`, `"Esc"`, `"Quit"` under the `HintBar`).

E2e, `e2e/widgets.rs` (a probe `button("Before")` with `AutoFocus`, then a dialog `{ title: "Probe?", body: "Sure?", actions: [("Cancel", Default), ("Quit", Danger)], focused: 0 }` spawned from a system with the current `InputFocus`, and an observer recording `DialogClosed` actions):
- `a_dialog_focuses_its_chosen_button_and_keeps_the_focus_inside` — `focus_label() == Some("Cancel")`; `Tab` ×4 → the focus stays a descendant of the root; `click_at((5, 5))` (the backdrop) → the dialog is still there, the focus still inside, no close recorded.
- `escape_closes_a_dialog_and_gives_the_focus_back` — `press(Escape)` → closes `[None]`, no `DialogRoot`, `focus_label() == Some("Before")`.
- `a_dialog_button_closes_it_with_its_action` — `click(&L::role(Role::Button, "Quit"))` → `[Some(1)]`; a second dialog, `press(Enter)` → `[Some(1), Some(0)]`.
- `a_key_cap_draws_arrow_glyphs` — probes `key_cap("←", ..)` and `key_cap("Enter", ..)` with `load_font` and `MONO_FONT_PATH` → `bright_pixels_in(&L::text("←")) > 20`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::widgets::dialog ui::widgets::hints e2e::widgets` → compile errors.

- [ ] **Step 3: Implement** the two files and add their plugins to `widgets_plugin`. `spawn_dialog`, `hint_bar`, `key_cap`, `Hint`: `#[allow(dead_code)]` with "first used by the main menu (Lot 1, Task 3)".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): dialog and key-hint widgets"`

---

### Task 3: The main menu — layout, background, quit confirmation, version and hints

**Files:**
- Create: `client/src/ui/menus/background.rs`
- Modify: `client/src/ui/menus/home.rs` (rewrite), `client/src/ui/menus/mod.rs`, `client/src/ui/button.rs` (`MenuButtonAction::{Solo, Multi, Settings, Quit}` go), `client/src/ui/widgets/{button,dialog,hints}.rs` (allowances go), `client/src/audio/mod.rs`, `client/src/e2e/main_menu.rs`, `client/src/e2e/capture.rs`, `client/src/e2e/widgets.rs`, `client/tests/golden/menu_main.png`

**Interfaces:**
- Consumes: `button`, `ButtonVariant`, `spawn_dialog`, `Dialog`, `DialogClosed`, `dialog_open`, `hint_bar`, `Hint`, `ArrowNav`, `load_title_image`, `load_background_image`.
- Produces, in `background.rs`: `pub const BACKGROUND_TEXEL_PX: f32 = 4.0;` `pub const DIRT_TINT: Color` (the texture darkened to about 45 %); `pub fn menu_background(assets: &AssetServer) -> impl Bundle` (full-window node, `ImageNode` of the dirt texture with `color: DIRT_TINT` and `NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: BACKGROUND_TEXEL_PX }`, and a full-window child with a `BackgroundGradient` `RadialGradient` from `Color::NONE` at 45 % to `BEVEL_LO` at 85 % alpha at 100 %, `Pickable::IGNORE`); `pub fn scale_menu_background(...)` (on a change of `UiScale`, `stretch_value = BACKGROUND_TEXEL_PX * ui_scale`).
- Produces, in `home.rs`: `#[derive(Component, Clone, Copy)] pub enum MainMenuButton { Singleplayer, Multiplayer, Settings, Quit }`; `pub const QUIT_TITLE: &str = "Quit Rustcraft?";` `pub const MAIN_HINTS: [Hint; 3]` = `Tab` Move, `Enter` Select, `Esc` Quit; `pub fn open_quit_dialog_on_escape(...)` (`Update`, `in_state(MenuState::Main)` and `not(dialog_open)`, Escape just pressed); `fn ctrl_held(keys: &ButtonInput<KeyCode>) -> bool`.
  Layout: root (`menu_background`, `DespawnOnExit(MenuState::Main)`, `TabGroup::new(0)`, padding `SPACE_XXL`, column) → [logo 601×94 at top-left, a row → [left column (`ArrowNav`, column, `row_gap: SPACE_S`, `width: 280`) of `button("Singleplayer")` (`AutoFocus`), `"Multiplayer"`, `"Settings"`, then `margin_top: SPACE_XL` `button("Quit game", Ghost)`, right side kept empty for Task 4], version `Text(format!("v{}", env!("CARGO_PKG_VERSION")))` absolute top-right, `FONT_S`, `MUTED`, `hint_bar(MAIN_HINTS)`]. One observer for `Activate` on `MainMenuButton` (does nothing while `ctrl_held`): Singleplayer / Multiplayer / Settings set their `MenuState`; Quit opens the quit dialog `{ QUIT_TITLE, body: "", actions: [("Cancel", Default), ("Quit", Danger)], focused: 0 }`, whose `DialogClosed { action: Some(1) }` writes `AppExit::Success`.
- `escape_button` keeps only the `Solo | Multi → Main` arm (and the settings arms until Task 6); `menu_action` keeps the variants still spawned.
- Produces, in `audio/mod.rs`: a global observer `click_on_activate(_: On<Activate>, MessageWriter<PlaySound>)` — the themed buttons click like the legacy ones did.
- `e2e/widgets.rs`: `spawn_group` uses `TabGroup::modal()`, so Tab stays among the probes now that the menu itself has focusable buttons (the journeys of those tests are unchanged).

- [ ] **Step 1: Write the failing tests**

Unit, `audio/mod.rs`: `activating_a_widget_button_clicks` — app with `add_message::<PlaySound>()` and the observer; `trigger(Activate { entity })` → one `PlaySound` of `Sound::Click`.

E2e, `e2e/main_menu.rs`:
- `singleplayer_opens_the_world_list`, `multiplayer_opens_the_server_list`, `settings_opens_the_settings`, `two_harnesses_run_side_by_side`: unchanged.
- `quit_exits_the_game` — click `"Quit game"`; wait for `QUIT_TITLE`; `!has_exited()`; click `L::role(Role::Button, "Quit")` → `has_exited()`.
- `escape_asks_before_quitting_with_cancel_focused` — Escape → `QUIT_TITLE` visible, `focus_label() == Some("Cancel")`, not exited; Escape → no dialog (also after 2 more frames: it was not reopened), not exited, `focus_label() == Some("Singleplayer")`; Escape, Enter → closed, not exited; Escape, Tab, Enter → exited.
- `the_keyboard_alone_opens_a_screen` — `focus_label() == Some("Singleplayer")` with no ring (`Outline.color == NONE`); ArrowDown → `"Multiplayer"`; Tab → `"Settings"`, its ring `ACCENT`; Enter → `MenuState::Settings`.
- `hovering_a_button_darkens_it` becomes `hovering_a_button_lightens_it` — `hover(&L::role(Role::Button, "Singleplayer"))` → its `BackgroundColor(RAISE_HOVER)`.
- `the_main_menu_shows_the_version_and_the_key_hints` — `L::text("v0.0.0")` (from `env!`) visible, its centre right of 1280 × 3/4 and above 100; `L::text("Esc").within(L::marker::<HintBar>())` visible.

E2e, `e2e/capture.rs`: `resizing_lays_the_menu_out_again` reads `"Quit game"`: after `resize(800, 600)` its centre is inside the window and `x < 400` (left column). `the_main_menu_matches_its_golden` unchanged (empty folder: no card yet).

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- audio e2e::main_menu e2e::capture` → Escape exits, no "Quit game".

- [ ] **Step 3: Implement** as in Interfaces; `scale_menu_background` joins the menu systems. Remove what this task orphans (the legacy main-menu imports and variants); `style.rs` and the stone-button assets stay while Settings and Lots 2/3 use them.

- [ ] **Step 4: Run the tests; regenerate `menu_main` on purpose**

Read `client/tests/golden/menu_main.actual.png`: dark dirt tiled crisply (whole texels), a vignette, the logo top-left at its own size, three raised buttons and a ghost "Quit game" stacked on the left, the version top-right, the hint bar at the bottom, no ring. Then `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_main_menu_matches_its_golden`, rerun without it → PASS; no `.actual.png` / `.diff.png` left. Full suite green (`e2e::widgets` included).

- [ ] **Step 5: Commit** (the report lists the changed locators of `e2e/main_menu.rs`, `e2e/capture.rs` and the probe group of `e2e/widgets.rs`) — `git commit -m "feat(client): a new main menu that asks before quitting, on the darkened dirt background"`

---

### Task 4: The "Continue" card

**Files:**
- Create: `client/src/ui/menus/worlds.rs`
- Modify: `server/src/lib.rs` (`pub use world::format::{WorldMeta, WORLD_FILE};`), `client/src/ui/menus/home.rs`, `client/src/ui/menus/mod.rs`, `client/src/ui/menus/solo.rs` (the `Load` branch calls `start_world`), `client/src/e2e/seed.rs`, `client/src/e2e/main_menu.rs`, `client/src/e2e/capture.rs`, `client/tests/golden/menu_main.png`

**Interfaces:**
- Produces, in `worlds.rs` (plain functions, unit-tested):
  ```rust
  pub struct WorldSummary { pub name: String, pub seed: Option<u32>, pub day: Option<u64>, pub played: SystemTime }
  /// The world whose `world.ron` was modified last. `name` is the folder name (what Play loads);
  /// seed and day come from `world.ron`, and are `None` when it cannot be read.
  pub fn last_played_world(saves: &Path) -> Option<WorldSummary>;
  pub fn day_of(time: u64) -> u64;                 // time / DAY_DURATION_IN_TICKS + 1
  pub fn played_ago(elapsed: Duration) -> String;
  pub fn thumbnail_color(seed: Option<u32>) -> Color;   // one of six earthy colours by seed % 6; RAISE for None
  pub fn start_world(name: &str, selected: &mut SelectedWorld,
                     game: &mut NextState<GameState>, menu: &mut NextState<MenuState>);
  ```
- Produces, in `home.rs`: `#[derive(Component)] pub struct ContinueCard { pub world: String }`; the card on the right of the row, only when `last_played_world` finds one: `"Continue"` (`FONT_S`, `MUTED`) above a panel (`PANEL`, raised bevel, padding `SPACE_L`, `width: 300`) → [row → [48×48 swatch `thumbnail_color`, sunken bevel; column → [name `FONT_L`, `"Day {n}"` (absent when `None`), `played_ago(..)` `FONT_S` `MUTED`]], `button("Play", Primary)`]. `pub fn continue_on_ctrl_enter(...)` (`Update`, `in_state(MenuState::Main)`, `not(dialog_open)`; Enter just pressed with Control held, and a card). `pub const CONTINUE_HINT: Hint` = `Ctrl+Enter` Continue, added to the hint bar only with a card.
- Produces, in `e2e/seed.rs`: `GameFolder::with_played_world(self, name: &str, time: u64, played_ago: Duration) -> Self` (a `world.ron` like `with_world` with this `time`, its mtime set to `now - played_ago` with `File::set_modified`).

- [ ] **Step 1: Write the failing tests**

Unit, `worlds.rs` (temp folders as in `solo.rs`; `world.ron` written with `ron::ser::to_string_pretty(&WorldMeta::new(..))`):
- `the_last_played_world_is_the_newest_world_ron` — "Old" 5 days ago, "Recent" 2 hours ago, a newer folder without `world.ron` → "Recent".
- `the_day_and_seed_come_from_world_ron` — `WorldSeed(7)`, `time = 3 * DAY_DURATION_IN_TICKS + 5` → `seed == Some(7)`, `day == Some(4)`.
- `an_unreadable_world_ron_still_counts_without_a_day` — empty `world.ron` → `Some`, `day == None`.
- `no_saves_folder_means_no_world` → `None`.
- `played_ago_reads_like_a_person_would_say_it` — `0 s`, `59 s` → `"played just now"`; `60 s` → `"played 1 minute ago"`; `5 min` → `"played 5 minutes ago"`; `1 h` → `"played 1 hour ago"`; `25 h` → `"played 1 day ago"`; `3 d 1 h` → `"played 3 days ago"`.
- `the_thumbnail_colour_follows_the_seed` — same seed, same colour; seeds 0..6 give six colours; `None` → `RAISE`.

E2e, `e2e/main_menu.rs`:
- `continue_shows_the_last_played_world` — `with_played_world("Old", 0, 5 days)`, `with_played_world("Recent", 3 * DAY_DURATION_IN_TICKS + 10, 2 h)` → within `L::marker::<ContinueCard>()`: `"Recent"`, `"Day 4"`, `"played 2 hours ago"`; `"Old"` not visible; `L::text("Ctrl+Enter")` visible.
- `without_a_world_there_is_no_continue_card` — empty folder → no `ContinueCard`, no `"Ctrl+Enter"`.
- `continue_plays_the_last_world_by_ctrl_enter_and_by_click` (one server journey) — folder with "Recent"; `focus_label() == Some("Singleplayer")`; `key_down(ControlLeft)`, `press(Enter)`, `key_up(ControlLeft)` → `SelectedWorld.name == Some("Recent")`, `menu_state() != MenuState::Solo`; wait for `GameState::Game`; Escape, click `"Back to menu"`, wait for the main menu → the card shows `"played just now"`; click `L::role(Role::Button, "Play")` → `GameState::Game`.

E2e, `e2e/capture.rs`: `the_main_menu_matches_its_golden` starts from `GameFolder::new().with_played_world("Survie", 0, 3 days + 1 h)`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- menus::worlds e2e::main_menu` → compile errors, then no card.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests; regenerate `menu_main` on purpose** — read the `.actual.png`: the card on the right with the swatch, "Survie", "Day 1", "played 3 days ago", a green Play; the `Ctrl+Enter` hint; nothing else moved. `UPDATE_GOLDEN=1 …`, rerun → PASS, no leftovers. Full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a Continue card on the main menu for the last played world"`

---

### Task 5: The tabs, segmented control and slider widgets

**Files:**
- Create: `client/src/ui/widgets/tabs.rs`, `client/src/ui/widgets/segmented.rs`, `client/src/ui/widgets/slider.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/e2e/widgets.rs`

**Interfaces:**
- Consumes: `theme::*`, `FocusRing`.
- Produces, in `segmented.rs` and `tabs.rs` (both on `RadioGroup` / `RadioButton`; the group carries `TabIndex(0)` and `FocusRing` and owns the arrows, the options have no `TabIndex`, so a click focuses the group; `Checked` is kept by our own observer of `ValueChange<Entity>` on the group, which also triggers the event below; the two files may share that wiring through a `pub(super)` helper):
  ```rust
  pub struct Segmented<'a> { pub label: &'a str, pub options: &'a [&'a str], pub selected: usize }
  #[derive(Component)] pub struct Segment(pub usize);
  #[derive(EntityEvent)] pub struct SegmentChange { pub entity: Entity, pub index: usize }
  pub fn spawn_segmented(commands: &mut Commands, segmented: &Segmented, font: Handle<Font>) -> Entity;

  pub struct Tabs<'a> { pub label: &'a str, pub tabs: &'a [&'a str], pub selected: usize }
  #[derive(Component)] pub struct TabButton(pub usize);
  #[derive(EntityEvent)] pub struct TabChange { pub entity: Entity, pub index: usize }
  pub fn spawn_tabs(commands: &mut Commands, tabs: &Tabs, font: Handle<Font>) -> Entity;
  ```
  Roles: segmented `RadioGroup` / `RadioButton` (from the widgets) with `AccessibleLabel`s; tabs override them with `AccessibilityNode(Role::TabList)` / `(Role::Tab)`. Looks: a segment is `ACCENT` with `PANEL` text when checked, `RAISE_HOVER` when hovered, else no fill and `TEXT`, inside a sunken `SLOT` frame; a tab is `RAISE` with `MUTED` text and a raised bevel, `RAISE_HOVER` when hovered, `PANEL` with `TEXT` and a 2 px `ACCENT` bottom border when checked.
- Produces, in `slider.rs`:
  ```rust
  pub const SLIDER_STEP: f32 = 0.05;
  pub struct SliderField<'a> { pub label: &'a str, pub value: f32 }   // 0..=1
  pub struct SliderParts { pub root: Entity, pub slider: Entity }
  #[derive(Component)] pub struct SliderReadout;
  pub fn spawn_slider(commands: &mut Commands, field: &SliderField, font: Handle<Font>) -> SliderParts;
  fn percent_label(value: f32) -> String;   // rounded: "55%"
  ```
  Row `root` → [label `FONT_M` (`width: 180`), `slider` = (`Slider { track_click: TrackClick::Snap, .. }`, `SliderValue`, `SliderRange::new(0., 1.)`, `SliderStep(SLIDER_STEP)`, `TabIndex(0)`, `FocusRing`, `Hovered`, `AccessibleLabel(label)`, 240×20, sunken bevel on `SLOT`) → [fill (`ACCENT`, absolute, height 100 %), thumb (`SliderThumb`, `RAISE`, raised bevel, 12 px wide, absolute)], readout (`SliderReadout`, `FONT_M`, `width: 64`, right-aligned)]. Observers on `slider`: `slider_self_update`; `On<Pointer<Scroll>>` → `SetSliderValue { change: RelativeStep(y.signum()) }` (up raises) and `propagate(false)`. A `PostUpdate` system places fill, thumb and readout on `Changed<SliderValue>`. Callers observe `ValueChange<f32>` on `slider`.

- [ ] **Step 1: Write the failing tests**

Unit: `slider.rs` `a_percentage_is_rounded` (`0.549999 → "55%"`, `0. → "0%"`, `1. → "100%"`); `tabs.rs` `the_tab_list_and_its_tabs_have_their_roles`; `segmented.rs` `a_segmented_control_is_a_radio_group_of_radio_buttons`.

E2e, `e2e/widgets.rs`:
- `a_click_the_arrows_and_the_wheel_move_a_slider` — probe slider "Volume" at 0.2: `click(&L::role(Role::Slider, "Volume"))` → readout `"50%"` (Snap at the centre), `InputFocus` on the slider; ArrowRight → `"55%"`; ArrowLeft ×2 → `"45%"`; `scroll(1.)` → `"50%"`; `scroll(-1.)` → `"45%"`; End → `"100%"`; ArrowRight → still `"100%"`; the fill's width equals the track's inner width.
- `the_wheel_moves_only_the_slider_under_the_cursor` — sliders A and B at 0.5; hover A, `scroll(1.)` → A `"55%"`, B `"50%"`.
- `a_segment_is_chosen_by_click_and_by_arrows` — options Auto, 1, 1.5, 2, 3, selected 0: click `"2"` → changes `[3]`, only `"2"` has `Checked`, its background `ACCENT`; ArrowLeft → `[3, 2]`; hover `"3"` → `RAISE_HOVER`.
- `tabs_switch_by_click_and_by_arrows` — tabs A, B, C: click B → `[1]`; ArrowRight → `[1, 2]`; ArrowRight → `[1, 2, 0]` (wraps); hovering an unchecked tab → `RAISE_HOVER`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::widgets e2e::widgets` → compile errors.

- [ ] **Step 3: Implement** the three files and their plugins; `dead_code` allowances name "the settings screen (Lot 1, Task 6)".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): tabs, segmented control and slider widgets"`

---

### Task 6: One settings screen — tabs, Audio and Interface

**Files:**
- Create: `client/src/ui/menus/settings/interface.rs`
- Modify: `client/src/ui/menus/settings/mod.rs` (the screen), `client/src/ui/menus/settings/audio.rs` (rewrite), `client/src/ui/menus/settings/controls.rs` (re-parented, see Step 3), `client/src/ui/menus/mod.rs`, `client/src/ui/button.rs` (`MenuState::{SettingsControls, SettingsAudio}` and `MenuButtonAction::{SettingsControls, SettingsAudio, BackToSettings}` go), `client/src/ui/style.rs` (drop what is orphaned: `big_button_style`, `background_image_style`, `BUTTON_TEXT_COLOR`), `client/src/audio/settings.rs` (`step`, `VOLUME_STEP` and `save_audio_settings` go), `client/src/e2e/settings.rs`, `client/tests/golden/menu_{settings,audio,controls}.png`
- Delete: `client/src/ui/menus/settings/menu.rs`

**Interfaces:**
- Consumes: `spawn_tabs`, `TabChange`, `spawn_slider`, `spawn_segmented`, `SegmentChange`, `button`, `hint_bar`, `menu_background`, `dialog_open`, `ui_scale_for`, `InterfaceSettings`, `write_interface_settings`, `AudioSettings::set`, `write_audio_settings`.
- Produces, in `settings/mod.rs`:
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum SettingsContext { Menu, Game }
  #[derive(Component)] pub struct SettingsRoot { pub context: SettingsContext }
  #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)] pub enum SettingsTab { #[default] Controls, Audio, Interface }
  #[derive(Component)] pub struct SettingsPanel;          // the content of the current tab, respawned on TabChange
  #[derive(Component)] struct CloseSettings;               // a close request, applied in PostUpdate
  pub const SETTINGS_HINTS: [Hint; 4];                     // Tab Move, ←→ Change, Enter Select, Esc Back
  pub fn spawn_settings(commands: &mut Commands, assets: &AssetServer, context: SettingsContext) -> Entity;
  pub fn settings_open(roots: Query<(), With<SettingsRoot>>) -> bool;
  pub fn settings_plugin(app: &mut App);                   // added by menu_plugin
  ```
  Root: `Menu` → `menu_background`, `DespawnOnExit(MenuState::Settings)`, `TabGroup::new(0)`; (`Game` is completed in Task 9). Centred panel (`width: 720`, `max_width: 100%`, `PANEL`, raised bevel, padding `SPACE_L`, column, `row_gap: SPACE_M`, bottom margin clear of the hint bar) → [`"Settings"` `FONT_XL`, `spawn_tabs(Tabs { label: "Settings", tabs: ["Controls", "Audio", "Interface"], selected: 0 })` with `AutoFocus`, `SettingsPanel` (`flex_grow: 1`, min height 0), footer row → `button("Back")`], `hint_bar(SETTINGS_HINTS)`. Back's `Activate` and `settings_escape` (`Update`, `run_if(settings_open)`, skips while `dialog_open` or the hosted recorder of Step 3 is visible) insert `CloseSettings`; a `PostUpdate` system applies it: `Menu` → `NextState(MenuState::Main)`.
- Produces, in `audio.rs`: `#[derive(Component)] pub struct VolumeSlider(pub Channel)`; `pub fn spawn_audio_tab(commands, panel: Entity, settings: &AudioSettings, font)` — one `spawn_slider(SliderField { label: channel.label(), value })` per `Channel::ALL`; `ValueChange<f32>` → `AudioSettings::set`, and `write_audio_settings` when `is_final`.
- Produces, in `interface.rs`: `#[derive(Component)] pub struct ScaleLimitNote;` `pub fn spawn_interface_tab(commands, panel: Entity, settings: &InterfaceSettings, font)` — `spawn_segmented(Segmented { label: "UI scale", options: labels of UiScaleChoice::ALL, selected })`; `SegmentChange` → set `InterfaceSettings.ui_scale`, `write_interface_settings`; under it the note `"Limited to {scale} by the window height"` (`FONT_S`, `MUTED`), displayed only while `ui_scale_for(choice, height) < choice.factor()`, kept current by a system on `UiScale` and `InterfaceSettings` changes. No Graphics tab: no graphics setting exists (render distance is a runtime key, not a setting).

- [ ] **Step 1: Write the failing tests** (e2e, `e2e/settings.rs`; `settings()` waits for `L::role(Role::Tab, "Controls")`, `audio()` and `interface()` click their tab)

- `the_settings_match_their_golden` — `interface()`, `hover_at` a corner, golden `menu_settings` (the Interface tab: the only one without its own golden).
- `controls_audio_and_back_open_their_screen` becomes `the_tabs_show_their_content_and_back_returns_to_the_menu` — the Controls content shows; click tab "Audio" → `L::role(Role::Slider, "Master")`, `menu_state() == Settings`; tab "Interface" → `L::role(Role::RadioGroup, "UI scale")`; click `"Back"` → `MenuState::Main`.
- `the_keyboard_alone_walks_the_tabs` — `focus_label() == Some("Settings")` (the tab list); ArrowRight → the Master slider shows; Tab → `focus_label() == Some("Master")`; ArrowLeft → `"95%"` within `L::row("Master")`.
- `escape_leaves_the_settings_and_the_audio` becomes `escape_leaves_the_settings_from_any_tab` — `audio()`, Escape → `MenuState::Main`.
- `the_steppers_change_the_volume_by_ten_percent` becomes `a_click_the_arrows_and_the_wheel_change_a_volume_saved_at_once` — `audio()`; click Master → `"50%"` within `row("Master")`; ArrowRight → `"55%"`; `scroll(1.)` → `"60%"`; ArrowRight ×10 → `"100%"`; then, without leaving, `audio.ron` has `master == 1.0`.
- `the_volumes_are_saved_when_leaving` becomes `a_volume_is_saved_at_once` — click Effects → `wait_until` `audio.ron` has `effects == 0.5`.
- `the_audio_screen_matches_its_golden` — `audio()`, `hover_at` a corner, golden `menu_audio`.
- `the_ui_scale_is_chosen_clamped_and_saved` — 1920×1080, `interface()`: click `"3"` → `UiScale == 1.5` and `"Limited to 1.5 by the window height"` visible, `interface.ron` holds `X3`; `resize(800, 600)` → `UiScale == 1.0`, note `"Limited to 1 by the window height"`; `resize(1920, 1080)`, click `"1"` → `1.0`, no note; a second `Harness::start(folder.clone(), 1920×1080)` → `UiScale == 1.0`.
- `every_control_of_the_settings_lightens_on_hover` — hover tab "Audio" → `RAISE_HOVER`; hover `"Back"` → `RAISE_HOVER`.
- The controls tests (`each_action_lists_its_keys`, `a_key_pressed_after_clicking_an_action_is_bound_and_saved`, `the_clear_button_removes_the_keys_of_an_action`, `the_controls_match_their_golden`) keep their journeys on the hosted list: they wait for `L::text("MoveForward")` instead of "Keyboard Controls", and the bound key is read from `keybindings.ron` after `"Back"` (the settings close).

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client e2e::settings` → no tab "Controls".

- [ ] **Step 3: Implement**

- The screen, the Audio and Interface tabs as in Interfaces; `TabChange` despawns the children of `SettingsPanel` and spawns the chosen tab's content.
- The Controls tab hosts the existing list for one task: `controls_menu_setup` becomes `spawn_controls_tab(commands, panel, ..)` spawning only the scroll area and the recorder under `panel` (its title, Back button and background go); `controls_update_system` runs while an `ActionRecorder` exists; `save_keybindings` runs `OnExit(MenuState::Settings)`. Its unit tests adapt to the new spawn function. Task 7 replaces all of it.
- `MenuState` is now `Main, Solo, Multi, Settings, Disabled`; remove every system, variant, style helper and import this orphans (`rg` for each removed name finds nothing).

- [ ] **Step 4: Run the tests; regenerate three goldens on purpose** — `menu_settings` (the tab bar with Interface checked, the segmented control with Auto checked, no note, Back, the hint bar), `menu_audio` (three sliders at 100 % with crisp thumbs, readouts aligned), `menu_controls` (interim: the old list inside the panel, nothing outside it). `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_settings_match the_audio_screen_matches the_controls_match`, rerun → PASS, no leftovers. Full suite green.

- [ ] **Step 5: Commit** (the report lists the renamed tests and changed locators of `e2e/settings.rs`) — `git commit -m "feat(client): one settings screen with tabs, volume sliders and a UI scale that save at once"`

---

### Task 7: The Controls tab — human names, two key columns, capture and clear

**Files:**
- Modify: `client/src/ui/menus/settings/controls.rs` (rewrite), `client/src/ui/menus/settings/mod.rs` (hints of the tab, `settings_escape` reads `KeyCapture`), `client/src/ui/menus/mod.rs` (`save_keybindings` no longer `OnExit`), `client/src/input/data.rs`, `client/src/input/keyboard.rs`, `client/src/e2e/settings.rs`, `client/tests/golden/menu_controls.png`

**Interfaces:**
- Consumes: `key_cap`, `default_bindings`, `write_keybindings`, `MAX_KEYS_PER_ACTION`, `ArrowNav`, `FocusRing`, `ScrollArea`, `ScrollIntoView`.
- Produces, in `input/data.rs`:
  ```rust
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum ActionCategory { Movement, Interface, View, Debug }
  impl ActionCategory { pub const ALL: [ActionCategory; 4]; pub fn label(self) -> &'static str }
  impl GameAction {
      /// Every action but Escape, which always pauses and cannot be rebound, in display order.
      pub const LISTED: [GameAction; 18];
      pub fn name(self) -> &'static str;
      pub fn category(self) -> ActionCategory;
  }
  ```
  Exact names — Movement: MoveForward "Move forward", MoveBackward "Move backward", MoveLeft "Move left", MoveRight "Move right", Jump "Jump", ToggleFlyMode "Toggle flying", FlyUp "Fly up", FlyDown "Fly down"; Interface: ToggleInventory "Inventory", OpenChat "Chat"; View: ToggleViewMode "Camera view", RenderDistanceMinus "Lower render distance", RenderDistancePlus "Raise render distance"; Debug: ToggleFps "Debug overlay", ToggleChunkDebugMode "Chunk borders", ToggleBlockWireframeDebugMode "Block wireframe", ToggleRaycastDebugMode "Raycast debug", ReloadChunks "Reload chunks".
- Produces, in `input/keyboard.rs`: `pub fn set_key(map: &mut KeyMap, action: GameAction, slot: usize, key: KeyCode)` (replaces the key in `slot`, or appends when the slot is empty; a key already in the other slot of that action is not repeated); `pub fn clear_key(map: &mut KeyMap, action: GameAction, slot: usize)` (clearing the primary promotes the secondary).
- Produces, in `controls.rs`:
  ```rust
  #[derive(Component, Clone, Copy, PartialEq, Eq, Debug)] pub struct KeySlot { pub action: GameAction, pub slot: usize }
  #[derive(Resource, Default)] pub struct KeyCapture { slot: Option<Entity>, ending: bool }
  impl KeyCapture { pub fn is_active(&self) -> bool }
  pub const PRESS_A_KEY: &str = "Press a key...";
  pub const CONTROLS_HINTS: [Hint; 5];          // Tab Move, Enter Change key, Del Clear, Esc Back, ↑↓ Move
  fn key_label(key: KeyCode) -> String;
  fn slot_label(action: GameAction, slot: usize) -> String;   // "Jump, primary key" / "Jump, secondary key"
  pub fn spawn_controls_tab(commands: &mut Commands, panel: Entity, key_map: &KeyMap, fonts: (Handle<Font>, Handle<Font>));
  ```
  Layout: column headers `"Primary"`, `"Secondary"` (`MUTED`), then a `ScrollArea` (`overflow: scroll_y`, `ArrowNav`, `flex_grow: 1`) holding, per `ActionCategory`, its label (`FONT_S`, `MUTED`) and one row per action: [name `FONT_M` (`flex_grow: 1`), two slots 120 px wide]. A slot is `(KeySlot, AccessibilityNode(Role::Button), AccessibleLabel(slot_label), TabIndex(0), FocusRing, Hovered, RAISE + raised bevel)` showing `key_cap(key_label(key))`, nothing when empty, `PRESS_A_KEY` with an `ACCENT` border while capturing; `RAISE_HOVER` on hover. `key_label`: letters and digits bare (`W`, `1`), `Space`, `Enter`, `Tab`, `Backspace`, `Left Shift`, `Right Shift`, `Left Ctrl`, `Right Ctrl`, `Left Alt`, `Right Alt`, `F1`–`F12`, arrows `↑ ↓ ← →`, anything else its `Debug` name without `Key`.
  Behaviour (observers on the slots): Enter (pressed, not repeat) or a left click starts the capture of that slot. While capturing, the next key pressed on it is consumed (`propagate(false)`): Escape cancels; any other key is `set_key` then `write_keybindings`; the capture ends in `PostUpdate` (`ending`). The focus leaving the capturing slot cancels it (a click elsewhere, for the mouse alone). Not capturing: Delete or Backspace, or a right click, `clear_key` then `write_keybindings`. A slot gaining the focus triggers `ScrollIntoView`. The rows refresh when `KeyMap` changes.

- [ ] **Step 1: Write the failing tests**

Unit, `data.rs`: `every_listed_action_has_a_human_name_and_a_category` — `LISTED` has 18 distinct actions, no `Escape`; `MoveForward.name() == "Move forward"`, `ToggleFps.name() == "Debug overlay"`; `Jump` is Movement, `OpenChat` Interface, `ToggleViewMode` View, `ReloadChunks` Debug.
Unit, `keyboard.rs`: `setting_and_clearing_key_slots` — `[Space]`: `set_key(Jump, 1, KeyJ)` → `[Space, KeyJ]`; `set_key(Jump, 0, KeyK)` → `[KeyK, KeyJ]`; `set_key(Jump, 1, KeyK)` → `[KeyK]`; on `[]`, `set_key(Jump, 1, KeyJ)` → `[KeyJ]`; `[KeyK, KeyJ]`, `clear_key(Jump, 0)` → `[KeyJ]`; `clear_key(Jump, 1)` on `[KeyJ]` → `[KeyJ]`.
Unit, `controls.rs`: `keys_have_human_labels` — `KeyW "W"`, `Digit1 "1"`, `Space "Space"`, `ShiftLeft "Left Shift"`, `ControlRight "Right Ctrl"`, `F3 "F3"`, `ArrowLeft "←"`.

E2e, `e2e/settings.rs` (`controls()` = `settings()`, the default tab):
- `each_action_lists_its_keys` — `L::text("W").within(L::row("Move forward"))`, `"Space"` within `row("Jump")`; `"Movement"`, `"Interface"`, `"View"`, `"Debug"` shown; no `L::text("MoveForward")`.
- `the_arrow_keys_show_their_glyph` — `bright_pixels_in(&L::text("↑").within(L::row("Move forward"))) > 20`.
- `a_key_pressed_after_clicking_an_action_is_bound_and_saved` becomes `a_key_pressed_after_clicking_a_slot_is_bound_and_saved_at_once` — click `L::role(Role::Button, "Jump, secondary key")` → `PRESS_A_KEY` visible; `press(KeyJ)` → `"J"` within `row("Jump")`; `keybindings.ron` has `Jump == [Space, KeyJ]` without leaving.
- `the_keyboard_alone_rebinds_cancels_and_clears` — Tab from the tab list → `focus_label() == Some("Move forward, primary key")`; ArrowDown → `"Move backward, primary key"`; Enter → `PRESS_A_KEY`; Escape → no `PRESS_A_KEY`, `"S"` still within its row, `menu_state() == Settings` (after 2 more frames too); Enter, `press(KeyX)` → `"X"`; Delete → `KeyMap[MoveBackward] == [ArrowDown]`; Escape → `MenuState::Main`.
- `the_clear_button_removes_the_keys_of_an_action` becomes `a_right_click_clears_a_slot_and_a_click_elsewhere_cancels_a_capture` — `right_click` Jump's primary → `KeyMap[Jump] == []`; click Jump's primary → capturing; click `L::text("Jump")` (the name) → no `PRESS_A_KEY`, `KeyMap[Jump] == []`.
- `the_wheel_scrolls_the_controls_list_under_the_cursor` — `hover(&L::text("Move forward"))`, `scroll(-3.)` → its centre moved up; hover the tab "Audio", `scroll(-3.)` → nothing moved.
- `every_control_of_the_settings_lightens_on_hover` gains: hover Jump's primary slot → `RAISE_HOVER`.
- `the_controls_match_their_golden` — `hover_at` a corner first.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- input:: settings::controls e2e::settings` → compile errors, then code names on screen.

- [ ] **Step 3: Implement** as in Interfaces; the legacy list, recorder, `ClearButton`, `EditControlButton`, `ActionRecorder`, the trash icon and the `OnExit` save go, with their tests; `settings_escape` skips while `KeyCapture::is_active()`.

- [ ] **Step 4: Run the tests; regenerate `menu_controls` on purpose** — categories, human names, two aligned key columns of key caps, `↑ ↓ ← →` drawn, no ring, no hover. `UPDATE_GOLDEN=1 …`, rerun → PASS, no leftovers. Full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): the controls tab with human names, two key columns, key capture and clearing"`

---

### Task 8: Conflicts and "Reset to defaults"

**Files:**
- Modify: `client/src/input/keyboard.rs`, `client/src/ui/menus/settings/controls.rs`, `client/src/e2e/settings.rs`

**Interfaces:**
- Produces, in `keyboard.rs`:
  ```rust
  /// Pairs that share a key on purpose: Space jumps on foot and flies up in fly mode.
  pub const SHARED_KEYS: [(GameAction, GameAction); 1] = [(GameAction::Jump, GameAction::FlyUp)];
  /// For each action, the keys it shares with other actions, and with which.
  pub fn conflicts(map: &KeyMap) -> BTreeMap<GameAction, Vec<(KeyCode, GameAction)>>;
  ```
- Produces, in `controls.rs`: `#[derive(Component)] pub struct ConflictMessage(pub GameAction);` under each row (inside it, `FONT_S`, `DANGER`, `Display::None` without conflict) showing `conflict_message(..)` = `"{key} is also bound to {name}"`, several joined by `" · "`; the slot holding a conflicting key has a `DANGER` border. `#[derive(Component)] pub struct ResetBindings;` on `button("Reset to defaults")` under the list: `KeyMap = default_bindings()`, `write_keybindings`. A conflict is shown, never refused.

- [ ] **Step 1: Write the failing tests**

Unit, `keyboard.rs`: `conflicts_are_reported_on_both_actions` — defaults plus `Jump: [Space, KeyW]` → `Jump: [(KeyW, MoveForward)]`, `MoveForward: [(KeyW, Jump)]`, nothing else; `jump_and_fly_up_may_share_space` — `conflicts(&default_bindings())` is empty.
Unit, `controls.rs`: `a_conflict_message_names_the_key_and_the_other_action` — `[(KeyW, MoveForward)]` → `"W is also bound to Move forward"`; two → joined by `" · "`.

E2e, `e2e/settings.rs`: `a_key_bound_twice_is_shown_on_both_rows_and_reset_restores_the_defaults` — no `L::text_containing("is also bound")` at first; click Jump's secondary, `press(KeyW)` → `"W is also bound to Move forward"` within `row("Jump")` and `"W is also bound to Jump"` within `row("Move forward")`; click `"Reset to defaults"` → no message, `KeyMap == default_bindings()`, `keybindings.ron` equals the defaults.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- input::keyboard settings::controls e2e::settings::a_key_bound_twice` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, `menu_controls` unchanged (no conflict by default; the reset button sits below the visible rows or is in the golden since Task 7 — if it moves the golden, regenerate after a look and say so in the report).

- [ ] **Step 5: Commit** — `git commit -m "feat(client): key conflicts shown on both rows, and a reset of the key bindings"`

---

### Task 9: Settings from the pause menu

**Files:**
- Modify: `client/src/ui/menus/pause.rs`, `client/src/ui/hud/mod.rs` (one system), `client/src/ui/menus/settings/mod.rs` (`SettingsContext::Game`), `client/src/e2e/game.rs`

**Interfaces:**
- Consumes: `spawn_settings`, `settings_open`, `SettingsRoot`, `KeyCapture`.
- Produces: `PauseButtonAction::Settings` (a legacy button `"Settings"` between `"Resume"` and `"Save"`); `pub fn open_settings_from_pause(...)` (on its press, `spawn_settings(.., SettingsContext::Game)`); `render_pause_menu` ignores Escape and its buttons while `settings_open`. The `Game` root: `UiDialog`, `Visibility::Visible`, `GlobalZIndex(6)` (above the pause menu's 5), `FocusPolicy::Block`, `TabGroup::modal()`, `DespawnOnExit(GameState::Game)`, `BackgroundColor(BEVEL_LO.with_alpha(0.7))` instead of the dirt; `CloseSettings` despawns it and clears `InputFocus`; the pause menu stays open beneath.

- [ ] **Step 1: Write the failing tests**

Unit, `pause.rs`: `escape_with_the_settings_open_leaves_the_pause_menu_open` — the `app` helper, pause `Visible`, a `SettingsRoot` entity: Escape → pause still `Visible`.

E2e, `e2e/game.rs`, in `the_pause_menu_resumes_saves_and_leaves`, before its `assert!(!player.exists())`: Escape, click `"Settings"` → `L::role(Role::Tab, "Controls")` visible, `GameState::Game`, `UIMode::Opened`; click Jump's secondary, `press(KeyE)` → `"E"` within `row("Jump")` and `InventoryRoot` not visible; Delete; click tab "Audio" → Master slider shown; Escape → no `SettingsRoot`, `"Resume"` still visible; click `"Resume"`. The existing `!player.exists()` then proves no click reached the legacy "Save" beneath the overlay.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- menus::pause e2e::game::the_pause_menu` → no "Settings" button.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): the settings open from the pause menu without leaving the game"`

---

### Task 10: Legacy cleanup, review snapshots at three resolutions, and docs

**Files:**
- Modify: `client/src/e2e/review.rs`, `README.md`, `docs/ARCHITECTURE.md`, plus whatever the review finds in the files of Tasks 2–9

- [ ] **Step 1: Check the legacy code** — `rg -n "MenuButtonAction|button_system|ScrollingList|mouse_scroll|fill_image_node|text_font|NORMAL_BUTTON|load_button_background|load_dark_button" client/src`: every hit is in `list_screen.rs`, `solo.rs`, `multi.rs`, `pause.rs`, `chat.rs`, `button.rs` or `style.rs`, and every item left in `button.rs`, `style.rs` and `assets.rs` has a user (`cargo clippy` with no `dead_code` allowance proves it). `MenuButtonAction` holds `BackToMainMenu` only.

- [ ] **Step 2: Extend the review journey** — menus: start from `GameFolder::new().with_played_world("Review", 0, 3 days)`; per resolution: `"main"`; Escape → `"quit_dialog"`, Escape; Settings → `"settings_controls"`, click Jump's secondary → `"settings_capture"`, Escape (cancels), tab Audio → `"settings_audio"`, tab Interface → `"settings_interface"`, Back. In game, after the pause snapshot: click `"Settings"` → `snapshot_in_game("settings_in_game")`, Escape.

- [ ] **Step 3: Run and look**

Run: `UI_SNAPSHOTS=1 timeout 600 cargo test -q -p client review -- --ignored`
Read every new `target/ui-snapshots/every_screen_at_three_resolutions/*_{800x600,1280x720,1920x1080}.png`. Check: everything fits 800×600 (the card beside the buttons, the hint bar clear of them, the settings panel and its Back above the hint bar, the controls list scrolling inside the panel); the dirt texels and the logo are whole and even at 1.5 (1920×1080); the vignette darkens the corners only; the dialog centred with Cancel on the left, Quit last; the capture slot shows "Press a key..." with its accent border; arrow glyphs drawn; nothing overflows its button. Fix any defect in its task's file with a test first.

- [ ] **Step 4: Positions from the window** — `rg -n "cursor_position|world_to_viewport" client/src`: every result that ends in a `Val::Px` divides by `UiScale` (Lot 0 lesson); fix any that does not, with a test.

- [ ] **Step 5: Docs** — ARCHITECTURE "UI widgets": `dialog.rs` (a modal over the screen, Cancel focused, Escape closes it, the focus comes back; closes in `PostUpdate`), `hints.rs` (the key-hint bar and key caps, arrows in Fira Mono), `tabs.rs` and `segmented.rs` (on `RadioGroup`), `slider.rs` (5 % steps, wheel under the cursor); one sentence on the Escape rule of the Global Constraints; the settings are one screen, a state in the menus and an overlay in the game. README: under "Saves", one line — "The settings are saved as soon as they change, in the game folder: `keybindings.ron`, `audio.ron` and `interface.ron` (UI scale)."; in the controls table, "Exit Game — Escape" becomes "Pause / back — Escape".

- [ ] **Step 6: Full suite** — `timeout 900 cargo test --workspace --no-fail-fast` green; fmt and clippy clean; `git status` shows no `.actual.png` / `.diff.png`.

- [ ] **Step 7: Commit** — `git commit -m "test(client): review snapshots of the main menu and the settings, and docs of the new widgets"`
