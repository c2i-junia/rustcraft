# End-to-end UI tests — design

## Intent

The UI of Rustcraft is about to be redesigned (branch `feat/ui`): menus, keyboard navigation (Tab, Enter), inventory, HUD. Before touching it, a suite of end-to-end tests "in the style of Playwright" must cover every screen, so the redesign can move fast without breaking flows.

The tests drive the real client the way a player does — a cursor moved over a button and clicked, keys pressed, text typed — and check what the player would see: the screen shown, the rows of a list, a dialog opened, a file saved. They do not call the systems of a screen directly.

## Goals

- Every screen and overlay of the UI is reached and exercised by at least one test: main menu, singleplayer world list, multiplayer server list, settings, controls, audio, connection screen, in-game HUD (hotbar, reticle, chat, inventory and crafting, debug overlays), pause menu.
- Full journeys run end to end, including the real integrated server: create a world, play it, open the inventory, pause, save, go back to the menu. Multiplayer connects to a real dedicated server started by the test.
- Tests find elements the way a player names them: by their visible text first, by a marker component or a `Name` when the element has no text (icons).
- Golden screenshots of each menu screen at 1280x720, compared with the existing golden machinery (lavapipe, tolerance, `.actual.png` / `.diff.png`, `UPDATE_GOLDEN=1`).
- Review snapshots: every test can save pictures of what it saw (not compared), at several resolutions, so a human can review the UI. They feed the presentation of the redesign.
- The suite runs in `cargo test --workspace`, so in `just check` and in the CI, in parallel, with no window and no fixed network port.

## Non-goals

- No test encodes a known defect of the current UI. The audit of 2026-10-18 found these, which stay untested; the redesign brings tests for the behaviour it wants (TDD), which fail first:
  - Enter does nothing in the fields of the world and server lists; no Tab or keyboard activation anywhere; the controls screen cannot be left with the keyboard.
  - Escape on the main menu quits at once; deleting a world or a server asks no confirmation.
  - "Create world" only adds a row in memory: the world is lost if it is not played.
  - Errors (invalid or duplicate world name, invalid address, host names refused, unreadable `servers.ron`) are only logged; empty server names and duplicate servers are accepted; the fields are not cleared after Add; localhost comes back after being deleted.
  - The connection screen never gives up and has no error; Escape does not cancel it; failures return to the main menu, not to the list.
  - The inventory overlaps the hotbar at 800x600, 1280x720 and 1366x768, and a click on its padding reaches the hotbar cell beneath; the hotbar is dimmed under the inventory.
  - The chat keeps only the last 2 messages; no 1–9 keys for the hotbar; trackpad scrolling never moves the hotbar selection.
  - The debug overlay is shown by default, above the dialogs; F3, F4, F6, F7 work while the chat or the pause menu is open; the egui inspector shows on every screen.
  - No hover feedback on the settings buttons, the row icons, the controls' Back and Cancel; the pressed look shows the normal image; the wheel scrolls every list at once.
  - Controls: the same key can be bound twice, the recorder cannot be cancelled, arrow keys show as empty labels, actions are named by their code name.
- A coverage line found broken today by a defect missing from that list is not bent to pass: it becomes a test marked `#[ignore = "defect: …"]` stating the intended behaviour, and the defect is reported for the redesign. *(Builder's call.)*
- No golden of the 3D world: the world, the time of day and the mobs vary. In-game screens are checked by behaviour and by review snapshots only. The existing inventory goldens (`client/src/ui/hud/inventory/goldens.rs`) stay as they are.
- No change of the UI itself, apart from what testability needs (below).
- No tests for macOS or Windows: like the goldens, the suite needs lavapipe, so it is ignored off Linux.

## Anti-goals

- Tests that set a `NextState` or insert an `Interaction` to "click": they would pass while the real click is broken (for example the inventory hiding a hotbar cell under its padding).
- Tests coupled to pixel positions or to the entity tree of a screen: the redesign rewrites the layouts, the tests must survive it with, at most, a changed label or an added confirmation step.
- A suite slow or flaky enough that people skip it.

## Constraints

- Bevy 0.19. `bevy_ui::focus::ui_focus_system` computes `Interaction` from the cursor of a `Window` targeted by a camera; cameras rendering to an image get no cursor. `ButtonInput` is rebuilt from `MouseButtonInput` / `KeyboardInput` messages every frame by `InputPlugin`: input must be injected as messages. `bevy_simple_text_input` reads `KeyboardInput` messages (`logical_key` for characters, `key_code` for Enter, Backspace, arrows).
- `client` is a binary crate: the tests live in `#[cfg(test)]` modules inside it, like `ui/golden.rs`.
- The integrated server is a `std::thread` running a Bevy app on wall-clock time: journeys wait with a real-time timeout, not a frame count.
- The client writes `keybindings.ron`, `audio.ron`, `servers.ron` and `saves/` in its game folder: each test gets its own temporary game folder, removed at the end.
- `just check` (fmt, clippy `-D warnings`, tests) passes after every commit.

## Decisions

- **One way to build the app.** The wiring of `main.rs` (plugins, resources, states) moves into a function both `main` and the harness call, so the tests run the app the player runs. Only what cannot run headless differs: no winit, no log, no gamepad, the egui inspector left out, lavapipe as the adapter, a primary `Window` with no surface (it gives the cursor and the resolution, at scale factor 1).
- **Screenshots.** For a picture, the harness points every camera to an image of the window's size, renders, then points them back to the window. Layout is the same since the size is.
- **Locators.** `text("Singleplayer")` finds the visible `Text` with that content; `click()` moves the cursor to the centre of its node and presses then releases the left button over two frames, so the real focus system decides what is hit. Other locators: by marker component, by `Name`. A locator that finds no visible element, or more than one, fails the test with the list of texts on screen.
- **Scoping.** Text that repeats on a screen (`100%` three times on the audio screen, a key bound to several actions) is read inside a scope, as Playwright filters a row by the text it has: `row("Master")` is the nearest ancestor of the text "Master" that also holds other texts or buttons, and locators chained on it only search inside it. *(Builder's call.)*
- **`Name` as test ids.** Elements without text that a test must click (the play and delete icons of a row, the clear button of a key binding, the volume steppers) get a `Name` naming their action and their row, e.g. `Name::new("Delete Survie")`. This is the only change of the screens.
- **Waiting.** `wait_until(condition)` runs frames until the condition holds or a timeout passes (30 s for journeys through a server, a few frames otherwise), and fails with what was expected.
- **Seeding.** Helpers write a game folder before the app starts: saved worlds (with a player inventory when a test needs items), a server list, key bindings.
- **Where.** `client/src/e2e/` (`#[cfg(test)]`): the harness, then one test module per screen or journey. The golden comparison of `ui/golden.rs` is shared, not copied.
- **Speed.** The suite adds at most about 2 minutes to `cargo test --workspace` on a 4-core machine. Journeys through a server share one app across their steps (one world created, then many checks), rather than one server per check. *(Builder's call.)*
- **Crafting table.** Reaching the 3x3 dialog needs a table placed in the world and the camera aimed at it: left out of the end-to-end suite, as its display is covered by the inventory goldens and its rules by the unit tests of `shared`. *(Builder's call.)*
- **Spike findings.** A click through the cursor of a surfaceless primary window works (`client/src/ui/recon_spike.rs`, throwaway). The cameras need `IsDefaultUiCamera`, otherwise the UI has no camera while they render to the image; one frame must run after each change of render target before reading positions or clicking.
- **Review snapshots** go to `target/ui-snapshots/<test>/<name>_<width>x<height>.png`, ignored by git.

## Coverage

Each line is a test or a step of a journey; all check the current, intended behaviour.

- **Main menu**: golden; Singleplayer, Multiplayer and Settings open their screen; Quit exits the app; hovering a button changes its look.
- **Singleplayer**: golden with three saved worlds; the saved worlds are listed in order; typing a name and clicking Create adds a row; an invalid or duplicate name adds nothing; the play icon of a world starts the connection then the game; the delete icon removes the row and the world's folder; Back and Escape return to the main menu; the wheel scrolls a long list.
- **Multiplayer**: golden; localhost and the saved servers are listed; clicking a field focuses it and not the other; typing a name and an address then Add adds a row, saved to `servers.ron` when the screen is left; an invalid address adds nothing; delete removes a server; Back and Escape return to the main menu; connecting to a dedicated server started by the test reaches the game.
- **Settings**: golden; Controls, Audio and Back open their screen; Escape returns to the main menu.
- **Controls**: golden; each action lists its keys; clicking an action then pressing a key adds the key, saved in `keybindings.ron` when the screen is left; the clear button removes an action's keys; Back returns to the settings.
- **Audio**: golden; + and − change the volume shown by 10 %, between 0 % and 100 %; the volumes are saved in `audio.ron` when the screen is left; Back and Escape return to the settings.
- **Connection screen**: shown while connecting, with its animated text; Cancel returns to the menu.
- **In game** (journeys through the integrated server):
  - the reticle, the hotbar of 9 cells and the debug overlay are shown; F3 hides and shows the overlay;
  - the mouse wheel moves the hotbar selection;
  - E opens and closes the inventory; Escape closes it; with a seeded inventory, clicking a stack then an empty cell moves it, between the inventory and the hotbar too; crafting planks in the 2x2 grid gives the result;
  - T opens the chat, typing a message and Enter sends it and shows it; Escape closes the chat without sending;
  - Escape opens the pause menu; Resume closes it; Save writes the world; Back to menu returns to the main menu and the world is saved.

## Left to the builder

- The exact API names of the harness, the number of frames to wait outside journeys, how journeys are split into tests to keep the suite fast.
- Which resolutions the review snapshots are taken at (at least 800x600, 1280x720, 1920x1080 for the menus, the inventory and the pause menu).
- Review snapshots are only written when `UI_SNAPSHOTS=1` is set. *(Builder's call.)*
