# UI redesign — design

**Date:** 2026-10-18 · **Branch:** `feat/ui-redesign` (from `main` at c7a8c53) · **Status:** approved by the owner, who delegated every open choice ("je te laisse trancher et prendre tes propres recommandations").

The verified notes on the Bevy 0.19.1 UI APIs this design relies on are in
`2026-10-18-bevy-ui-0.19-notes.md`, next to this file. The proposal this implements, with mockups of every screen, is the artifact
https://claude.ai/artifact/BvMQVHaQm8ypvKDVmLtdsg. This document is the binding version for the
builder: where the two differ, this document wins.

## Intent

Rustcraft's UI works but feels like four different programs, cannot be driven from the keyboard,
breaks at low resolution, and hides errors in the logs. The owner's words: the main menu is ugly;
Tab should work everywhere "like a browser"; Enter must submit text fields; the singleplayer and
multiplayer menus need modernizing; the inventory is ugly, breaks at low resolution, covers the
hotbar and does not look related to it; "L'UX est super important", "tout est à revoir".

Success: a player can do everything with the keyboard alone or the mouse alone, every screen
looks like the same game, every screen fits 800×600 and stays readable at 4K, and nothing
irreversible happens by accident.

## Goals

1. **Keyboard first.** Tab / Shift+Tab move the focus through every menu like a browser; arrows
   move within lists, grids and sliders; Enter or Space activates the focused control; Enter in a
   text field submits its form; Escape closes the dialog, cancels a key capture, or goes back one
   screen. The focus is always visible (amber ring). Each screen focuses its first useful control
   when it opens.
2. **One visual system** ("pixel-craft modernised"): dark bevelled panels, the game's pixel font,
   one accent colour, shared by menus, pause, inventory, chat and loading.
3. **Fits every screen.** A `UiScale` follows the window height; layouts fit 800×600 at scale 1.
4. **Always feedback.** Hover, focus, pressed, disabled, loading and error states are all visible;
   errors appear next to what caused them, in the UI, not only in the logs.
5. **Nothing irreversible by accident.** Deleting a world or a server, and quitting from the main
   menu, ask for confirmation with the focus on Cancel.
6. Every defect listed in the end-to-end test spec (`2026-10-18-ui-e2e-tests-design.md`,
   "Non-goals") is fixed, each with a test that failed first.

## Non-goals

- A 3D panorama behind the menus (the dirt texture is reworked instead; panorama may come later).
- Translation: the UI stays in English. No string table.
- Gamepad navigation (the focus model allows it later).
- Depending on `bevy_feathers` (editor-oriented by its own docs; copy its patterns instead).
- Changing gameplay, networking semantics or save formats beyond what a screen below requires.

## Anti-goals (failure even if it "works")

- An end-to-end journey that passed before the redesign no longer works for a player (a test may
  change its labels or locators, never lose the journey it covers).
- Blurry or unevenly scaled pixel art (icons and textures must land on whole multiples of their
  texels).
- A control reachable by mouse but not by keyboard, or a focus that escapes an open dialog.
- A text field that swallows game keys after it is closed, or game keys (E, T, F3…) acting while
  the player types.

## Decisions (made by the builder on the owner's behalf)

1. **Visual direction A**, pixel-craft modernised (tokens below). The stone button textures are
   retired in favour of drawn bevels; the title image and the dirt background stay.
2. **Menu background:** the dirt texture, darkened, with a vignette.
3. **English UI**, no i18n layer.
4. **"Continue"** on the main menu: the most recently played world, i.e. the world whose
   `world.ron` was modified last. No new persisted state.
5. **Order of work:** Lot 0 (foundations), Lot 5 (hotbar and inventory), then Lots 1, 2, 3, 4, 6.
6. **Toolkit:** `bevy::input_focus` (`InputFocus`, `InputFocusVisible`, `AutoFocus`,
   `FocusedInput<KeyboardInput>`, `TabNavigationPlugin`, `TabIndex`, `TabGroup`, `TabGroup::modal()`),
   `bevy::ui_widgets` (`Button` + `Activate`, `Slider`, `Checkbox`, `RadioGroup` (also for tabs),
   `ScrollArea`/`Scrollbar`), `bevy::text::EditableText` + `TextCursorStyle` for text
   fields. Our own layer adds visuals, `TabIndex`, labels and arrow navigation.
   **`bevy_simple_text_input` is removed.** Pin `bevy = "=0.19.1"` (the widgets are experimental).
7. **Arrow navigation:** our own observer around the directional navigator's
   `find_best_candidate`, limited to the focused container or the open modal. Not the global
   automatic navigator (it ignores z-order and would escape modals).
8. **Accessibility:** every widget carries an `AccessibilityNode` with a role and a label
   (`AccessibleLabel`), using `accesskit = "0.24"`. The e2e locators gain `role(Role, label)`.
9. **Debug overlay** (F3) hidden by default and drawn below menus and dialogs. The egui inspector
   only in debug builds (`cfg(debug_assertions)`).
10. **Server status** in the server list comes from a small status request answered by the
    server (players, max players, protocol, latency). If the transport cannot carry an
    unconnected request, the server answers on a separate UDP status port next to the game port;
    the builder decides with the code in front of them, and keeps it minimal.

## Visual system

Logical pixels at `UiScale` 1 (design height 720).

| Token | Value | Use |
|---|---|---|
| panel | `#111318` opaque | panel background |
| raise / raise-hover | `#252A33` / `#2E3440` | buttons, tabs |
| bevel hi / lo | `#48505F` / `#07080A`, 2 px | raised: hi top-left, lo bottom-right; sunken (fields, slots): inverted |
| slot | `#0C0E12` | field and slot interiors |
| accent | `#F0B54A` | focus ring (3 px outline, 2 px offset), selection, slider fill — nothing else |
| ok / danger | `#78C75F` / `#E3593F` | primary action, online · delete, error |
| text / muted | `#EFE9DA` / `#9AA2AE` | text · hints, metadata |
| type | RustCraft font at 16 / 20 / 24 / 32 px; Fira Mono for the debug overlay only | |
| spacing | 4 · 8 · 12 · 16 · 24 · 32 | |

Components: button (normal, hover, focus, pressed = bevel inverted, disabled = 40 % opacity;
variants primary, danger, ghost), text field (with placeholder, caret, error state and message
under it), slider (track, fill, thumb; ←/→ by 5 %), segmented control, tabs, list with
selectable cards and a visible scrollbar, modal dialog (backdrop, title, body, actions right
aligned, primary last), toast (top-right, 3 s), key-hint bar (bottom of menus: `[Enter] Select`…),
inventory slot (sunken, count bottom-right with shadow, no count for 1, number 1–9 for hotbar
slots, selected = accent border), tooltip.

**UiScale:** auto = `max(1, floor(height / 720 × 2) / 2)` (720→1, 1080→1.5, 1440→2, 2160→3); the
player can force 1, 1.5, 2 or 3 in Settings → Interface, clamped so the logical height stays ≥ 600.
Item icons are 32 logical px, so at every allowed scale they are a whole multiple of their 16 px
texture. Images keep nearest sampling.

## Screens (what must be true)

**Main menu.** Logo top-left; Singleplayer, Multiplayer, Settings stacked under it; "Quit game" as
a ghost button; a "Continue" card on the right with the last world (thumbnail colour, name, day,
"played … ago") and a Play button (also Ctrl+Enter); version top-right; key-hint bar. Focus starts
on Singleplayer. Escape opens "Quit Rustcraft?" (Cancel focused).

**Singleplayer.** Title bar with "New world" (Ctrl+N); a scrollable list of world cards (name,
in-game day, seed, last played, size on disk), sorted by last played; a detail pane for the
selected world with Play (Enter), Rename, Delete (Del). ↑/↓ select; the wheel scrolls only the
list under the cursor. New world: a dialog with Name (validated as you type: empty, invalid
characters, already exists — message under the field, Create disabled) and an optional Seed;
Enter creates, Escape cancels; the world is written to disk at once (`saves/<name>/world.ron`).
Rename: same dialog shape, renames the folder and the name in `world.ron`. Delete: confirmation
dialog naming the world and its size, Cancel focused.

**Multiplayer.** Same layout: server cards with status dot, players `n / max`, latency bars or ms,
"offline"; detail pane with Join (Enter), Edit, Remove (Del); Add server (Ctrl+N) and Refresh
(F5). Add/Edit dialog: Name and Address (`host:port`, host names accepted and resolved, default
port 8000 when omitted), validated as you type; duplicates rejected. Remove asks for confirmation.
A deleted "localhost" stays deleted; the list is empty-state friendly ("No servers yet — Add
server").

**Settings.** One screen with tabs: Controls, Audio, Interface (and Graphics only if a graphics
setting exists). Changes apply and save at once. Audio: sliders (drag, wheel, ←/→ by 5 %). Interface:
UI scale segmented control. Controls: actions with human names grouped by category (Movement,
Interface, Debug…), primary and secondary key columns, arrows shown as glyph icons, Enter on a row
starts the capture ("Press a key…"), Escape cancels it (Escape itself cannot be rebound), Del clears
the focused slot, a conflict is shown on both rows with a message, "Reset to defaults". Settings
are also reachable from the pause menu without leaving the game.

**Connecting.** A card with the real steps (starting the local server, connecting, loading chunks
`n / total`, spawning) and a progress bar; Cancel (Escape) at any time. After 10 s without
answer, or on a refused/failed connection, an error card says what happened and what to do, with
"Try again" (focused) and "Back to <list>", which returns to the list the player came from.

**HUD.** Hotbar centred at the bottom, 9 slots of 50 logical px with numbers 1–9; keys 1–9 select
a slot; the wheel (including trackpad pixel scrolling, accumulated) moves the selection; the item
name shows for 2 s above the hotbar when the selection changes; reticle with a dark outline.

**Inventory.** One panel that grows upward from the hotbar: its bottom row *is* the hotbar, at the
same place with the same slots, so opening the inventory never covers or moves it. Above: the
3×9 backpack, and at the top the crafting grid (2×2, or 3×3 at a crafting table) with its result.
Clicks on the panel never reach the game; clicks outside it close nothing and drop nothing.
Tooltip on hover (item name, count, hint); Shift+click moves a stack between backpack and hotbar;
1–9 over a slot swaps it with that hotbar slot; the held stack follows the cursor, centred, at
slot size. Fits 800×600 at scale 1.

**Pause.** Same panel and buttons: Back to game (focused), Settings, Save now (toast "World saved"),
Save and quit to title ("Saving…" until the server has stopped). Escape returns to the game.

**Chat.** T opens, `/` opens with "/" typed; Enter sends, Escape cancels and keeps no draft; the
log keeps the last 100 messages with times, scrolls with the wheel and PageUp/PageDown, fades
after 7 s when closed; ↑/↓ recall sent messages. While the chat or any text field has focus, game
keys (movement, E, F-keys, 1–9) do nothing.

**Debug.** F3 toggles the overlay, hidden by default, below menus and dialogs; other F-keys
ignored while typing.

## Architecture

- `client/src/ui/theme.rs` — the tokens (colours, sizes, fonts) as constants and a `Theme`
  resource where runtime values are needed.
- `client/src/ui/widgets/` — one file per widget: `button`, `text_field`, `slider`, `segmented`,
  `tabs`, `list`, `dialog`, `toast`, `hints`, `slot`, `tooltip`, plus `focus.rs` (focus ring,
  arrow navigation, Escape-to-back, AutoFocus per screen, focus restore after a dialog) and
  `scale.rs` (`UiScale`). Each widget is a function returning a bundle or spawning a small tree,
  with its own observers; screens only assemble widgets.
- Screen logic that is not drawing (validating a world or server name, listing worlds with their
  metadata, resolving an address, conflict detection in key maps) lives in plain functions with
  unit tests.
- Menu screens keep `MenuState`; dialogs are entities with `TabGroup::modal()` spawned over the
  screen, not states.
- The legacy `Interaction`-based `button_system` and `MenuButtonAction` are removed once no screen
  uses them.

## Testing

- **Harness first (Lot 0):** the e2e input must also write `WindowEvent::CursorMoved`,
  `WindowEvent::MouseButtonInput` and `WindowEvent::MouseWheel` (bevy_picking reads only those),
  and `press` must send real logical keys (`Key::Enter`, `Key::Escape`, `Key::Backspace`, arrows,
  `Key::Tab`, `Key::Shift`) for `EditableText`. Proven by a test that clicks a `ui_widgets::Button`
  and sees `Activate`, and a test that types into an `EditableText` and submits with Enter.
- TDD everywhere: every behaviour in "Screens" gets a test that fails first; every defect from the
  e2e spec's non-goals gets its test with its fix.
- Existing e2e tests keep their journeys. When a screen changes, its tests change their locators
  (text, `name`, `role`) and steps in the same task; the change is listed in the task report.
- Goldens of menu screens are regenerated deliberately when a screen is redesigned, after a human
  look at the PNG; review snapshots (`UI_SNAPSHOTS=1`) at 800×600, 1280×720 and 1920×1080 are
  checked at the end of each lot.
- Tests run under `timeout`; the suite stays under ~2 minutes.

## Lots

Each lot is planned in its own plan just before it starts, delivered as reviewed commits, and
keeps the whole suite green.

0. **Foundations:** harness picking and keys; theme; button and text field widgets; focus ring,
   Tab navigation, arrow navigation, AutoFocus; `UiScale`; a11y roles and the `role` locator;
   `bevy_simple_text_input` replaced in the existing screens by the new text field (Enter submits
   from here on). No screen redesigned yet beyond what the replacement needs. Every other widget
   (slider, segmented, tabs, list, dialog, toast, hints, slot, tooltip) is built in the first lot
   that uses it, in `ui/widgets/`, so that no widget exists without a screen and its tests.
5. **Hotbar and inventory** (done second): HUD hotbar, item name, 1–9, trackpad wheel, reticle;
   unified inventory panel, tooltips, Shift+click, 1–9 swap, crafting table panel; debug overlay
   below menus and hidden by default; game keys gated while typing.
1. **Main menu and settings:** new main menu with Continue and quit confirmation; settings tabs,
   sliders, interface scale, controls screen.
2. **Singleplayer:** world cards, detail pane, new-world / rename / delete dialogs, world written at
   creation.
3. **Multiplayer:** server cards, status request, host names, add/edit/remove dialogs.
4. **Connecting and errors:** steps and progress, timeout, error card, return to the origin list.
6. **Pause, chat, polish:** pause with settings and save toast; chat history, recall, `/`; final
   pass on review snapshots at three resolutions.

## Left to the builder

Exact pixel sizes within the tokens, file splits, the status-request transport (decision 10),
whether `ListBox` or our own list fits the cards better, and the order of tasks inside a lot.
