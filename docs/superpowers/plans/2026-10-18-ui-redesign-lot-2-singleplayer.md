# UI Redesign Lot 2 (Singleplayer) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the legacy world list with the Singleplayer screen of the spec: a scrollable list of world cards sorted by last played, a detail pane with Play, Rename and Delete, a "New world" dialog (Ctrl+N) that validates as you type and writes the world to disk at once, a rename dialog, a delete confirmation, an empty state and key hints, all usable with the keyboard alone or the mouse alone.

**Architecture:** What touches the disk or decides a name (listing the worlds with their size, creating, renaming, deleting, checking a name or a seed) moves into plain, unit-tested functions in `ui/menus/worlds.rs`, which the main menu's Continue card already reads, so the list and the card share one source. Two widgets grow: `dialog.rs` gets form dialogs (fields, a submit that the caller confirms, Escape in a field closing the dialog) and `text_field.rs` an opt-in reserved error line; one widget is new, `list.rs` (a scroll list with its scrollbar, extracted from the controls tab, and selectable cards with a roving tab stop). The screen in `ui/menus/solo/` only assembles them and rebuilds its cards from disk after every change.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate, ScrollArea, ScrollIntoView, Scrollbar, ScrollbarThumb}`, `bevy::ui::{Selected, InteractionDisabled}`, `bevy::input_focus`, `bevy::text::{EditableText, EditableTextFilter, TextEdit, TextEditChange}`, `bevy::ui_widgets::SelectAllOnFocus`), `accesskit` 0.24, RON 0.6, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md` — only "Lot 2 — Singleplayer", the "Singleplayer" paragraph of "Screens" and decision 4 ("Continue"); "Visual system", "Architecture", "Testing" and the anti-goals bind every task. Defects to fix: `docs/superpowers/specs/2026-10-18-ui-e2e-tests-design.md`, "Non-goals" (the world-list lines). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md` ("notes §n"). Conventions and products: the Lot 0, Lot 5 and Lot 1 plans next to this file.

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `fix(client): …`, `test(client): …`, `refactor(client): …`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::solo`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift, which runs no destructor while a panic unwinds: a failing test's harness is never dropped. Do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`. Widget probes live in `client/src/e2e/widgets.rs` (start from `main_menu()`, probes under a `GlobalZIndex(1)` root, driven only through `click`/`hover`/`press`/`type_text`/`scroll`). Singleplayer tests go in `e2e/solo.rs`. Every `enter_world` starts an integrated server: this lot adds **one** server journey (Task 5's keyboard journey, which replaces `playing_a_world_connects_then_enters_the_game`), and chains its checks.
- Never a bare `git stash`; set work aside with a WIP commit.
- `bevy::prelude::Button` is the legacy widget. Import the headless one as `use bevy::ui_widgets::Button as UiButton;`. The new screen uses no `Interaction`. Put the widget component or the explicit `AccessibilityNode(accesskit::Node::new(Role::…))` **before** `AccessibleLabel` in a bundle (notes §5).
- A widget item no screen uses yet allows `dead_code` on the item with a comment naming its first user; the allowance goes in the task that uses it. Never crate-wide.
- **Escape.** A dialog consumes Escape through `FocusedInput<KeyboardInput>` and closes in `PostUpdate`; a text field consumes it and reports `Cancel` in `PostUpdate`, which a form dialog turns into its close. The handlers that read `ButtonInput<KeyCode>` in `Update` (`escape_button`, whose Solo arm stays the screen's Escape, and the Ctrl+N opener) skip while `dialog_open` or `text_field_focused` holds. Nothing else in this lot reads Escape.
- **Fonts.** The RustCraft font has no `← ↑ → ↓`, `…`, `×`, `−` and no `é` (checked with `fc-query`: it covers ASCII without the backtick, a few Latin-1 letters, `·` U+00B7 and `—`). Arrows appear only in key caps (Fira Mono). `·` and `"` may be used in copy.
- **World files.** The client writes `world.ron` only through `server::write_meta` (atomic). Creating never writes into an existing folder (`fs::create_dir`, not `create_dir_all`, for the world's own folder). Renaming rewrites only the `name` of a `world.ron` it can parse as `WorldMeta`, keeps its `format_version` as read and puts its modification time back (renaming is not playing). An unreadable `world.ron` is never rewritten.
- **The list is the disk.** After every create, rename and delete the cards are rebuilt from `list_worlds`, which `last_played_world` (the Continue card) also reads: the first card is always the Continue world.
- **Ownership.** Lot 3 owns `multi.rs`, `list_screen.rs` (Task 7 only drops what Solo alone used and the Escape stopgap), `spawn_row`, `ScrollingList`, `mouse_scroll`, `button_system`, `menu_action` and `MenuButtonAction::BackToMainMenu`, `escape_button`'s Multi arm, `style.rs`, the stone-button images, `graphics/play.png` and `graphics/trash.png`, and the e2e helpers `wait_for_icons` and `play_rows`. They stay. `settings/` is touched only by Task 3, as a pure refactor.
- **Goldens.** `menu_solo` changes in Task 4 (interim: the field gives way to a "New world" button) and Task 5; `menu_solo_empty` is new in Task 5. Each is regenerated with `UPDATE_GOLDEN=1` only after reading the `.actual.png`. Goldens hold nothing time-dependent: worlds are seeded with fixed ages in whole hours or days, the cursor is moved to a corner with `hover_at`, no text field has the focus. `menu_controls` must stay byte-identical in Task 3 and `menu_multi` throughout (that is why the reserved error line is opt-in). Any other golden that changes is a bug.

## Decisions (where the spec is silent)

1. **Our own list, not `ListBox`.** Decided because `ListBox` selects on Enter (we play), wraps its arrows, and reports a click and Enter as the same `ValueChange`. Cards use a roving tab stop: only the selected card has `TabIndex(0)`, the others `TabIndex(-1)` (a click still focuses them: `acquire_focus` takes any `TabIndex`), so Tab crosses the list in one stop, like a browser list box.
2. **Selection follows the focus.** A click or ↑/↓/Home/End on a card focuses and selects it; Enter on the focused card plays, Del asks to delete it. Play, Rename and Delete in the detail pane act on the selected world, which stays selected while the focus is on them. No double-click: Play is one click away and picking gives no double-click event to rely on.
3. **Names.** Trimmed before they are checked and used (a trailing space makes a folder Windows cannot open), ASCII only (the font lacks most accented letters; the old message said "letters", which `é` contradicted), compared without case against the other worlds (Windows and macOS folders ignore case, so "survie" would land in "Survie"). The name field opens filled with a free name ("New world", "New world 2", …) entirely selected (`SelectAllOnFocus`), so Ctrl+N then Enter makes a world and typing replaces the name; "Enter a name" shows only once the field has been emptied.
4. **Seed.** Digits only (`EditableTextFilter`), at most 10, a `u32`; empty means random (`rand::random`). Decided because `WorldSeed` is a `u32` and a typed number then reads back unchanged on the card.
5. **Order.** By last played, newest first, ties by name ascending (the list reads alphabetically then). The Continue card is the first entry, so the existing tie test changes its expected world from "Charlie" to "Alpha" — its intent (one folder, one answer) is kept. A created world counts as just played (decision 4: the last modified `world.ron`); a renamed one keeps its time.
6. **Card and pane.** The card shows the spec's five facts: a 40 px thumbnail, the name, `Day 4 · Seed 7`, and on the right `played 2 hours ago` over `12 KB`. A world whose `world.ron` cannot be read shows `Cannot read world.ron` (`DANGER`) in place of day and seed. The detail pane (280 px) repeats them as a 64 px thumbnail, the name, `Day 4`, `Seed 7`, `played 2 hours ago`, `12 KB on disk`, then Play (Primary), Rename, Delete (Danger). Sizes read like a file manager: kilobytes rounded up under 1 MiB, one decimal for MB and GB.
7. **Title and frame.** The title is "Worlds" (not "Singleplayer", which the tests use to find the main menu), "New world" sits on the right of the title bar, "Back" bottom-left as in the settings. Empty state: the list area says "No worlds yet" and "Create one with New world (Ctrl+N)."; the detail pane is hidden and New world has the focus.
8. **Dialogs.** New world: title "New world", fields "Name" and "Seed (optional)" (placeholder "Random"), actions Cancel, Create. Rename: title `Rename "Survie"`, field "Name" filled with the current name, actions Cancel, Rename (disabled while the name is unchanged, without a message). Delete: title `Delete "Survie"?`, body `Its 12 KB of saves will be deleted for good.`, actions Cancel (focused), Delete (Danger). A failure (disk full, permission) shows under the name field and keeps the dialog open, or for Delete under the detail pane's buttons; never only in the log.
9. **Hints per state.** A card focused: `↑↓ Select`, `Enter Play`, `Del Delete`, `Ctrl+N New world`, `Esc Back`. Anything else focused, or the empty state: `Tab Move`, `Enter Select`, `Ctrl+N New world`, `Esc Back`. A form dialog: `Tab Move`, `Enter Create` (or `Rename`), `Esc Cancel`. The delete dialog keeps the widget's own hints.
10. **The Task 3 stopgap.** With Solo off `list_screen.rs`, its Escape stopgap only serves Multiplayer: Escape in a server field now leaves the field (the focus is cleared) and a second Escape leaves the screen, instead of jumping to the main menu with a half-typed server.

## Review Focus

1. A name equal to another world's but for case or spaces (`"SURVIE "` next to "Survie") is refused as existing, and creation never writes into a folder that exists: on a case-insensitive disk it would overwrite that world. (Task 1: `world_names_are_checked_and_trimmed`, `creating_never_writes_into_an_existing_folder`)
2. Renaming a world saved in format 1, or one whose `world.ron` is unreadable, keeps its version and leaves the file alone, and keeps its last played time. (Task 1: `renaming_keeps_the_format_version_and_leaves_an_unreadable_world_ron_alone`, `renaming_moves_the_folder_and_the_name_and_keeps_the_played_time`)
3. The Enter that opens the world list from the main menu does not also play the first world; one Enter in the dialog creates one world. (Task 5: `the_keyboard_alone_walks_the_worlds_and_plays_one`; Task 4: `ctrl_n_then_enter_creates_one_world_on_disk`)
4. Escape in a field of a dialog closes the dialog and nothing else, on that frame or the next. (Task 2: `escape_in_a_form_field_closes_only_the_dialog`; Task 4: `escape_cancels_the_dialog_and_stays_on_the_list`)
5. Deleting the selected world focuses its neighbour; deleting the last one shows the empty state with New world focused; the Continue card then follows. (Task 5: `delete_asks_first_and_the_focus_moves_to_the_next_world`, `the_continue_card_is_the_first_world_of_the_list`)
6. A 32-character name without spaces stays inside its card and the detail pane at 800×600. (Task 5: `a_long_name_stays_inside_its_card_at_800x600`)

## Defects of the e2e spec fixed here

"Create world" only adds a row in memory (Task 4); errors only logged, validated as you type (Task 4); deleting a world asks no confirmation (Task 5); no Tab or keyboard activation on the world list (Task 5); no hover feedback on the row icons (Task 5); the wheel scrolls every list at once (Task 5). Lot 0 parked items: the page jumps when a field error appears (Task 2); the "letters" message (Task 1); Enter tests do not assert a single add and `FieldError` clearing is untested (Task 4); Escape in the list screen's field goes back to the main menu (Task 7).

## Files

- `client/src/ui/menus/worlds.rs`, `server/src/lib.rs` — the worlds on disk (Task 1).
- `client/src/ui/widgets/{text_field,dialog}.rs` — reserved error line, `set_text`, form dialogs (Task 2).
- `client/src/ui/widgets/list.rs` (new), `client/src/ui/menus/settings/controls.rs` — the list widget (Task 3).
- `client/src/ui/menus/solo/{mod,dialogs}.rs` (`solo.rs` becomes `solo/mod.rs` in Task 4), `client/src/ui/menus/mod.rs`, `client/src/ui/menus/list_screen.rs` — the screen (Tasks 4–7).
- `client/src/e2e/{solo,seed,helpers,widgets,game,main_menu,locator,input,multi,review}.rs`, `client/tests/golden/menu_solo{,_empty}.png`, `README.md`, `docs/ARCHITECTURE.md`.

---

### Task 1: The worlds on disk — listing with size, names, seeds, create, rename, delete

**Files:**
- Modify: `client/src/ui/menus/worlds.rs`, `client/src/ui/menus/solo.rs` (imports the moved items), `client/src/ui/menus/home.rs` (only if the `WorldSummary` change needs it), `server/src/lib.rs` (`pub use world::format::{write_meta, WorldMeta, WORLD_FILE};`)

**Interfaces:**
- Produces, in `worlds.rs` (plain functions; `saves` is `game_folder/saves`):
  ```rust
  #[derive(Clone, Debug)]
  pub struct WorldSummary { pub name: String, pub seed: Option<u32>, pub day: Option<u64>,
                            pub played: SystemTime, pub size: u64 }   // size: bytes of every file of the folder
  /// The folders of `saves` holding a `world.ron`, newest `world.ron` first, ties by name ascending.
  /// Names that are not UTF-8 are skipped; a missing or unreadable `saves` gives none. Creates nothing.
  pub fn list_worlds(saves: &Path) -> Vec<WorldSummary>;
  pub fn last_played_world(saves: &Path) -> Option<WorldSummary>;   // list_worlds(saves).into_iter().next()
  pub fn format_size(bytes: u64) -> String;
  pub const MAX_WORLD_NAME_LENGTH: usize = 32;
  pub const EMPTY_WORLD_NAME: &str = "Enter a name";
  pub const INVALID_WORLD_NAME: &str = "Use only A-Z, a-z, 0-9, spaces, _ and -";
  pub const WORLD_EXISTS: &str = "A world with this name already exists";
  pub const INVALID_SEED: &str = "Use a number from 0 to 4294967295";
  /// The trimmed name, or the message to show. `others` are the names of the other worlds.
  pub fn check_world_name(typed: &str, others: &[String]) -> Result<String, &'static str>;
  pub fn free_world_name(others: &[String]) -> String;            // "New world", then "New world 2", …
  pub fn parse_seed(typed: &str) -> Result<Option<u32>, &'static str>;
  pub fn create_world(saves: &Path, name: &str, seed: u32) -> io::Result<()>;
  pub fn rename_world(saves: &Path, from: &str, to: &str) -> io::Result<()>;
  pub fn delete_world(saves: &Path, name: &str) -> io::Result<()>;
  ```
  `is_valid_world_name`, `is_single_path_component` and the body of `delete_save_files` move here from `solo.rs` (private, except as listed); until Task 4 replaces it, the legacy screen calls `delete_world`, imports the constants, and its `new_world_name` keeps generating a name for an empty field but checks any other with `check_world_name`. `create_world`: creates `saves` if needed, then `fs::create_dir(saves/name)` (`AlreadyExists` if present), then `write_meta(WorldMeta::new(name, WorldSeed(seed), 0, HashMap::new()))`; on a failed write it removes the folder it just made. `rename_world`: both names single components; `AlreadyExists` when `to` exists and is not `from` but for case; `fs::rename`; then, if `world.ron` parses as `WorldMeta`, sets `name`, writes it back with `write_meta` and restores the previous modification time. Comparisons with `others` use `eq_ignore_ascii_case`.

- [ ] **Step 1: Write the failing tests** (unit, `worlds.rs`, with `tempfile::tempdir()` and the existing `save_world` / `write_world_ron_at` helpers; the listing and deletion tests of `solo.rs` move here and call `list_worlds` / `delete_world`)

- `the_worlds_are_listed_by_last_played_then_by_name` — "Old" 5 days ago, "Recent" 2 hours, "Bravo" and "Alpha" at the same time 1 day ago, a folder without `world.ron`, a plain file, a non-UTF-8 folder (`#[cfg(unix)]`) → names `["Recent", "Alpha", "Bravo", "Old"]`.
- `two_worlds_played_at_the_same_time_are_told_apart_by_name` now expects `"Alpha"`; `the_last_played_world_is_the_newest_world_ron` and the other `last_played_world` tests are unchanged.
- `a_missing_or_unreadable_saves_folder_lists_no_world_and_creates_nothing` — missing folder → `[]`, still missing; `saves` being a file → `[]`.
- `the_size_counts_every_file_of_the_world` — `world.ron` of 10 bytes, `chunks/a.bin` 2000, `players/1.ron` 30 → `size == 2040`.
- `sizes_read_like_a_file_manager_says_them` — `0 → "0 KB"`, `1 → "1 KB"`, `1024 → "1 KB"`, `1025 → "2 KB"`, `1_048_576 → "1.0 MB"`, `1_572_864 → "1.5 MB"`, `3 << 30 → "3.0 GB"`.
- `world_names_are_checked_and_trimmed` — others `["Survie"]`: `"  Base  " → Ok("Base")`; `""`, `"   "` → `EMPTY_WORLD_NAME`; `"bad/name"`, `"é"`, `"a.b"`, 33 × `x` → `INVALID_WORLD_NAME`; `"survie"`, `"SURVIE "` → `WORLD_EXISTS`; `"Survie2" → Ok`.
- `a_free_name_is_offered` — `[] → "New world"`; `["new world"] → "New world 2"`; `["New world", "New world 2"] → "New world 3"`.
- `a_seed_is_a_number_or_nothing` — `"" → Ok(None)`; `"0" → Ok(Some(0))`; `"4294967295" → Ok(Some(u32::MAX))`; `"4294967296"`, `"12a"` → `Err(INVALID_SEED)`.
- `creating_a_world_writes_its_world_ron_at_once` — `saves` missing; `create_world(saves, "Nouveau", 7)` → `world.ron` parses as `WorldMeta` with `format_version == 2`, `name == "Nouveau"`, `seed.0 == 7`, `time == 0`, no mobs; `list_worlds` gives it with `day == Some(1)`, `seed == Some(7)`.
- `creating_never_writes_into_an_existing_folder` — "Survie" with `world.ron` = `"precious"` → `create_world(.., "Survie", 1)` is `Err` of kind `AlreadyExists`, contents still `"precious"`; `".."` and `"a/b"` → `Err` of kind `InvalidInput`.
- `renaming_moves_the_folder_and_the_name_and_keeps_the_played_time` — "Survie" saved 3 days ago with `chunks/0_0_0.bin` → `rename_world(.., "Survie", "Base")`: no `Survie/`, `Base/chunks/0_0_0.bin` there, `Base/world.ron` has `name == "Base"`, and `list_worlds(..)[0].played` equals the `played` read before.
- `renaming_keeps_the_format_version_and_leaves_an_unreadable_world_ron_alone` — a `world.ron` with `format_version: 1` → still 1 after a rename; one holding `"garbage"` → moved, still `"garbage"`.
- `renaming_onto_another_world_is_refused_but_a_change_of_case_is_not` — "A" and "B": `rename_world(.., "A", "B")` → `AlreadyExists`, both intact; `rename_world(.., "A", "a")` → `Ok`, listed as "a".

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- menus::worlds menus::solo` → compile errors (no `list_worlds`, no `size`).

- [ ] **Step 3: Implement** as in Interfaces. `dead_code` allowances on `check_world_name`, `free_world_name`, `parse_seed`, `create_world` ("the New world dialog, Task 4"), `rename_world` ("the rename dialog, Task 6"), `list_worlds`'s `size` field ("the world cards, Task 5") as needed.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green; no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): list, create, rename and delete worlds on disk, with their size"`

---

### Task 2: Form dialogs, and a field error that moves nothing

**Files:**
- Modify: `client/src/ui/widgets/text_field.rs`, `client/src/ui/widgets/dialog.rs`, `client/src/ui/hud/chat.rs` and `client/src/ui/menus/list_screen.rs` (one new field each, `reserve_error: false`), `client/src/e2e/widgets.rs`

**Interfaces:**
- Produces, in `text_field.rs`: `TextField` gains `pub reserve_error: bool` — "keeps one line under the field for its error, so that an error appearing moves nothing below"; when true the message node keeps its line (`min_height` of one `FONT_S` line) and toggles `Visibility::Hidden` / `Inherited` instead of `Display::None`. `#[derive(Clone, Copy)]` on `TextFieldParts`. `pub fn set_text(text: &mut EditableText, value: &str)` — `queue_edit(TextEdit::SelectAll)` then `queue_edit(TextEdit::Insert(value.into()))` (notes §3).
- Produces, in `dialog.rs`:
  ```rust
  pub struct DialogParts { pub root: Entity, pub actions: Vec<Entity> }
  /// Triggered on the root of a form dialog when Enter is pressed in one of its fields or its
  /// primary (last) action is activated, unless that action has `InteractionDisabled`. The dialog
  /// stays open: the caller closes it with `close_dialog` once the submission succeeded.
  #[derive(EntityEvent)] pub struct DialogSubmit { pub entity: Entity }
  /// A dialog whose `fields` (caption, field) sit between its body and its actions. The first
  /// field has the focus (`dialog.focused` is not used); `hints` replace the dialog's own hints.
  pub fn spawn_form_dialog(commands: &mut Commands, dialog: &Dialog, fields: &[(&str, TextFieldParts)],
                           hints: &[Hint], font: Handle<Font>, focus: &InputFocus) -> DialogParts;
  /// Closes the dialog in `PostUpdate` with `DialogClosed { action: Some(last) }`; the focus goes
  /// back where it was only when `restore_focus` (the caller may have focused something new).
  pub fn close_dialog(commands: &mut Commands, root: Entity, restore_focus: bool);
  ```
  `spawn_dialog` and `spawn_form_dialog` share one private builder; `spawn_dialog`'s behaviour is unchanged. In a form dialog: each field is a column (caption `FONT_S` `MUTED`, then the field's `root`, `row_gap: SPACE_XS`) in a column with `row_gap: SPACE_M`; `AutoFocus` and the backdrop-click fallback go to `fields[0].1.input`; a field's `Submit` and the primary's `Activate` trigger `DialogSubmit` (checked against `InteractionDisabled` on the primary); a field's `Cancel` closes with `None`; the other actions close with `Some(i)` as before. `CloseRequest` carries the `restore_focus` flag.

- [ ] **Step 1: Write the failing tests** (e2e, `e2e/widgets.rs`; `form_probe(h)` spawns a probe `button("Before")` with `AutoFocus`, then from a system `spawn_form_dialog(Dialog { title: "Probe form", body: "", actions: [("Cancel", Default), ("Create", Primary)], focused: 0 }, [("Name", name field), ("Seed", seed field)], [Tab Move, Enter Create, Esc Cancel])` with `reserve_error: true` fields, and observers recording `DialogSubmit` counts and `DialogClosed` actions)

- `a_form_dialog_focuses_its_first_field_and_submits_once_per_enter` — `focus_label() == Some("Name")`; `type_text("abc")`, `press(Enter)`, `frames(3)` → 1 submit, the dialog still open; `press(Tab)` → `"Seed"`; Enter → 2; click `L::role(Role::Button, "Create")` → 3.
- `a_disabled_primary_blocks_the_submit` — insert `InteractionDisabled` on `actions[1]`: Enter in the field and a click on Create → 0 submits.
- `escape_in_a_form_field_closes_only_the_dialog` — `press(Escape)` → closes `[None]`, no `DialogRoot`, `focus_label() == Some("Before")`; after 2 more frames the probe button is still there and still focused.
- `close_dialog_can_leave_the_focus_to_the_caller` — a `DialogSubmit` observer that focuses a probe `"After"` and calls `close_dialog(root, false)`: Enter → no `DialogRoot`, `focus_label() == Some("After")`; a second form closed with `true` → focus back on `"Before"`.
- `a_form_dialog_shows_its_own_hints` — `L::text("Create").within(L::marker::<HintBar>())` visible, `L::text("Select").within(L::marker::<HintBar>())` not.
- `an_error_moves_nothing_below_a_field_that_reserves_its_line` — `y = center_of(Create).y`; `FieldError(Some("Bad"))` on the name input → `"Bad"` visible, Create's `y` unchanged; `FieldError(None)` → hidden, `y` unchanged.
- `set_text_replaces_the_value` — `set_text(.., "New world")` on a field holding `"abc"` → `value == "New world"`.
- The existing `an_error_shows_under_the_field_with_a_red_bevel` keeps passing on a field with `reserve_error: false` (the Lot 0 behaviour, `menu_multi` unchanged).

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client e2e::widgets` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces; `dead_code` on `spawn_form_dialog`, `DialogSubmit`, `close_dialog`, `set_text` with "the New world dialog (Lot 2, Task 4)".

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green, no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): form dialogs, and a field error that moves nothing"`

---

### Task 3: The list widget — a scroll list with its scrollbar, and selectable cards

**Files:**
- Create: `client/src/ui/widgets/list.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/ui/menus/settings/controls.rs` (uses `spawn_scroll_list`; its scrollbar spawning, `style_thumb`, `SCROLLBAR_WIDTH`, `SCROLLBAR_MIN_THUMB` and `RING_ROOM` move to `list.rs`), `client/src/e2e/widgets.rs`

**Interfaces:**
- Consumes: `FocusRing`, `sunken_bevel`, the raised bevel colours, `theme::*`.
- Produces, in `list.rs`:
  ```rust
  pub const SCROLLBAR_WIDTH: f32 = 8.0;
  pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;
  /// The room around the items for their focus ring, which the list would clip.
  pub const RING_ROOM: f32 = FOCUS_RING_WIDTH + FOCUS_RING_OFFSET;
  /// `frame` is the row to place (it grows to fill its parent); `list` is the vertical
  /// `ScrollArea` in a sunken well, padded by `RING_ROOM`, where the caller spawns the items.
  pub struct ScrollListParts { pub frame: Entity, pub list: Entity }
  pub fn spawn_scroll_list(commands: &mut Commands) -> ScrollListParts;
  /// An item of a list of cards, a direct child of `ScrollListParts::list`.
  #[derive(Component)] pub struct ListCard;
  pub fn list_card(label: &str) -> impl Bundle;
  /// Triggered on the list when another card is selected.
  #[derive(EntityEvent)] pub struct CardSelected { pub entity: Entity, pub card: Entity }
  ```
  `spawn_scroll_list` is the well, `ScrollArea` and `Scrollbar` (with its thumb, lightened while hovered or dragged) now built in `spawn_controls_tab`, exactly as they are. `list_card(label)` = `(ListCard, AccessibilityNode(Role::ListItem), AccessibleLabel(label), Selectable, TabIndex(-1), FocusRing, Hovered, Node { border: BEVEL_WIDTH, padding: SPACE_M, column_gap: SPACE_M, align_items: Center }, BackgroundColor(RAISE), raised bevel)`. Global observers: a `ListCard` gaining the focus gets `Selected` and `TabIndex(0)`, its sibling cards lose them, `ScrollIntoView` is triggered on it and `CardSelected` on its parent; ↑/↓ on a focused card focus the previous/next card in child order, Home/End the first/last, no wrap (`FocusCause::Navigated`, `InputFocusVisible(true)`, `propagate(false)` only when it moved). Style system: `RAISE_HOVER` while hovered; a `Selected` card has a 2 px `ACCENT` border instead of its bevel. The caller gives the list its role (`AccessibilityNode(Role::ListBox)` and a label).

- [ ] **Step 1: Write the failing tests**

Unit, `list.rs`: `a_card_is_a_list_item_with_its_label` (bare `World`: `Role::ListItem`, label `"Probe"`, `TabIndex(-1)`).

E2e, `e2e/widgets.rs` (`card_probe(h)`: a 300×200 group holding `spawn_scroll_list` with 12 `list_card("Card n")` of height 48 and a probe button `"After"` below it; an observer recording `CardSelected` card labels):
- `a_card_is_selected_by_click_and_by_arrows` — click `L::role(Role::ListItem, "Card 2")` → only Card 2 has `Selected`, selections `["Card 2"]`; ArrowDown → `"Card 3"`; Home → `"Card 1"`; End → `"Card 12"`, whose rect lies inside the list's; ArrowDown → still Card 12, nothing recorded.
- `the_list_is_one_tab_stop_on_its_selected_card` — after selecting Card 5, Tab → `focus_label() == Some("After")`; Shift+Tab → `"Card 5"`.
- `hovering_a_card_lightens_it_and_the_selected_one_has_an_accent_border` — hover Card 3 → `BackgroundColor(RAISE_HOVER)`; Card 2 selected → its four border sides `ACCENT`.
- `the_wheel_scrolls_the_card_list_under_the_cursor` — hover Card 1, `scroll(-3.)` → the list's `ScrollPosition.y > 0` and the thumb moved down; hover `"After"`, `scroll(-3.)` → unchanged.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::widgets::list e2e::widgets` → compile errors.

- [ ] **Step 3: Implement** `list.rs` and its plugin; switch `spawn_controls_tab` to `spawn_scroll_list` (it inserts `ArrowNav` on `list`). `dead_code` on `list_card`, `ListCard`, `CardSelected` with "the world cards (Lot 2, Task 5)".

- [ ] **Step 4: Run the tests and the suite** → PASS; `e2e::settings` green, `menu_controls` unchanged (the refactor proves itself there); full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a list widget with its scrollbar and selectable cards"`

---

### Task 4: The New world dialog — validated as you type, written to disk at once

**Files:**
- Create: `client/src/ui/menus/solo/dialogs.rs`
- Move: `client/src/ui/menus/solo.rs` → `client/src/ui/menus/solo/mod.rs`
- Modify: `client/src/ui/menus/solo/mod.rs`, `client/src/ui/menus/mod.rs` (`solo::solo_plugin(app)`), `client/src/e2e/solo.rs`, `client/src/e2e/input.rs`, `client/src/e2e/review.rs`, `client/tests/golden/menu_solo.png`

**Interfaces:**
- Consumes: `spawn_form_dialog`, `DialogSubmit`, `close_dialog`, `set_text`, `spawn_text_field` (`reserve_error: true`), `check_world_name`, `free_world_name`, `parse_seed`, `create_world`, `MAX_WORLD_NAME_LENGTH`, the messages.
- Produces, in `dialogs.rs`:
  ```rust
  pub const NEW_WORLD_TITLE: &str = "New world";
  #[derive(Component)] pub struct WorldNameInput;      // moved here; on the name field's input
  #[derive(Component)] pub struct WorldSeedInput;
  /// The fields of an open world dialog and what they are checked against, on its root.
  #[derive(Component)] struct WorldForm { name: Entity, seed: Option<Entity>, primary: Entity,
                                          others: Vec<String>, current: Option<String> }
  /// Triggered once the worlds on disk changed: the screen lists them again and focuses the card
  /// of `focus` (none: the first card, or New world when there is none).
  #[derive(Event)] pub struct WorldsChanged { pub focus: Option<String> }
  pub fn open_new_world_dialog(commands: &mut Commands, assets: &AssetServer, focus: &InputFocus, others: Vec<String>) -> Entity;
  pub fn open_new_world_on_ctrl_n(...);   // Update, in_state(Solo), not(dialog_open), not(text_field_focused)
  fn validate_world_forms(...);           // Update: on a changed input, FieldError on each field and InteractionDisabled on `primary`
  fn create_world_on_submit(...);         // observer of DialogSubmit on the root
  pub(super) fn plugin(app: &mut App);    // added by `solo_plugin`
  ```
  The dialog as in Decision 8: the name field (`max_characters: Some(MAX_WORLD_NAME_LENGTH)`, `SelectAllOnFocus`, filled with `free_world_name(&others)` by `set_text`), the seed field (`EditableTextFilter` of ASCII digits, `max_characters: Some(10)`, placeholder `"Random"`), hints `[Tab Move, Enter Create, Esc Cancel]`. Validation: the name by `check_world_name` (no message for the unchanged `current` of a rename, but `primary` disabled), the seed by `parse_seed`; `primary` is disabled while either fails. `create_world_on_submit` checks again, calls `create_world(saves, &name, seed.unwrap_or_else(rand::random))`; `Ok` → `WorldsChanged { focus: Some(name) }` and `close_dialog(root, true)`; `Err(e)` → `FieldError(Some(format!("Could not create the world: {e}")))` on the name field, dialog open.
- The legacy screen for this one task: `ListScreen` of Solo has no input and its add button reads `"New world"` (activating it opens the dialog with the names of `WorldList`); a `WorldsChanged` observer adds the legacy row of `focus`. `new_world_name`, `generate_new_world_name`, `create_world` (the legacy one), `create_world_on_submit` (the legacy one) and their test go.

- [ ] **Step 1: Write the failing tests** (e2e, `e2e/solo.rs`; `ctrl_n(h)` = `key_down(ControlLeft)`, `press(KeyN)`, `key_up(ControlLeft)`, then wait for `L::role(Role::Dialog, NEW_WORLD_TITLE)`)

- `new_world_opens_a_dialog_with_a_free_name_ready_to_replace` — folder with "New world": click `"New world"` → the dialog, `focus_label() == Some("Name")`, `value_of::<WorldNameInput>() == "New world 2"`; `type_text("Base")` → `"Base"` (the name was selected).
- `ctrl_n_then_enter_creates_one_world_on_disk` (replaces `enter_in_the_world_name_creates_the_world`) — `ctrl_n`, `type_text("Entered")`, `press(Enter)`, `frames(3)` → no dialog; exactly one `L::name("Play Entered")`; `saves/` holds exactly the three seeded folders and `Entered`; `saves/Entered/world.ron` parses as `WorldMeta` with `name == "Entered"`, `time == 0`.
- `a_typed_name_and_create_add_a_world` keeps its journey: click `"New world"`, `type_text("Nouveau")`, click `L::role(Role::Button, "Create")` → `"Play Nouveau"`, and its folder exists.
- `a_typed_seed_is_kept` — `ctrl_n`, `type_text("Seeded")`, Tab, `type_text("12ab34")` → `value_of::<WorldSeedInput>() == "1234"`; Enter → `world.ron` seed `1234`.
- `the_name_is_checked_as_you_type_and_nothing_moves` (replaces `an_invalid_name_adds_no_world` and `a_duplicate_name_adds_no_world`, journeys kept) — `ctrl_n`, `y = center_of(Create).y`; `type_text("bad/name")` → `INVALID_WORLD_NAME` visible, Create has `InteractionDisabled`; Enter → still open, `play_rows() == 3`; Backspace ×8 → `EMPTY_WORLD_NAME`; `type_text("survie")` → `WORLD_EXISTS`; `type_text("2")` → no message visible (the error cleared), Create enabled; Tab, `type_text("4294967296")` → `INVALID_SEED`, Create disabled; Backspace → enabled; Create's `y` never changed.
- `escape_cancels_the_dialog_and_stays_on_the_list` — `ctrl_n`, Escape → no dialog, `menu_state() == Solo` (and after 2 more frames), no new folder; Escape → `MenuState::Main`.
- `after_a_create_new_world_opens_with_a_fresh_name` (replaces `after_create_a_click_on_the_field_types_again`) — create "One" by click, `ctrl_n` → `value_of::<WorldNameInput>() == "New world"`; `type_text("Two")`, Enter → `"Play One"` and `"Play Two"` once each.
- `e2e/input.rs` `typed_text_goes_to_the_focused_input` — opens the list, `ctrl_n`, `type_text("My world 2")` → `value_of::<WorldNameInput>() == "My world 2"`.
- `the_world_list_matches_its_golden` — no click on the title any more (no field, no caret); `hover_at` a corner. `e2e/review.rs` waits for `"New world"` instead of `"Create world"`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- e2e::solo e2e::input` → compile errors, then no dialog.

- [ ] **Step 3: Implement** as in Interfaces; remove the allowances of Tasks 1 and 2 this task uses.

- [ ] **Step 4: Run the tests; regenerate `menu_solo` on purpose (interim)** — read `menu_solo.actual.png`: the legacy rows, then the "New world" and "Back to menu" buttons where the field and "Create world" were; nothing else moved. `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_world_list_matches_its_golden`, rerun → PASS, no leftovers. Full suite green.

- [ ] **Step 5: Commit** (the report lists the replaced tests and changed locators) — `git commit -m "feat(client): a New world dialog that checks the name as you type and writes the world at once"`

---

### Task 5: The Singleplayer screen — cards, detail pane, delete, keyboard, empty state

**Files:**
- Modify: `client/src/ui/menus/solo/mod.rs` (rewrite), `client/src/ui/menus/solo/dialogs.rs` (delete dialog), `client/src/ui/menus/mod.rs` (the legacy `solo_action` / `list_worlds` systems go), `client/src/e2e/{solo,seed,helpers,game,main_menu,locator,review}.rs`, `client/tests/golden/menu_solo.png`, `client/tests/golden/menu_solo_empty.png` (new)

**Interfaces:**
- Consumes: `spawn_scroll_list`, `list_card`, `CardSelected`, `button`, `hint_bar`, `HINT_BAR_HEIGHT`, `menu_background`, `spawn_dialog`, `DialogClosed`, `list_worlds`, `delete_world`, `format_size`, `played_ago`, `elapsed_since`, `thumbnail_color`, `start_world`, `WorldsChanged`, `open_new_world_dialog`.
- Produces, in `solo/mod.rs`:
  ```rust
  #[derive(Resource, Default, Debug, Clone)] pub struct SelectedWorld { pub name: Option<String> }   // unchanged
  pub const WORLDS_TITLE: &str = "Worlds";
  #[derive(Component)] pub struct WorldCard(pub WorldSummary);
  #[derive(Component)] pub struct WorldList;            // the ScrollArea of the cards (Role::ListBox, "Worlds")
  #[derive(Component)] pub struct WorldDetail;          // the detail pane
  #[derive(Component, Clone, Copy)] pub enum WorldAction { Play, Delete }   // Rename in Task 6
  #[derive(Component)] pub struct WorldError;           // under the pane's buttons, DANGER, hidden when empty
  #[derive(Component)] pub struct NewWorldButton;
  #[derive(Component, PartialEq, Eq)] enum SoloHints { Cards, Elsewhere }
  pub const CARD_HINTS: [Hint; 5];                      // ↑↓ Select, Enter Play, Del Delete, Ctrl+N New world, Esc Back
  pub const SCREEN_HINTS: [Hint; 4];                    // Tab Move, Enter Select, Ctrl+N New world, Esc Back
  pub fn solo_plugin(app: &mut App);
  fn solo_setup(...);                                   // OnEnter(MenuState::Solo)
  /// Despawns the cards and spawns one per world, in order; returns them. Shows the empty state when there is none.
  fn show_worlds(commands: &mut Commands, list: Entity, worlds: &[WorldSummary], font: &Handle<Font>) -> Vec<Entity>;
  ```
  Layout (Decisions 6 and 7): root `menu_background`, `DespawnOnExit(MenuState::Solo)`, `TabGroup::new(0)`; content column, padding `SPACE_XL` with the bottom clear of the hint bar, `row_gap: SPACE_M` → [title bar (`"Worlds"` `FONT_XL`, spacer, `button("New world")` + `NewWorldButton`), body row (`flex_grow: 1`, `min_height: 0`, `column_gap: SPACE_L`) → [`spawn_scroll_list` frame with `WorldList` on its list, detail pane (`width: 280`, `PANEL`, raised bevel, padding `SPACE_L`, an `ArrowNav` column of Play (Primary), Delete (Danger), then `WorldError`)], footer → `button("Back")`], hint bar. Text in cards and pane wraps with `LineBreak::WordOrCharacter` and the text columns have `min_width: 0`. Behaviour: on `WorldsChanged` (and on setup) `show_worlds(list_worlds(saves))` and `AutoFocus` the card of `focus`, else the first card, else New world; `CardSelected` fills the pane; Enter on a focused card (no Control held) → `start_world`; Del on a focused card or Delete in the pane → the delete dialog (Decision 8), whose `Some(1)` calls `delete_world` and triggers `WorldsChanged { focus: <next card, else previous> }`, or shows `WorldError` (`"Could not delete the world: {e}"`); Play in the pane → `start_world`; New world and Ctrl+N → `open_new_world_dialog` with the names of the `WorldCard`s, whose success now calls `close_dialog(root, false)` (the new card takes the focus); Back → `MenuState::Main`; Escape stays in `escape_button`'s Solo arm. A system in `Update` keeps the hint bar on `CARD_HINTS` while a `ListCard` has the focus and on `SCREEN_HINTS` otherwise. Everything of the legacy screen goes (`WorldList` the legacy one, `WorldItem`, `SoloButtonAction`, `add_world_item`, `solo_action`, the legacy `list_worlds` system and its `saved_world_names`).
- Produces, in `e2e/seed.rs`: `GameFolder::with_seeded_world(self, name: &str, seed: u32, time: u64, played_ago: Duration) -> Self` (`with_played_world` calls it with seed 42). In `e2e/helpers.rs`: `Harness::rect_of(&mut self, locator: &L) -> Rect` (physical, like `center_of`). In `e2e/solo.rs`: `fn card(name: &str) -> L` (`L::role(Role::ListItem, name)`), `fn card_names(h) -> Vec<String>` (the `WorldCard` names in child order), `fn selected_world(h) -> Option<String>`, and `impl Harness { pub(super) fn play_world(&mut self, world: &str) }` (wait for the card, click it, click `L::role(Role::Button, "Play")`), which `e2e/game.rs`'s `enter_world_with` uses instead of `wait_for_icons` + `"Play {world}"`.

- [ ] **Step 1: Write the failing tests** (e2e, `e2e/solo.rs`; `world_list()` seeds "Survie" (seed 3, day 4, 3 days ago), "Creative build" (seed 1, 2 hours), "Test world 3" (seed 5, 20 days) and waits for `L::text(WORLDS_TITLE)`)

- `the_world_list_matches_its_golden` — `hover_at` a corner; golden `menu_solo`. New `the_empty_world_list_matches_its_golden` — empty folder; golden `menu_solo_empty`.
- `the_saved_worlds_are_listed_by_last_played` — `card_names == ["Creative build", "Survie", "Test world 3"]`; within `card("Survie")`: `"Day 4 · Seed 3"`, `"played 3 days ago"`, `"1 KB"`.
- `the_first_world_is_selected_and_shown_in_the_detail_pane` — `selected_world == Some("Creative build")`, focus on its card; within `L::marker::<WorldDetail>()`: `"Creative build"`, `"Seed 1"`, `"played 2 hours ago"`, `"1 KB on disk"`; click `card("Survie")` → the pane shows `"Day 4"`.
- `the_keyboard_alone_walks_the_worlds_and_plays_one` (the server journey; replaces `playing_a_world_connects_then_enters_the_game`) — from `main_menu_with(folder)`: `focus_label() == Some("Singleplayer")`, Enter → the list, `focus_label() == Some("Creative build")`, and after 3 frames `game_state() == Menu` (the Enter that opened the list did not play); ArrowDown → `"Survie"` selected and in the pane; End → `"Test world 3"`; Home → `"Creative build"`; Tab → `"Play"`; Shift+Tab → `"Creative build"`; ArrowDown, Enter → `SelectedWorld.name == Some("Survie")`, `GameState::Game` within `SERVER`.
- `delete_asks_first_and_the_focus_moves_to_the_next_world` — focus "Survie" (click), `press(Delete)` → `L::role(Role::Dialog, "Delete \"Survie\"?")`, `"Its 1 KB of saves will be deleted for good."` visible, `focus_label() == Some("Cancel")`; Escape → no dialog, `menu_state() == Solo`, folder still there; click `L::role(Role::Button, "Delete").within(L::marker::<WorldDetail>())`, then `"Delete"` within the dialog → `saves/Survie` gone, `card_names == ["Creative build", "Test world 3"]`, focus and selection on `"Test world 3"`; delete it and "Creative build" too → `L::text("No worlds yet")` visible, no `WorldDetail` shown, `focus_label() == Some("New world")`.
- `the_continue_card_is_the_first_world_of_the_list` — from the main menu: the Continue card shows `"Creative build"`; Singleplayer → first card `"Creative build"`; delete it; Escape → the Continue card shows `"Survie"`, `"played 3 days ago"`.
- `a_created_world_is_first_selected_and_focused` — `ctrl_n`, `type_text("Fresh")`, Enter → `card_names[0] == "Fresh"`, `focus_label() == Some("Fresh")`, `"played just now"` within its card.
- `the_key_hints_follow_the_focus` — card focused → `L::text("Del").within(HintBar)` visible; Tab → `"Del"` gone, `"Tab"` shown; a dialog open (`ctrl_n`) → `"Create"` within the dialog's bar; empty folder → `"Ctrl+N"` and no `"Del"`.
- `hovering_a_card_or_a_button_lightens_it` — hover `card("Survie")` (not selected) → `RAISE_HOVER`; hover `"New world"` → `RAISE_HOVER`.
- `the_wheel_scrolls_only_the_list_under_the_cursor` (replaces `the_wheel_scrolls_a_long_list`, journey kept) — 15 worlds; hover `card("world 01")`, `scroll(-5.)` → its centre moved up; hover the detail pane, `scroll(-5.)` → nothing moved.
- `back_and_escape_return_to_the_main_menu` — click `"Back"`; Singleplayer, Escape (a card focused) → `MenuState::Main` both times.
- `a_long_name_stays_inside_its_card_at_800x600` — a world named 32 × `W`, 800×600 → `rect_of(L::text(name).within(card(name)))` inside `rect_of(card(name))`, and the pane's name inside `rect_of(L::marker::<WorldDetail>())`.
- Locators elsewhere: `e2e/main_menu.rs` and `e2e/locator.rs` wait for `L::text(WORLDS_TITLE)` instead of `"World list"` and click `"Back"` instead of `"Back to menu"`; `e2e/review.rs` leaves the list with `"Back"`; `e2e/game.rs` uses `play_world`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client e2e::solo` → compile errors, then no `"Worlds"`.

- [ ] **Step 3: Implement** as in Interfaces; remove the allowances of Task 3 and every legacy item this orphans (`rg` for each removed name finds nothing).

- [ ] **Step 4: Run the tests; regenerate the goldens on purpose** — `menu_solo`: the dirt background, "Worlds" and "New world" in the title bar, three cards with distinct thumbnails, the first with its accent border, the scrollbar well, the pane with its facts and green Play, "Back", the hint bar, no ring, no hover. `menu_solo_empty`: "No worlds yet" and its line in the list area, no pane, "New world" without ring. `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_world_list_matches the_empty_world_list_matches`, rerun → PASS, no leftovers. Full suite green (`e2e::game` through `play_world`).

- [ ] **Step 5: Commit** (the report lists the replaced tests and changed locators) — `git commit -m "feat(client): the Singleplayer screen with world cards, a detail pane and a confirmed delete"`

---

### Task 6: Rename

**Files:**
- Modify: `client/src/ui/menus/solo/{mod,dialogs}.rs`, `client/src/e2e/solo.rs`, `client/tests/golden/menu_solo.png` (only if the third button moves it: regenerate after a look and say so)

**Interfaces:**
- Consumes: `spawn_form_dialog`, `WorldForm` with `current: Some(name)`, `rename_world`, `WorldsChanged`.
- Produces: `WorldAction::Rename` (a `button("Rename")` between Play and Delete); `pub fn open_rename_dialog(commands: &mut Commands, assets: &AssetServer, focus: &InputFocus, world: &str, others: Vec<String>) -> Entity` — Decision 8, one name field filled with `world`, hints `[Tab Move, Enter Rename, Esc Cancel]`; its submit calls `rename_world` → `WorldsChanged { focus: Some(new) }` and `close_dialog(root, false)`, or `FieldError(Some(format!("Could not rename the world: {e}")))`.

- [ ] **Step 1: Write the failing test** (e2e, `e2e/solo.rs`)

`rename_renames_the_folder_and_keeps_the_place_of_the_world` — select "Survie", click `L::role(Role::Button, "Rename")` → `L::role(Role::Dialog, "Rename \"Survie\"")`, `value_of::<WorldNameInput>() == "Survie"`, Rename has `InteractionDisabled`, no message; `type_text("Creative build")` → `WORLD_EXISTS`; select all is gone, so Backspace ×14, `type_text("Base")`, Enter → `saves/Base/world.ron` has `name == "Base"`, no `saves/Survie`, `card_names == ["Creative build", "Base", "Test world 3"]`, focus on `"Base"`, `"played 3 days ago"` within its card; Rename again, Escape → nothing changed; Escape → the main menu's Continue card still shows `"Creative build"`.

- [ ] **Step 2: Run it to verify it fails** — `timeout 300 cargo test -q -p client e2e::solo::rename` → no "Rename".

- [ ] **Step 3: Implement** as in Interfaces; remove the allowance on `rename_world`.

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): rename a world, its folder and its world.ron"`

---

### Task 7: Legacy cleanup, the Escape stopgap, review snapshots and docs

**Files:**
- Modify: `client/src/ui/menus/list_screen.rs`, `client/src/ui/menus/mod.rs`, `client/src/e2e/multi.rs` (one test), `client/src/e2e/review.rs`, `README.md`, `docs/ARCHITECTURE.md`, plus whatever the review finds in the files of Tasks 2–6

- [ ] **Step 1: The stopgap, test first** — in `e2e/multi.rs`: `escape_in_a_field_leaves_the_field_then_the_screen` — click `L::role(Role::TextInput, "Server name")`, `type_text("Ho")`, Escape → `menu_state() == Multi` (and after 2 more frames), `InputFocus` empty; Escape → `MenuState::Main`. Run → FAIL (the stopgap goes to the main menu). In `list_screen.rs` the field's `Cancel` observer clears `InputFocus` instead (Decision 10). Run → PASS.

- [ ] **Step 2: Drop what only Solo used** — `ListScreen::stacked` and its branch, `InputField::focused` and its `AutoFocus`; the module doc of `list_screen.rs` names the server list only. Then `rg -n "MenuButtonAction|button_system|ScrollingList|mouse_scroll|spawn_list_screen|spawn_row|fill_image_node|load_button_background|load_dark_button|wait_for_icons|play_rows" client/src`: every hit is in `list_screen.rs`, `multi.rs`, `button.rs`, `style.rs`, `assets.rs`, `mod.rs`, `chat.rs` (`text_font`), `e2e/multi.rs`, `e2e/helpers.rs` or `e2e/game.rs` (the connection test on Multiplayer) — the list Lot 3 inherits; clippy without allowance proves every item left has a user.

- [ ] **Step 3: Extend the review journey** — per resolution, replacing the `"solo"` line: Singleplayer → `"solo"`; Ctrl+N → `"solo_new_world"`; `type_text("Review")` → `"solo_new_world_error"` (`WORLD_EXISTS`), Escape; Del → `"solo_delete"`, Escape; Rename → `"solo_rename"`, Escape; Back.

- [ ] **Step 4: Run and look** — `UI_SNAPSHOTS=1 timeout 600 cargo test -q -p client review -- --ignored`, then read every new `target/ui-snapshots/every_screen_at_three_resolutions/solo*_{800x600,1280x720,1920x1080}.png`. Check: everything fits 800×600 (list, pane and Back above the hint bar; the five card hints on one line; the dialogs with both fields, their reserved error line and their buttons inside the window); the error message under the name field without anything moving; thumbnails and bevels crisp at 1.5; the selected card's accent border distinct from the focus ring; Danger red only on Delete and errors; nothing overflows a card or a button. Fix any defect in its task's file with a test first.

- [ ] **Step 5: Positions from the window** — `rg -n "cursor_position|world_to_viewport" client/src`: every result that ends in a `Val::Px` divides by `UiScale`; fix any that does not, with a test.

- [ ] **Step 6: Docs** — ARCHITECTURE "UI widgets": `list.rs` (a scroll list with its scrollbar; cards selected by focus, one tab stop, ↑↓ Home End), form dialogs in `dialog.rs` (fields, `DialogSubmit` confirmed by the caller with `close_dialog`, Escape in a field closes the dialog), `reserve_error` in `text_field.rs`; one sentence: the world list is rebuilt from disk after every change and shares `list_worlds` with the Continue card. ARCHITECTURE "Saves": Singleplayer writes `world.ron` when a world is created, renames the folder and the name in `world.ron` (keeping its version and date), and deletes the folder after a confirmation. README "Saves": one line saying the same; the e2e example waits for `L::text("Worlds")`.

- [ ] **Step 7: Full suite** — `timeout 900 cargo test --workspace --no-fail-fast` green; fmt and clippy clean; `git status` shows no `.actual.png` / `.diff.png`.

- [ ] **Step 8: Commit** — `git commit -m "test(client): review snapshots of the Singleplayer screen, Escape in a server field, and docs"`
