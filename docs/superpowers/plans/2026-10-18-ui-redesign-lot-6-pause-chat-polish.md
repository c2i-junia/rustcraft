# UI Redesign Lot 6 (Pause, chat, polish) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the pause menu on the widgets (Back to game focused, Settings, Save now with a "World saved" toast, Save and quit to title showing "Saving..." until the server has stopped, without freezing the game), make closing the window in a world save before quitting, give the chat a history of 100 timed messages that scrolls, fades and recalls sent messages with Up/Down, open it with `/`, and close the redesign with a review of every screen and the removal of the last legacy UI code.

**Architecture:** The pause menu becomes a modal spawned on Escape and despawned on close, assembled from `button`, `hint_bar` and a new `toast` widget; it no longer uses `Interaction`, which removes the last legacy button and `click_sounds`. Leaving a world is a non-blocking wait in the game (`network/leave.rs`: `LeaveGame`, `Leaving`, `STOP_TIMEOUT`): Exit is sent, the frames keep running behind "Saving...", and the game ends once the local server thread `is_finished()`. `terminate_server_connection` never waits on a thread again. Quitting (window close, main menu Quit) goes through one `QuitRequested` event (`ui/quit.rs`) that saves a running world first. "World saved" comes from an in-process counter of finished saves shared with the embedded server. The chat keeps its entities and gains plain, unit-tested functions for times, fading and recall.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate, ScrollArea, Scrollbar}`, `bevy::input_focus`, `bevy::text::EditableText`, `bevy::window::WindowCloseRequested`), `accesskit` 0.24 (`Role::Status`, `Role::Alert`), `chrono` 0.4 (local time of chat lines, new), lavapipe for e2e.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md`: only "Lot 6: Pause, chat, polish", the "Pause", "Chat", "HUD" and "Debug" paragraphs of "Screens", the toast of "Visual system", and the end of "Testing" (review snapshots). "Visual system", "Architecture", "Testing" and the anti-goals bind every task. Defects to fix: `docs/superpowers/specs/2026-10-18-ui-e2e-tests-design.md`, "Non-goals" (the chat and pause lines, and the whole list closed in Task 7). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md`. Conventions and products: the Lot 0–5 plans next to this file. Lot 4 is the closest model (Global Constraints, the session's end, one server journey); Lot 1 made the settings overlay (`SettingsContext::Game`, `CloseSettings`), the dialog and hint widgets and `KeyCapture`.

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `feat(shared): …`, `fix(…)`, `test(…)`, `refactor(…)`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- This plan was written against `27c1a3c`. Small fixes to `client/src/network/{session,cleanup,setup}.rs`, `client/src/game.rs`, `client/src/ui/menus/connect_error.rs` and `server/src/init.rs` may have landed since: read each file as it is before editing it, and keep those fixes.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::pause`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift, which runs no destructor while a panic unwinds: do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`, driven only through `click`/`hover`/`press`/`type_text`/`scroll` and the new `type_on`/`request_close`. Lot 6's tests go in the new `e2e/pause.rs` (pause, saving, leaving, quitting) and `e2e/chat.rs`; the pause and chat tests of `e2e/game.rs` move there with their journeys. Every `enter_world` starts a server: **one journey per behaviour group**, chaining its checks. **One** dedicated-server journey (Task 1).
- **No fixed network port in tests**: servers bind with `server::acquire_local_socket_pair`.
- Never a bare `git stash`; set work aside with a WIP commit.
- After Task 1 no screen uses `bevy::prelude::Button` or `Interaction`. Use `use bevy::ui_widgets::Button as UiButton;`. The widget component or the explicit `AccessibilityNode(Role::…)` goes **before** `AccessibleLabel` in a bundle.
- A widget item no screen uses yet allows `dead_code` on the item with a comment naming its first user; never crate-wide.
- **Escape.** Whatever consumes Escape (a dialog, a key capture, the settings, the chat field) finishes in `PostUpdate`; the handlers reading `ButtonInput<KeyCode>` in `Update` (`toggle_pause_menu`, `settings_escape`) skip while `dialog_open`, `text_field_focused`, `KeyCapture::is_active()`, `settings_open` or `leaving` holds. The pause menu closes in `PostUpdate`.
- **The first end of a session wins** (Lot 4). Leaving on purpose is one more writer: `watch_session` and `end_session` stand down while `Leaving` exists.
- **The UI thread never waits on the network or a thread.** A local server thread is joined only once `is_finished()`. The one bounded wait left is `close_session_on_app_exit`, in the app's last frame, for an `AppExit` that did not come through `QuitRequested`.
- **Protocol.** `PROTOCOL_ID` stays `5`; no game message changes. The save counter is shared in-process with the embedded server only.
- **Dependencies.** One new crate: `chrono = { version = "0.4", default-features = false, features = ["clock"] }` in `client/Cargo.toml` (Task 5).
- **Fonts.** Only `FONT_PATH` (RustCraft) and `MONO_FONT_PATH` (key caps, debug overlay). The RustCraft font has no `…`, `←↑→↓`, `×`, `−` or accented letters: write `Saving...`.
- **Pixel art.** A node placed from `Window::cursor_position()` or `world_to_viewport` divides by `UiScale`.
- **Goldens.** No golden changes and none is added: the pause menu, the chat and the toasts are drawn over the 3D world (e2e spec: no golden of the world) and are checked by behaviour and review snapshots. Any golden that changes is a bug.
- **The version string** `v0.0.0` (from `client/Cargo.toml`, while git tags reach `v1.0`) is the owner's call: do not change it (Task 7 reports it).

## Decisions (where the spec is silent)

1. **The pause menu is a modal spawned on open and despawned on close**, not a hidden node: `AutoFocus` then focuses "Back to game" each time, and `TabGroup::modal()` keeps Tab and the arrows inside. Title `Game menu` (`FONT_XL`), decided because the server keeps ticking beneath it (also the embedded one), so "Paused" would be untrue; under it the world or server name (`FONT_S`, `MUTED`, wrapping). A centred panel as the dialogs' (`PANEL`, raised bevel, padding `SPACE_XL`, `width: 320`, `max_width: 90 %`), stacked `Default` buttons at full width with `row_gap: SPACE_S` (as the main menu's column); the leaving action sits apart (`margin_top: SPACE_L`). Root: full window, `BackgroundColor(BEVEL_LO.with_alpha(0.6))`, `GlobalZIndex(5)`, `UiDialog`, `AccessibilityNode(Role::Dialog)` labelled `Game menu`. Hints `Tab Move`, `Enter Select`, `Esc Back`.
2. **On a server the pause menu offers `Back to game`, `Settings`, `Disconnect`.** A dedicated server ignores a client's save request (`save_world_request` saves only when `is_solo`), so "Save now" would do nothing, and the world is not the player's to save. `Disconnect` ends the session as a cancel (`end_session(.., None)`), which returns to the server list with its card focused (Lot 4, Decision 2). A world keeps the spec's `Save and quit to title`, which lands on the main menu.
3. **The HUD is hidden under the pause menu and the in-game settings** (parked item b): `HudBottom` (the hotbar and its item name) and the `Reticle` are `Visibility::Hidden` while either is open. Decided because a hotbar half covered by the hint bar reads as a layout bug at every size (at 800×600 its slots show above the 44 px bar), nothing in those menus concerns it, and the hint bar must stay at the bottom as on every other screen.
4. **Closing the in-game settings gives the focus back to the button that opened them** (parked item a): `SettingsRoot` remembers it (`restore_focus`), as `DialogRoot` does.
5. **"World saved" is said only once the save is written.** `world.ron` is written before the chunks (`write_job`), so its date would announce an unfinished save, and a server message would need a protocol bump for something only the embedded server does. The embedded server's `WorldWriter` counts the jobs it wrote without a failed chunk in a `SaveCounter` shared with the client of the same process. "Save now" reads `Saving...` and is disabled until the counter moves (`World saved` toast) or `SAVE_TIMEOUT` (10 s) passes (`Could not save the world`, an error toast). An autosave finishing just after the click also counts: it is a save made at most a few seconds earlier.
6. **Toasts.** Top-right (`top`/`right`: `SPACE_L`), stacked downward with `SPACE_S` between them, `max_width: 360`, `PANEL` with a raised bevel and a 4 px left border `OK` (info) or `DANGER` (error), text `FONT_S` `TEXT`, wrapping. `GlobalZIndex(40)`: over the screens and overlays, under the dialogs (`DIALOG_Z` 50). `Pickable::IGNORE`: a toast never takes a click. Info is `Role::Status`, error `Role::Alert`, labelled with their text. 3 s on `Time<Real>`, then despawned; no animation. They survive a change of state.
7. **Leaving a world does not block a frame** (parked item d). `Save and quit to title` sends `Exit` and stays in the game behind the pause menu, whose buttons are disabled and whose leaving button reads `Saving...`; Escape does nothing then; the hint bar hides. The game ends once the local server thread has finished, or after `STOP_TIMEOUT` (10 s), when the title opens anyway and the server finishes in the background (Lot 4's `finish_stopping_local_server` joins it; `ExitSent` keeps it from connecting again). `terminate_server_connection` no longer waits on the thread.
8. **The title-bar close in a world saves, then quits, without confirmation** (parked item f). Decided because nothing is lost (the world is saved first), the close button is a deliberate gesture, and a confirmation would also hold up an OS logout. A card `Saving the world...` with `Close the window again to quit now.` (`MUTED`) shows over everything until the world is saved or `STOP_TIMEOUT` passes; a second close quits at once. In the menus, with no server left running, the window closes at once, as today. The main menu's Quit goes through the same request.
9. **Quitting while a world is still starting waits for it, bounded** (parked item e): fixed rather than kept as a known limit, because Decision 8's quit already waits without blocking and Lot 4's `finish_stopping_local_server` already connects to a loading world and tells it to stop once it accepts. Quitting then cancels the session and waits up to `STOP_TIMEOUT` from the request. The "Known limits" line becomes: a world still loading 10 s after the quit is stopped with the process (its files are written atomically, so the save stays readable).
10. **The chat sits above the hotbar and its item name** (parked item c): bottom-left, `left: SPACE_L`, its bottom edge `SPACE_S` above the top of the item-name band (`HOTBAR_TOP + ITEM_NAME_BAND`), `width: 420`. Decided because at 800×600 the room beside the hotbar is about 150 px, too narrow to type in, and one position at every size keeps the chat from touching the hotbar or the item name.
11. **The chat's look.** Lines in `FONT_S`: the time (`MUTED`) then `<author> message` (`TEXT`), two texts in a row (`column_gap: SPACE_S`), wrapping (`LineBreak::WordOrCharacter`). Open: a log of 10 visible lines with a scrollbar as the lists' (`spawn_scroll_list`), its well `PANEL` at 0.75 alpha (a deviation from "panel opaque", decided because the chat is read over the moving game and an opaque block would hide a quarter of the view at 800×600), then the text field. Closed: only the lines younger than 8 s, at most 10, each on its own `PANEL` 0.6-alpha strip; they are hidden while another dialog is open (inventory, pause, settings), which would otherwise overlap them at 800×600.
12. **Times** are the local time of the server's timestamp, `HH:MM` (24 h): the timestamp is UTC milliseconds and std has no time zone, so `chrono`'s `clock` feature gives the local offset.
13. **Fading.** "Fades after 7 s" reads as: fully shown for 7 s, then fading linearly to nothing over 1 s (`CHAT_FADE_AFTER`, `CHAT_FADE`).
14. **Recall.** Sent messages of the current game (kept on the chat root, so they go with the game), at most 50, a repeat of the last one not recorded. Up recalls older, stopping at the oldest; Down newer, then gives back the text typed before recalling; the caret goes to the end. Escape and sending still keep no draft and end the recall.
15. **`/` opens the chat with "/" typed.** It is the character, not a key: the logical key `Key::Character("/")` on release (as T), so it works on every layout (AZERTY's `/` is Shift+`:`), and its key repeat is never typed. Only in gameplay (`UIMode::Closed`). It is not a new bindable action. The server has no commands: a message starting with `/` is sent as typed, as today.
16. **Review at four sizes**: 800×600, 1280×720, 1920×1080 (`UiScale` 1.5) and 2560×1440 (`UiScale` 2, the auto scale at that height). Every screen of every lot goes through all four, since every loop of `review.rs` iterates `RESOLUTIONS`.

## Review Focus

1. The local server stopping during "Saving..." never shows "The world stopped unexpectedly" (Lot 4's in-game watcher would read it as a stop). (Task 3: `saving_and_quitting_is_never_an_error`)
2. Escape, clicks and the leaving button pressed again while "Saving..." do nothing: no second `Exit`, no pause menu closing on a game that is ending. (Task 3: `save_and_quit_shows_saving_until_the_world_is_saved`)
3. The Escape that closes the in-game settings does not also close the pause menu, and the focus lands on "Settings" with its ring when the keyboard opened them. (Task 1: `the_keyboard_alone_drives_the_pause_menu`)
4. A chat message arriving while the inventory is open does not draw over its panel. (Task 5: `a_closed_chat_shows_only_its_recent_lines`)
5. A `/` while the inventory is open, an arrow key in gameplay, or a physical `/` key that types another character on the player's layout opens or recalls nothing. (Task 6: `up_and_down_recall_sent_messages_and_a_slash_opens_the_chat`)
6. Closing the window during "Saving..." quits once the save is done, and a second close quits at once. (Task 4: `closing_the_window_in_a_world_saves_then_quits`, `a_second_close_quits_at_once`)

## Defects of the e2e spec fixed here

"The chat keeps only the last 2 messages" (Task 5); "no Tab or keyboard activation anywhere" and "the pressed look shows the normal image" for the pause menu, the last legacy screen (Task 1); "F3, F4, F6, F7 work while the chat or the pause menu is open" keeps its test with the new locators (Task 1). Task 7 closes the whole Non-goals list. Parked items: (a) Task 1, (b) Task 1, (c) Task 5, (d) Task 3, (e) Task 4, (f) Task 4, (g) Tasks 5 and 6, (h) Task 7 (reported, not changed).

## Files

- `client/src/ui/menus/pause.rs` (rewrite), `client/src/ui/menus/settings/mod.rs`, `client/src/ui/hud/{mod,reticle}.rs`, `client/src/audio/mod.rs`, `client/src/e2e/{pause,connecting,game,main_menu,review}.rs` (`pause.rs` new) — Task 1.
- `client/src/ui/widgets/{toast,mod,button}.rs` (`toast` new), `shared/src/lib.rs`, `server/src/world/writer.rs`, `server/src/init.rs`, every `GameServerConfig { .. }`, `client/src/network/setup.rs` — Task 2.
- `client/src/network/{leave,mod,cleanup,session}.rs` (`leave` new) — Task 3.
- `client/src/ui/quit.rs` (new), `client/src/ui/mod.rs`, `client/src/ui/menus/home.rs`, `client/src/main.rs`, `client/src/e2e/{harness,input}.rs` — Task 4.
- `client/src/ui/hud/chat.rs`, `client/src/ui/hud/inventory/setup.rs`, `client/src/ui/hud/hotbar.rs`, `client/src/ui/widgets/list.rs`, `client/Cargo.toml`, `client/src/e2e/chat.rs` (new); deleted `client/src/ui/style.rs` — Tasks 5, 6.
- `client/src/e2e/review.rs`, `README.md`, `docs/ARCHITECTURE.md` — Tasks 4, 7.

---

### Task 1: The pause menu on the widgets, with the focus back on Settings and no hotbar beneath its hints

**Files:**
- Create: `client/src/e2e/pause.rs`
- Modify: `client/src/ui/menus/pause.rs` (rewrite), `client/src/ui/menus/settings/mod.rs`, `client/src/ui/hud/mod.rs`, `client/src/ui/hud/reticle.rs`, `client/src/audio/mod.rs`, `client/src/e2e/{mod,game,connecting,main_menu,review}.rs`

**Interfaces:**
- Consumes: `button`, `ButtonVariant`, `hint_bar`, `Hint`, `ArrowNav`, `spawn_settings`, `settings_open`, `SettingsRoot`, `KeyCapture`, `dialog_open`, `text_field_focused`, `SessionOrigin`, `end_session`, `send_save_request_to_server`, `in_gameplay`, `HudBottom`, `Reticle`.
- Produces, in `ui/menus/pause.rs`:
  ```rust
  pub const PAUSE_TITLE: &str = "Game menu";
  pub const PAUSE_HINTS: [Hint; 3];                       // Tab Move, Enter Select, Esc Back
  #[derive(Component)] pub struct PauseMenu;             // the root
  #[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
  pub enum PauseButton { BackToGame, Settings, SaveNow, SaveAndQuit, Disconnect }
  impl PauseButton { pub fn label(self) -> &'static str }  // "Back to game", "Settings", "Save now", "Save and quit to title", "Disconnect"
  /// The buttons of the pause menu, top to bottom: a world's, or a server's (Decision 2).
  pub fn pause_buttons(origin: Option<&SessionOrigin>) -> &'static [PauseButton];
  pub fn pause_open(menus: Query<(), With<PauseMenu>>) -> bool;
  fn toggle_pause_menu(...);   // Update, Game, after GameSet::UiMode: Escape opens it (UIMode::Closed), closes the inventory, or asks it to close (ClosePause)
  fn close_pause_menu(...);    // PostUpdate: despawns a root with ClosePause, clears InputFocus
  fn act_on_pause_button(...); // observer On<Activate> on PauseButton
  fn hide_hud_under_menus(...);// Update, Game: HudBottom Hidden while pause_open || settings_open, else Inherited
  pub fn pause_plugin(app: &mut App);   // added by hud_plugin in place of setup_pause_menu, render_pause_menu, open_settings_from_pause
  ```
  Actions: `BackToGame` → close; `Settings` → `spawn_settings(.., SettingsContext::Game, Some(button))`; `SaveNow` → `send_save_request_to_server` (Task 2 adds the toast); `SaveAndQuit` → `GameState::Menu` (Task 3 makes it wait); `Disconnect` → `end_session(.., origin, None)`. `show_reticle` also hides the reticle while `pause_open || settings_open`.
- Changes, in `settings/mod.rs`: `SettingsRoot { pub context: SettingsContext, pub restore_focus: Option<Entity> }`; `pub fn spawn_settings(commands: &mut Commands, assets: &AssetServer, context: SettingsContext, restore_focus: Option<Entity>) -> Entity`; `close_settings` in `Game` sets `InputFocus` to `restore_focus` when it still exists, else clears it. The `Game` root loses `FocusPolicy::Block` (no `Interaction` is left beneath it).
- Removes: `PauseButtonAction`, `setup_pause_menu`, `render_pause_menu`, `open_settings_from_pause`, `audio`'s `click_sounds` (`click_on_activate`'s doc no longer mentions legacy buttons).
- Produces, in `e2e/connecting.rs`: `pub(super) fn spawn_test_server(world: &str) -> (SocketAddr, JoinHandle<Result<(), String>>)` — the server of `a_refused_join_says_why…` (`acquire_local_socket_pair`, `init_with_status`, `is_solo: true`), which that test now calls.

- [ ] **Step 1: Write the failing tests**

Unit:
- `pause.rs`: `a_world_and_a_server_offer_their_own_buttons` — `World` and `None` → `[BackToGame, Settings, SaveNow, SaveAndQuit]`; `Server` → `[BackToGame, Settings, Disconnect]`; labels as above.
- `pause.rs`: the existing Escape tests keep their cases on a bare `App` with `toggle_pause_menu` and `close_pause_menu`: `escape_opens_the_pause_menu_during_gameplay` (a `PauseMenu` exists after the update), `escape_closes_the_inventory_instead_of_opening_the_pause_menu`, `escape_while_typing_does_not_open_the_pause_menu`, `escape_with_the_settings_open_leaves_the_pause_menu_open`; new `escape_closes_the_pause_menu_only_after_the_frame` — open, Escape → the root still exists at the end of `Update`, gone after `PostUpdate`.
- `settings/mod.rs`: `closing_the_settings_in_game_gives_the_focus_back` — `SettingsRoot { Game, Some(opener) }` + `CloseSettings` → after `PostUpdate` `InputFocus == Some(opener)`, root gone; with the opener despawned first → `InputFocus == None`.

E2e, `e2e/pause.rs` (the pause and leaving tests of `e2e/game.rs` move here):
- `the_pause_menu_opens_on_back_to_game_and_escape_returns_to_the_game` — `enter_world(with_world("Pause"))`; Escape → `L::role(Role::Dialog, PAUSE_TITLE)` visible, `"Pause"` within it, `focus_label() == Some("Back to game")`, the four buttons' centres in order top to bottom, `"Esc"` within `HintBar`; `HudBottom` and `Reticle` not visible; Escape → no `PauseMenu`, `UIMode::Closed`, `HudBottom` visible; `frames(2)` → still closed.
- `the_keyboard_alone_drives_the_pause_menu` (e2e defect "no keyboard activation"; parked a; Review Focus 3) — Escape; ArrowDown → `focus_label() == Some("Settings")`, its `Outline.color == ACCENT`; Enter → `L::role(Role::Tab, "Controls")`; Escape → no `SettingsRoot`, `PauseMenu` still there, the focused entity has `PauseButton::Settings`, ring `ACCENT`; Enter → the settings open again; Escape; `click(L::role(Role::Button, "Settings"))`, Escape → the focused entity is the Settings button again; Tab ×5 → the focus stays a descendant of `PauseMenu`; focus Back to game, Enter → closed.
- `the_settings_over_the_pause_menu_leave_no_hotbar_beneath_their_hints` (parked b) — 800×600: Escape, click Settings → `HintBar` visible and no visible `Hotbar` or `InventoryCell` whose rect intersects it (`HudBottom` hidden); Escape, Escape → `HudBottom` visible.
- `the_pause_menu_resumes_saves_and_leaves` (moved) keeps its journey with the new labels; its check that the overlay keeps the pointer from the button beneath becomes: hover `"Save now"` with the settings open → its `BackgroundColor == RAISE` (not `RAISE_HOVER`).
- `the_pause_menu_of_a_server_disconnects_back_to_its_card` (the server journey) — `spawn_test_server("Local test")`, `with_servers(&[("Local test", "127.0.0.1:{port}")])`, join → `GameState::Game`; Escape → `"Disconnect"` shown, no `"Save now"`, no `"Save and quit to title"`; click `"Disconnect"` → `MenuState::Multi`, `selected_server == Some("Local test")`, `focus_label() == Some("Local test")`, no `Role::Dialog`; the server thread finishes within `SERVER` (our `Exit` stopped it) and is joined.
- Every other e2e locator of the old pause menu (`rg -n '"Resume"|"Back to menu"|text\("Save"\)' client/src/e2e`) moves to `L::role(Role::Button, "Back to game" | "Save and quit to title" | "Save now")`: `escape_closes_the_inventory`, `typing_in_the_chat_keeps_the_game_keys_quiet`, `the_click_that_closes_a_ui_does_not_reach_the_world` (its `hold_click` on Back to game), `continue_plays_the_last_world_by_ctrl_enter_and_by_click`, `review.rs`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- menus::pause menus::settings e2e::pause` → compile errors, then no `"Back to game"`.

- [ ] **Step 3: Implement** as in Interfaces and Decisions 1–4.

- [ ] **Step 4: Run the tests and the suite** → PASS; `rg -n "Interaction\b|PauseButtonAction|click_sounds" client/src --type rust` finds only `InteractionDisabled` and harness comments; full suite green, no golden changed.

- [ ] **Step 5: Commit** (the report lists the moved tests and the changed locators) — `git commit -m "feat(client): a pause menu on the widgets, keyboard first, with the focus back on Settings"`

---

### Task 2: Save now, and the toast that says the world is saved

**Files:**
- Create: `client/src/ui/widgets/toast.rs`
- Modify: `client/src/ui/widgets/{mod,button}.rs`, `shared/src/lib.rs`, `server/src/world/writer.rs`, `server/src/init.rs` (`WorldWriter::spawn(world_dir, config.saves.clone())`), every `GameServerConfig { .. }` (`rg -n "GameServerConfig \{"`: `saves: None` but the embedded server's), `client/src/network/setup.rs`, `client/src/ui/menus/pause.rs`, `client/src/e2e/{widgets,pause}.rs`

**Interfaces:**
- Produces, in `shared/src/lib.rs`:
  ```rust
  /// The saves of a world its embedded server finished writing, for the client of the same process.
  #[derive(Clone, Default, Debug)] pub struct SaveCounter(Arc<AtomicU64>);
  impl SaveCounter { pub fn get(&self) -> u64; pub fn bump(&self); }
  pub struct GameServerConfig { pub world_name: String, pub is_solo: bool, pub saves: Option<SaveCounter> }
  ```
- Produces, in `server/src/world/writer.rs`: `pub fn spawn(world_dir: PathBuf, saves: Option<SaveCounter>) -> std::io::Result<Self>` — the thread bumps `saves` after each job written without a failed chunk.
- Produces, in `network/setup.rs`: `#[derive(Resource, Clone)] pub struct LocalSaves(pub SaveCounter);` inserted by `launch_local_server_system`, whose server gets `saves: Some(..)`.
- Produces, in `widgets/toast.rs`:
  ```rust
  pub const TOAST_DURATION: Duration = Duration::from_secs(3);
  pub const TOAST_Z: i32 = 40;
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum ToastKind { Info, Error }
  /// A toast, despawned once `Time<Real>` reaches `until`.
  #[derive(Component)] pub struct Toast { pub until: Duration }
  pub fn show_toast(commands: &mut Commands, text: &str, kind: ToastKind, font: Handle<Font>) -> Entity;
  pub(super) fn plugin(app: &mut App);
  ```
  Look and placement as Decision 6; the stack is one absolute column spawned on first use, without `DespawnOnExit`.
- Produces, in `widgets/button.rs`: `pub fn set_label(label: &str) -> impl EntityCommand` — the button's `AccessibleLabel` and its `Text` child.
- Produces, in `pause.rs`:
  ```rust
  pub const SAVING: &str = "Saving...";
  pub const WORLD_SAVED: &str = "World saved";
  pub const SAVE_FAILED: &str = "Could not save the world";
  pub const SAVE_TIMEOUT: Duration = Duration::from_secs(10);
  /// A save asked by "Save now": the counter when it was asked, and when.
  #[derive(Resource)] pub struct SaveNow { pub from: u64, pub since: Instant }
  /// `Some(true)` once the counter passed `from`, `Some(false)` after SAVE_TIMEOUT, else `None`.
  pub fn save_outcome(from: u64, now: u64, elapsed: Duration) -> Option<bool>;
  fn watch_save_now(...);  // Update, Game: toast, Save now relabelled and enabled again, SaveNow removed
  ```
  `SaveNow` relabels its button `SAVING` and inserts `InteractionDisabled`; it is removed `OnExit(GameState::Game)`.

- [ ] **Step 1: Write the failing tests**

Unit:
- `pause.rs`: `a_save_is_told_once_written_or_after_ten_seconds` — `(5, 5, 1 s) → None`; `(5, 6, 1 s) → Some(true)`; `(5, 5, 10 s) → Some(false)`; `(5, 6, 11 s) → Some(true)`.
- `writer.rs`: `a_written_save_is_counted_and_a_failed_one_is_not` — `spawn(dir, Some(counter))`, submit a job → wait until `counter.get() == 1`; then a job whose chunk cannot be written (the existing `0_0_0.bin.tmp` directory trick), `close_for_test` → still `1`.
- `toast.rs`: `a_toast_has_the_role_of_its_kind` — Info → `Role::Status`, Error → `Role::Alert`, labelled with the text.

E2e:
- `e2e/widgets.rs`: `a_toast_shows_top_right_and_goes` — a probe `button("Under")` at the top-right, `show_toast("Hello", Info)` → visible, its rect's right edge `SPACE_L` from the window's and its top `SPACE_L` from the top (logical, × `UiScale`); a second toast below the first, not overlapping; `click` at the first toast's centre → the probe's `Activate` fires (the toast takes no click); set the first's `until` to `Duration::ZERO` → gone next frame, the second stays.
- `e2e/pause.rs`: `save_now_says_when_the_world_is_saved` — in `the_pause_menu_resumes_saves_and_leaves`, the Save step: click `"Save now"` → its label `SAVING` and `InteractionDisabled`; `L::role(Role::Status, WORLD_SAVED)` within `SERVER`; then the player file exists and `world.ron` is newer; the button reads `"Save now"`, enabled, focused; Enter → a second toast.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test --workspace -q -- menus::pause world::writer widgets::toast e2e::widgets::a_toast e2e::pause` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces and Decisions 5–6.

- [ ] **Step 4: Run the tests and the suite** → PASS; the server's save tests unchanged; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): Save now says when the world is written, in a toast"`

---

### Task 3: Save and quit without freezing the game

**Files:**
- Create: `client/src/network/leave.rs`
- Modify: `client/src/network/{mod,cleanup,session}.rs`, `client/src/ui/menus/pause.rs`, `client/src/e2e/pause.rs`

**Interfaces:**
- Consumes: `LocalServerHandle`, `join_local_server`, `watch_session`, `PauseButton`, `set_label`, `SAVING`.
- Produces, in `network/leave.rs`:
  ```rust
  pub const STOP_TIMEOUT: Duration = Duration::from_secs(10);
  /// The game is being left on purpose: Exit was sent, the game waits for the local server to stop.
  #[derive(Resource, Debug)] pub struct Leaving { pub since: Instant }
  #[derive(Clone, Copy, PartialEq, Eq, Debug)] pub enum LeaveStep { Wait, Stopped, GaveUp }
  pub fn leave_step(server_running: bool, elapsed: Duration) -> LeaveStep;   // Stopped wins over GaveUp
  /// Asks to leave the game for the title once the world is saved.
  #[derive(Event)] pub struct LeaveGame;
  fn start_leaving(...);   // observer: sends Exit now (send_game_message + send_packets), inserts Leaving and ExitSent; no local server → GameState::Menu at once; does nothing while Leaving exists
  fn finish_leaving(...);  // Update, Game, resource_exists::<Leaving>: Stopped | GaveUp → GameState::Menu
  pub fn leaving(leaving: Option<Res<Leaving>>) -> bool;
  pub fn leave_plugin(app: &mut App);   // added by network_plugin; OnExit(Game) removes Leaving
  ```
- Produces, in `network/cleanup.rs`: `/// Exit reached the local server of an ended session: its thread is waited for, never connected to again.` `#[derive(Resource)] pub struct ExitSent;` `stop_session` no longer waits: it joins a finished thread, keeps a running one (inserting `ExitSent` when `Exit` was sent); `finish_stopping_local_server` with `ExitSent` only joins the thread once finished, then removes `ExitSent`. `close_session_on_app_exit` keeps `wait_for_local_server(.., LOCAL_SERVER_STOP_TIMEOUT)`.
- Changes: `watch_session` and `end_session` do nothing while `Leaving` exists; the pause menu's `SaveAndQuit` triggers `LeaveGame`; while `leaving`, every `PauseButton` has `InteractionDisabled`, `SaveAndQuit` reads `SAVING`, the pause `HintBar` is hidden and `toggle_pause_menu` ignores Escape.

- [ ] **Step 1: Write the failing tests**

Unit:
- `leave.rs`: `leaving_waits_for_the_server_then_gives_up` — `(true, 1 s) → Wait`; `(false, 1 s) → Stopped`; `(true, 10 s) → GaveUp`; `(false, 11 s) → Stopped`.
- `leave.rs`: `leaving_never_blocks_a_frame` — bare `App`, states in `Game`, `leave_plugin`, `LocalServerHandle` of a thread parked on a channel, `Leaving { since: now }`: three `update`s each return within 50 ms and the state stays `Game`; release the thread → `update_until` `State == Menu`. Again with `since` 11 s back → `Menu` while the thread is parked, the handle kept; release.
- `session.rs`: `saving_and_quitting_is_never_an_error` (Review Focus 1) — `watch_session` with a disconnected `RenetClient` and `Leaving` → no `SessionEnd`, `NextState<GameState>` unchanged.
- `cleanup.rs`: `ending_a_session_never_waits_for_the_local_server` — `stop_session(None, None, Some(&mut LocalServerHandle(Some(parked))))` returns within 50 ms with the handle kept; `finish_stopping_local_server` with `ExitSent`, no client, a `LocalServerAddress`: no `RenetClient` inserted; release → handle `None`, no `ExitSent`. The existing cleanup tests keep passing (`update_until` where they waited inside the call).

E2e, `e2e/pause.rs`: `save_and_quit_shows_saving_until_the_world_is_saved` (parked d; Review Focus 2) — the end of `the_pause_menu_resumes_saves_and_leaves`: click `"Save and quit to title"`; `frames(1)` → `L::text(SAVING)` within `PauseMenu`, `game_state() == Game`, every `PauseButton` has `InteractionDisabled`; Escape and a click on `"Back to game"` → still `PauseMenu` and `SAVING`; `wait_until` the main menu within `SERVER`; `world.ron` newer than at the click; `LocalServerHandle.0.is_none()`; `menu_state() == Main` (no error card). Before this task the label was never drawn: the state changed in the click's frame.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- network:: e2e::pause::save_and_quit` → compile errors, then no `Saving...` frame.

- [ ] **Step 3: Implement** as in Interfaces and Decision 7.

- [ ] **Step 4: Run the tests and the suite** → PASS; `dropping_the_harness_in_game_stops_the_local_server` and every Lot 4 session test still green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): save and quit shows Saving... while the world is written, without freezing the game"`

---

### Task 4: Quitting saves a world first: the window's close button, a world still starting, the main menu's Quit

**Files:**
- Create: `client/src/ui/quit.rs`
- Modify: `client/src/ui/mod.rs`, `client/src/ui/menus/home.rs` (the quit dialog's `Some(1)` triggers `QuitRequested`), `client/src/main.rs` (`WindowPlugin { close_when_requested: false, .. }`), `client/src/e2e/harness.rs` (the same, and `request_close`), `client/src/e2e/input.rs` (`type_on`, used in Task 6), `client/src/e2e/{pause,main_menu}.rs`, `docs/ARCHITECTURE.md` ("Known limits")

**Interfaces:**
- Consumes: `LeaveGame`, `Leaving`, `LeaveStep`, `STOP_TIMEOUT`, `end_session`, `finish_stopping_local_server`, `LocalServerHandle`, `SessionOrigin`, `DIALOG_Z`.
- Produces, in `ui/quit.rs`:
  ```rust
  pub const QUITTING_TITLE: &str = "Saving the world...";
  pub const QUIT_NOW_NOTE: &str = "Close the window again to quit now.";
  /// Asked by the window's close button and the main menu's Quit.
  #[derive(Event)] pub struct QuitRequested;
  #[derive(Resource)] pub struct Quitting { pub since: Instant }
  #[derive(Component)] pub struct QuittingCard;
  /// Whether the app may exit: no session left and no local server running, or STOP_TIMEOUT passed.
  pub fn quit_step(in_session: bool, server_running: bool, elapsed: Duration) -> LeaveStep;
  pub fn spawn_quitting_card(commands: &mut Commands, assets: &AssetServer) -> Entity;
  fn forward_close_requests(...);  // WindowCloseRequested → trigger QuitRequested
  fn on_quit_requested(...);       // observer: Quitting exists → AppExit at once; nothing to save → AppExit; else Quitting + card, and in Game a world → LeaveGame, in PreGameLoading a world → end_session(.., None)
  fn quit_once_stopped(...);       // Update, resource_exists::<Quitting>: quit_step != Wait → AppExit::Success
  pub fn quit_plugin(app: &mut App);
  ```
  "Nothing to save": no `LocalServerHandle` thread running and no world session (`SessionOrigin::World` in `Game` or `PreGameLoading`). A server session quits at once (`close_session_on_app_exit` still sends its `Exit`). The card: full window, `GlobalZIndex(DIALOG_Z + 10)`, `UiDialog` + `Visibility::Visible` (game keys wait), `BackgroundColor(BEVEL_LO.with_alpha(0.7))`, a dialog-like panel with `QUITTING_TITLE` (`FONT_L`) and `QUIT_NOW_NOTE` (`FONT_S`, `MUTED`), `AccessibilityNode(Role::Dialog)` labelled `QUITTING_TITLE`, nothing focusable, no `DespawnOnExit`.
- Produces, in `e2e`: `Harness::request_close(&mut self)` (writes `WindowCloseRequested` for the primary window, runs a frame); `Harness::type_on(&mut self, key: KeyCode, c: char)` (types `c` from the physical `key`, as another layout would).

- [ ] **Step 1: Write the failing tests**

Unit, `quit.rs`:
- `the_app_quits_once_no_world_is_left_to_save` — `quit_step`: `(false, false, 0) → Stopped`; `(true, false, 1 s) → Wait`; `(false, true, 1 s) → Wait`; `(false, true, 10 s) → GaveUp`; `(true, true, 11 s) → GaveUp`.
- `quitting_waits_for_a_world_still_starting` (parked e) — bare `App` in `Menu` with `quit_plugin` and `finish_stopping_local_server`, a `LocalServerHandle` of a parked thread (a world still loading, no connection yet): trigger `QuitRequested` → after 3 updates no `AppExit`, a `QuittingCard`; release → `update_until` an `AppExit`, the handle `None` first.
- `a_second_close_quits_at_once` (Review Focus 6) — `Quitting` present, trigger `QuitRequested` → `AppExit` in that update.

E2e, `e2e/pause.rs`:
- `closing_the_window_in_a_world_saves_then_quits` (parked f; Review Focus 6) — `enter_world`; Escape, click `"Save and quit to title"`, then at once `request_close()` → `L::text(QUITTING_TITLE)` visible, `!has_exited()`; `wait_until(has_exited)` within `SERVER`; then `LocalServerHandle.0.is_none()` and `world.ron` newer than at the click.
- `closing_the_window_while_a_world_starts_waits_for_it` (parked e) — Singleplayer, `play_world("Slow")`, `request_close()` while `game_state() == PreGameLoading` → `QUITTING_TITLE` visible, not exited; `has_exited()` within `SERVER`, and `LocalServerHandle.0.is_none()` at that moment (joined, not killed).

E2e, `e2e/main_menu.rs`: `closing_the_window_in_the_menus_quits_at_once` — `request_close()` → `has_exited()`, no `QuittingCard`; `quit_exits_the_game` unchanged.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::quit e2e::pause::closing e2e::main_menu::closing` → compile errors, then the window closes at once.

- [ ] **Step 3: Implement** as in Interfaces and Decisions 8–9; the "Known limits" line becomes Decision 9's.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): quitting saves a running world first, also while it starts"`

---

### Task 5: The chat log — 100 timed messages above the hotbar, scrolling and fading

**Files:**
- Create: `client/src/e2e/chat.rs` (the chat tests of `e2e/game.rs` move here)
- Modify: `client/src/ui/hud/chat.rs`, `client/src/ui/hud/inventory/setup.rs`, `client/src/ui/hud/hotbar.rs`, `client/src/ui/widgets/list.rs` (`ScrollListParts` gains `pub well: Entity`), `client/Cargo.toml` (`chrono`), `client/src/ui/mod.rs`
- Delete: `client/src/ui/style.rs`

**Interfaces:**
- Produces, in `inventory/setup.rs`: `pub const HOTBAR_TOP: f32` — the top of the closed panel above the window's bottom (its `bottom`, margin, padding, bevels and a slot). In `hotbar.rs`: `pub const ITEM_NAME_BAND: f32` — the height the item name takes above it.
- Produces, in `chat.rs`:
  ```rust
  pub const CHAT_MAX_MESSAGES: usize = 100;
  pub const CHAT_WIDTH: f32 = 420.0;
  pub const CHAT_LOG_LINES: usize = 10;          // lines visible when open, and recent lines at most when closed
  pub const CHAT_FADE_AFTER: Duration = Duration::from_secs(7);
  pub const CHAT_FADE: Duration = Duration::from_secs(1);
  #[derive(Component)] pub struct ChatLog;        // the ScrollArea of the lines
  #[derive(Component)] pub struct ChatLine { pub received: Instant }
  /// `HH:MM` of a UTC timestamp in milliseconds, at `offset`.
  pub fn chat_time(timestamp_ms: u64, offset: chrono::FixedOffset) -> String;
  /// The opacity of a line `age` old over the game: 1 for CHAT_FADE_AFTER, then down to 0 over CHAT_FADE.
  pub fn line_alpha(age: Duration) -> f32;
  ```
  Layout as Decisions 10–11; the root keeps `ChatRoot`, `UiDialog`, `TabGroup::modal()`; lines are spawned with `TextFont` from `FONT_PATH` at `FONT_S` (no `text_font`). `display_chat_messages_system` keeps the last `CHAT_MAX_MESSAGES` and stays at the bottom when it was there. `fade_chat_messages_system`: closed → the last `CHAT_LOG_LINES` lines younger than `CHAT_FADE_AFTER + CHAT_FADE` shown with `line_alpha`, the others inherited; open, or `UIMode::Opened` → no strips, recent lines hidden while `Opened`. Opening scrolls the log to the bottom; closing too. The wheel scrolls the log under the cursor; PageUp / PageDown on the field scroll it by `CHAT_LOG_LINES - 1` lines.

- [ ] **Step 1: Write the failing tests**

Unit, `chat.rs`:
- `the_chat_keeps_the_last_hundred_messages` (replaces `…_keeps_only_the_latest`) — 102 messages → 100 lines, the first holds `"2"`.
- `a_line_shows_the_local_time_of_its_message` — `chat_time(0, east(3600)) == "01:00"`; `chat_time(13 h 7 min 59.999 s, UTC) == "13:07"`; `chat_time(23 h 30 min, east(3600)) == "00:30"`.
- `a_line_fades_after_seven_seconds` — `line_alpha`: `0 s → 1`, `6.9 s → 1`, `7.5 s → 0.5`, `8 s → 0`, `60 s → 0`.
- `a_closed_chat_shows_only_its_recent_lines` (Review Focus 4) — bare `App`, root hidden, `UIMode::Closed`: 12 lines received now → the last 10 `Visible`; one 9 s old → inherited; `UIMode::Opened` → none `Visible`.

E2e, `e2e/chat.rs`:
- `the_chat_keeps_its_history_with_times_and_scrolls` (e2e defect "keeps only the last 2") — send `"one"`, `"two"`, `"three"` (T, type, Enter, wait each) → T: the three shown, each line's first text matches `^\d\d:\d\d$`; Escape; push 30 messages `"line 0"`…`"line 29"` into `CachedChatConversation.unread` (incoming server messages); T → `"line 29"` visible, `"one"` not; hover the log, `scroll(5.)` → `"line 29"` moved down; PageUp → `"line 10"`-ish lines shown (`"line 29"` not visible); PageDown ×3 → `"line 29"` visible.
- `the_chat_sits_above_the_hotbar_and_its_item_name` (parked c) — at 800×600 and 1920×1080: press `Digit2` (the item name shows), T → the rects of `ChatRoot`, `HotbarItemName` and `Hotbar` do not intersect pairwise, the chat's bottom is at least `SPACE_S × UiScale` above the item name's top and inside the window.
- `the_chat_sends_a_message_and_escape_closes_it`, `typing_in_the_chat_keeps_the_game_keys_quiet`, `clicking_outside_the_chat_closes_it`: moved, unchanged.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::chat e2e::chat` → compile errors, then two lines kept.

- [ ] **Step 3: Implement** as in Interfaces; `style.rs` and `pub mod style` go.

- [ ] **Step 4: Run the tests and the suite** → PASS; `rg -n "text_font|ui::style|MessageAnimator|CHAT_COLOR" client/src` finds nothing; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a chat of 100 timed messages above the hotbar, that scrolls and fades"`

---

### Task 6: The chat field — Up/Down recall, and `/` opens it with a slash

**Files:**
- Modify: `client/src/ui/hud/chat.rs`, `client/src/ui/hud/mod.rs`, `client/src/e2e/chat.rs`

**Interfaces:**
- Produces, in `chat.rs`:
  ```rust
  /// The messages sent in this game, oldest first, and where Up/Down are among them. On the chat root.
  #[derive(Component, Default)] pub struct SentMessages { sent: Vec<String>, recalled: Option<usize>, draft: String }
  impl SentMessages {
      pub const MAX: usize = 50;
      pub fn record(&mut self, message: &str);              // not a repeat of the last one; ends the recall
      pub fn older(&mut self, current: &str) -> Option<&str>; // Up: keeps `current` as the draft on the first step
      pub fn newer(&mut self) -> Option<&str>;               // Down: past the newest, the draft
      pub fn end_recall(&mut self);
  }
  fn recall_on_arrows(...);  // observer on the field: FocusedInput<KeyboardInput>, ArrowUp / ArrowDown pressed → set_text, caret at the end
  fn open_chat_on_slash(...);// Update, Game, in_gameplay: a KeyboardInput released with logical Key::Character("/") → open, field "/", caret at the end
  ```
  Sending records the message; `close_chat_on` ends the recall.

- [ ] **Step 1: Write the failing tests**

Unit, `chat.rs`: `up_and_down_walk_the_sent_messages` — record `"a"`, `"b"`, `"b"` → `older("dr") == Some("b")`, `older == Some("a")`, `older == Some("a")`, `newer == Some("b")`, `newer == Some("dr")`, `newer == None`; recording 60 → the oldest kept is the 11th.

E2e, `e2e/chat.rs`: `up_and_down_recall_sent_messages_and_a_slash_opens_the_chat` (parked g; Review Focus 5) — send `"first"`, `"second"`; T, type `"dra"`; ArrowUp → `value_of::<ChatInput>() == "second"`; ArrowUp ×2 → `"first"`; ArrowDown → `"second"`; ArrowDown → `"dra"`; Escape; T → `""`; ArrowUp → `"second"`; Escape. ArrowUp in gameplay → no chat. `type_text("/")` → `ChatRoot` visible, value `"/"`, focus on the field; type `"help"`, Enter → `"/help"` shown. `type_on(KeyCode::Digit7, '/')` (a layout's Shift+7) → opens with `"/"`; Escape. `type_on(KeyCode::Slash, '!')` → stays closed. E → inventory; `type_text("/")` → no chat; E.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- hud::chat e2e::chat::up_and_down` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces and Decisions 14–15.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): recall sent chat messages with Up and Down, and open the chat with a slash"`

---

### Task 7: The whole UI reviewed — every screen at four sizes, one look, no legacy left

**Files:**
- Modify: `client/src/e2e/review.rs`, `client/src/e2e/capture.rs` (focus-ring check), `README.md`, `docs/ARCHITECTURE.md`, `client/src/e2e/input.rs` (its `Interaction` comment), `client/src/ui/widgets/dialog.rs` (`FocusPolicy` goes), plus whatever the review finds, each fix in its owning file with a test first

- [ ] **Step 1: Every focusable control has a ring** — `Harness::assert_focus_rings(&mut self)`: no visible entity with `TabIndex(>= 0)` lacks `FocusRing` (the chat field and text fields included, or their own focus look named). `settled_snapshot` and `snapshot_in_game` call it, so every screen below is checked.

- [ ] **Step 2: Extend the review journey** — `RESOLUTIONS` becomes `[(800, 600), (1280, 720), (1920, 1080), (2560, 1440)]` (Decision 16), which every screen of Lots 1–5 already loops over. In game, per resolution: Escape → `"pause"`; click Settings → `"settings_in_game"`, Escape → `"pause_after_settings"` (the ring on Settings); click Save now → wait the toast → `"toast_saved"`; insert `Leaving { since: now }` → `"pause_saving"`, remove it; Escape. Seed 30 incoming messages, T → `"chat"` (open, scrolled to the bottom); Escape → `"chat_recent"` (closed, fading strips); `type_text("/")` → `"chat_slash"`, Escape. `spawn_quitting_card` → `"quitting"`, despawn it.

- [ ] **Step 3: Run and look** — `UI_SNAPSHOTS=1 timeout 900 cargo test -q -p client review -- --ignored`, then read every `target/ui-snapshots/every_screen_at_three_resolutions/*_{800x600,1280x720,1920x1080,2560x1440}.png`. Check on every screen of every lot: it fits 800×600 with its hint bar clear of it; texels and icons whole at 1.5 and 2 (icons 48 and 64 px); nothing overflows its panel or button; in game: the pause panel centred with no hotbar beneath its hints, the toast top-right under nothing, the chat clear of the hotbar and the item name, the quitting card centred.

- [ ] **Step 4: Consistency pass** — fix each finding with a test first, in its owning file:
  - Spacing: `rg -n "px\((\d|\-)" client/src/ui` — every literal is a token of `theme.rs` or a named size constant with a doc comment (panel widths, `SLOT_SIZE`, `CHAT_WIDTH`, …).
  - Type: `rg -n "font_size" client/src/ui` — only `FONT_S/M/L/XL`.
  - Colours: `rg -n "Color::(srgb|srgba|WHITE|BLACK)|palettes::" client/src/ui` — only `theme.rs` (and the alpha of a token, e.g. `BEVEL_LO.with_alpha(..)`); `ACCENT` only on focus rings, selections and slider fills.
  - Hints: list every `Hint { keys, action }` (`rg -n "keys:" -A1 client/src/ui`) — the same key is spelled the same everywhere (`Esc`, `Enter`, `Tab`, `Del`, `Ctrl+N`, `F5`, `↑↓`, `←→`), the same action the same verb (`Move`, `Select`, `Back`, `Cancel`); the snapshots show the bar's hints match what the keys do on that screen.
  - Focus: Step 1 green on every screen; every screen opens with the focus on its first useful control (spec Goal 1).

- [ ] **Step 5: Legacy left behind** —
  - `rg -n "Interaction\b|prelude::Button|bevy::ui::Button|click_sounds|text_font|ui::style|FocusPolicy|PauseButtonAction|render_pause_menu|MenuButtonAction|button_system|ScrollingList|mouse_scroll|fill_image_node|load_button_background|load_dark_button|NORMAL_BUTTON" client/src` finds only `InteractionDisabled`. (`FocusPolicy` only ever affected `Interaction`: the dialog backdrop's goes.)
  - `ls data/graphics` holds `background.png`, `title.png` and the block textures only; `rg -n "RustcraftBold|RustcraftItalic|BoldItalic" -g '!target'` over the repository: an unreferenced font of `data/fonts` is deleted, a referenced one is named in the report with its user.
  - Clippy without any `dead_code` allowance on a UI item proves every item left has a user.

- [ ] **Step 6: Positions from the window** — `rg -n "cursor_position|world_to_viewport" client/src`: every result that ends in a `Val::Px` divides by `UiScale`; fix any that does not, with a test.

- [ ] **Step 7: Docs** — ARCHITECTURE "UI widgets": `toast.rs`, `set_label`; the pause menu (a modal, a world's and a server's buttons, the HUD hidden beneath it and the settings). "Network": leaving without waiting (`LeaveGame`, `Leaving`, `ExitSent`, `STOP_TIMEOUT`), quitting (`QuitRequested`, a world saved first, a second close quits), `SaveCounter` (in-process, no protocol change). "Known limits": Decision 9's line. README "Controls": `T` chat, `/` chat with a slash, `↑`/`↓` in the chat recall sent messages, PageUp/PageDown scroll it, `Esc` game menu; "Saves": "Save now" in the game menu says when the world is written; closing the window in a world saves it first, and closing it again quits at once.

- [ ] **Step 8: The e2e spec's Non-goals, closed** — in the task report, a checklist of every line of `2026-10-18-ui-e2e-tests-design.md` "Non-goals" with the test(s) that cover it, each proved by `rg -n "fn <name>" client/src`. Lot 6's: chat history → `the_chat_keeps_its_history_with_times_and_scrolls`, `the_chat_keeps_the_last_hundred_messages`; keyboard in the pause menu → `the_keyboard_alone_drives_the_pause_menu`; F-keys while the chat or the pause menu is open → `typing_in_the_chat_keeps_the_game_keys_quiet`; pressed look → Lot 0's button test, now the only button kind. A line without a test gets one, failing first, in this task.

- [ ] **Step 9: Full suite** — `timeout 900 cargo test --workspace --no-fail-fast` green and within about 2 minutes more than before the lot; fmt and clippy clean; `git status` shows no `.actual.png` / `.diff.png`.

- [ ] **Step 10: Commit** — `git commit -m "test(client): every screen reviewed at four sizes, the last legacy UI removed, and docs of leaving and the chat"`

- [ ] **Step 11: Report to the owner, unchanged** — (1) the main menu shows `v0.0.0`, read from `client/Cargo.toml`, while the git tags reach `v1.0`: which version should it show? (2) The RustCraft font has no accented letters, so a chat message such as "ça va" shows missing glyphs: keep it, or add a pixel font with Latin-1 coverage for player text?
