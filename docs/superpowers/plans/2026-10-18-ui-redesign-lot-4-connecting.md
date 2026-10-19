# UI Redesign Lot 4 (Connecting and errors) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the legacy "Connecting to server" screen with the Connecting card of the spec: the real steps (starting the world or connecting, loading chunks `n / 27`, spawning) and a progress bar, a Cancel that always works and Escape; a timeout; an error card for every failure (no answer, another version, full, refused, invalid name, world that cannot load, connection lost while loading, server stopped or lost in game) that says what happened and what to do, with "Try again" focused and "Back to Worlds" / "Back to Servers" returning to the list the player came from with their card selected.

**Architecture:** A session now knows where it came from (`SessionOrigin`, set by `start_world` and `join_server`) and how it ended (`SessionEnd`, written once by whoever ends it first: Cancel, the connection watcher, the in-game watcher). What decides a value lives in plain, unit-tested functions in `client/src/network/session.rs` (the step a session is at, its progress, which failure a netcode reason means, the chunks around the spawn). The client reads the reasons it already has (netcode's disconnect reason, Lot 3's status exchange, its own player name), so no game message changes. The game starts once the 27 chunks around the spawn are in, so chunks are polled from the authentication on and meshed on entering the game. Teardown keeps the existing transitions (`PreGameLoading → Menu`, `OnExit(Game)`); the menus then open on the error card (`MenuState::ConnectError`, the dialog widget over the menu background) or on the origin list.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate}`, `bevy::input_focus`, `AutoFocus`), `bevy_renet` 5 / `renet` 2 / `renet_netcode` 2 (`NetcodeClientTransport::disconnect_reason`, `NetcodeDisconnectReason`), `accesskit` 0.24 (`Role::ProgressIndicator`), Lot 3's `StatusProbe` and `InFlight`, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md` — only "Lot 4 — Connecting and errors" and the "Connecting" paragraph of "Screens"; "Visual system", "Architecture", "Testing" and the anti-goals bind every task. Defects to fix: `docs/superpowers/specs/2026-10-18-ui-e2e-tests-design.md`, "Non-goals" (the connection-screen line). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md`. Conventions and products: the Lot 0, 5, 1, 2 and 3 plans next to this file; Lot 3 is the model (Global Constraints, one server journey, `InFlight`, `StatusProbe`).

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `feat(shared): …`, `fix(…)`, `test(…)`, `refactor(…)`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::connecting`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift, which runs no destructor while a panic unwinds: do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`. Lot 4's tests go in the new `e2e/connecting.rs`, driven only through `click`/`hover`/`press`/`type_text`. **One** server journey (Task 6), chaining its checks; its server is a thread. At most two tests wait for netcode to give up (≈5 s each): Task 4's journey and the Task 7 review.
- **No fixed network port in tests**: servers bind with `server::acquire_local_socket_pair`. A test may send to `127.0.0.1:9` (discard, nothing answers) and to `127.0.0.1:8000` only to check the address handed over, then cancels at once. Never join a non-loopback address (`fixed_status`'s `10.0.0.x` servers are never joined).
- Never a bare `git stash`; set work aside with a WIP commit.
- `bevy::prelude::Button` is the legacy widget: `use bevy::ui_widgets::Button as UiButton;`. New screens use no `Interaction`. The widget component or the explicit `AccessibilityNode(Role::…)` goes **before** `AccessibleLabel` in a bundle.
- A widget item no screen uses yet allows `dead_code` on the item with a comment naming its first user; the allowance goes in the task that uses it. Never crate-wide.
- **Escape.** A dialog (the error card) consumes Escape through `FocusedInput<KeyboardInput>` and closes in `PostUpdate`. The connecting card reads Escape from `ButtonInput<KeyCode>` in `Update` while in `PreGameLoading` (no field or dialog exists there). `escape_button` keeps its arms and skips while `dialog_open` or `text_field_focused` holds.
- **The first end of a session wins.** Every writer of a session's end (`end_session`: Cancel, Escape, `watch_connection`, `watch_session`, Try again's `start_world`/`join_server`, `finish_join`, `escape_button`) does nothing while `NextState<GameState>` (or, for `escape_button`/`finish_join`, the other state) is already `Pending` this frame. Order between them never matters.
- **Fonts.** Only `FONT_PATH` (RustCraft) and `MONO_FONT_PATH` (key caps). The RustCraft font has no `← ↑ → ↓`, `…`, `×`, `−`, backtick or accented letters: write `...`; `"` and `:` are fine.
- **The UI thread never waits on the network or a thread.** The status asked after a silent server runs in `InFlight::spawn` (Lot 3); a local server thread is joined only once `is_finished()`.
- **Protocol and compatibility.** `PROTOCOL_ID` stays `5`: no game message changes. Reasons come from netcode (`NetcodeDisconnectReason`), the status exchange (`ServerStatus::Incompatible`) and the client's own `sanitize_username`. `BROADCAST_CHUNK_RADIUS` (value `1`, unchanged) moves to `shared` and becomes part of the protocol: the client waits for the `(2r+1)³ = 27` chunks around its spawn, so a change of `r` bumps `PROTOCOL_ID`. Old server + new client: same 27 chunks, unchanged behaviour plus the new errors. Old client + new server: unchanged.
- **Timeouts.** Netcode gives up after 5 s without a packet (`ClientAuthentication::Unsecure` fixes `timeout_seconds = 5`); `CONNECT_TIMEOUT = 10 s` without a new step applies once connected, to `Connecting` (authentication) and `LoadingChunks`. `StartingWorld` (the local server thread is the watchdog) and `Spawning` (local textures only) never time out; Cancel always works.
- **Ownership.** Lot 6 owns `pause.rs` (legacy `Button` + `Interaction`), `hud/chat.rs` (`style::text_font`) and `audio`'s `click_sounds` (now only the pause menu's clicks). Pause's "Save and quit" still lands on the main menu. Lot 4 touches the Solo and Multi screens only for `start_world`/`join_server`, the card focused on return, the Escape race and the Join rule.
- **Goldens.** `connect_error` is new (Task 4). Every other golden (`menu_main`, `menu_solo{,_empty}`, `menu_multi{,_empty}`, settings, inventory) stays byte-identical. The connecting card has no golden (its current step blinks and its content follows the network); it is checked by behaviour and review snapshots. Regenerate only with `UPDATE_GOLDEN=1` after reading the `.actual.png`.

## Decisions (where the spec is silent)

1. **The error card is the dialog widget** (`spawn_dialog_with_hints`) on its own menu screen, `MenuState::ConnectError`, over `menu_background`. Decided because the dialog already gives title, body, right-aligned actions with the primary last, a focused action, Escape and a focus trap; and because a failure tears the session down through the existing transitions (`PreGameLoading → Menu` and `OnExit(Game)`), so the card lives where the menu camera already is. Actions: `Back to Worlds` / `Back to Servers` (Default), `Try again` (Primary, focused). Hints `Tab Move`, `Enter Select`, `Esc Back`.
2. **Where the player lands.** A failure shows the error card; Back, Escape and a cancelled connection go to the list the session came from with its card selected and focused (`MenuState::Solo` / `Multi`). Continue (main menu) counts as the world list: the error is about a world and the list is where it can be renamed or deleted. A card gone meanwhile falls back to the list's usual focus. Pause's "Save and quit" still goes to the main menu (the spec's "to title").
3. **Steps, as they really happen.** World: `Starting the world` (until authenticated: the local server loads, then accepts), `Loading chunks` with `n / 27`, `Spawning` (textures and the local player). Server: `Connecting` (netcode and authentication), `Loading chunks`, `Spawning`. Title `Opening "Survie"` / `Joining "Friends"`; a server shows its typed address under it (`MUTED`). Progress: `0.1` while starting or connecting, `0.2 + 0.7 × n/27` while loading chunks, `0.95` while spawning.
4. **The card's look.** Panel as the dialog's (`PANEL`, raised bevel, `width: 480`, `max_width: 90 %`, padding `SPACE_XL`), centred on the dirt; title `FONT_L`; three rows of a 10 px square marker and a `FONT_M` label (done: `OK` marker, `MUTED` label; current: `TEXT` marker blinking 0.5 s on / 0.5 s off on `Time<Real>`, `TEXT` label; pending: hollow `BEVEL_HI` marker, `MUTED` label), `12 / 27` right-aligned on the chunk row; a 12 px progress bar (sunken, `SLOT`, fill `OK` — `ACCENT` is reserved for focus, selection and sliders); `Cancel` (Default) bottom-right, `AutoFocus`; hint bar `Esc Cancel`. The blinking marker is the only motion: the old animated dots go.
5. **Reasons without a protocol change** (`failure_of`). Before authentication: `ConnectionDenied` → full; `DisconnectedByServer` → refused (the server drops a name in use or invalid), or invalid name when `sanitize_username` refuses ours; any timeout or no reason → no answer, then for a server Lot 3's status is asked once: `Incompatible` → another version, else no answer. After authentication, before the game: lost while loading. In game: `DisconnectedByServer` → the server stopped (a dedicated server only disconnects a playing client when it shuts down), anything else → connection lost. Server full is unit-tested only (filling 64 places is not a test).
6. **The name is checked before connecting.** An invalid player name (`sanitize_username` is `None`) ends the session on entering `PreGameLoading`, before a local server starts or a packet leaves: every server would refuse it.
7. **Copy** (`error_text(error, origin)`; `N` the world or server name, `A` its typed address, `name` the player's):

   | Error | Title | Body |
   |---|---|---|
   | NoAnswer | `Cannot reach "N"` | `No answer from A. Check that the server is running and that its port is open, then try again.` |
   | Incompatible | `"N" runs another version` | `This server runs another version of Rustcraft. Join it with the same version as the server.` |
   | Full | `"N" is full` | `The server has no room for another player. Try again later.` |
   | Refused | `"N" refused you` | `The server closed the connection before letting you in. Another player may already be using the name "name".` |
   | InvalidName | `Your name is not accepted` | `Player names have 1 to 32 characters and no control characters. Start the game with another --player-name.` |
   | LostWhileLoading | `Connection to "N" lost` | `The server stopped answering while the world was loading.` (world: `The world stopped while loading.`) |
   | WorldFailed(reason) | `Cannot open "N"` | `The world could not be loaded: reason` |
   | WorldBusy | `Cannot open "N"` | `The world is still closing from the last session. Try again in a moment.` |
   | NoConnection(reason) | `Cannot connect` | `Could not open a network connection: reason` |
   | ServerStopped | `"N" stopped` | `The server shut down or closed your connection.` (world: `The world stopped unexpectedly.`) |
   | ConnectionLost | `Connection to "N" lost` | `The server stopped answering. Check your network, then try again.` |

8. **Chunks before the game.** The client polls the server's messages from the authentication on (not only in the game), counts the `(2r+1)³` chunks around the local player's chunk, and enters the game once all are in, textures loaded and the player spawned. Decided because the spec's `n / total` needs a real total, and the server generates and sends exactly that cube (`get_all_active_chunks(players, r)`), so the player never spawns over the void. Requests to mesh written while loading are lost (`world_render_system` reads messages in the game only): entering the game asks a mesh for every chunk held. Leaving the loading screen without playing forgets the chunks, as leaving the game does.
9. **A world that starts slowly.** While `StartingWorld` and the local server thread still runs, a dropped connection (netcode gave up after 5 s) is opened again (`open_connection`) instead of failing; the same in `finish_stopping_local_server`, so a server left running by a cancel is always told to stop. Try again while the previous local server still runs says `WorldBusy` instead of silently returning to the menu.
10. **Join stays enabled for Offline and Unknown host cards** (Lot 3 parked item). Decided because an old server without a status port reads Offline yet accepts players, a status can be stale, and the connecting card now says why within about 5 s and can be cancelled at once; an unknown host already fails in the pane with `Cannot find …`. Incompatible and Bad address stay disabled.
11. **Escape in the frame a join resolves** (Lot 3 parked item): whichever of `escape_button` and `finish_join` runs first wins and the other stands down (rule above), so the state is never "main menu over a connection". No ordering constraint is added.
12. **The flaky `the_connection_screen_shows_then_cancel_returns_to_the_menu`.** Cause: the legacy screen's text uses `FiraSans`, which nothing loads before; its load is queued in the same `OnEnter(PreGameLoading)` chain right after `setup_materials` queues every block and item texture, and the test waits for that text with `SCREEN` (2 s, wall clock). Alone the font arrives in time (3 runs of 3); under a full parallel lavapipe run it sometimes does not (failed once in Lot 1's full run) — the class of `781ac09` ("a longer timeout for the first screen"). Lot 3's fix (wait for the text before clicking) was for `[Cancel]` moving once that font arrived. Fix: the card uses only fonts the menus already hold (an `AssetServer::load` of a path whose handles were dropped in the same frame revives the asset, `bevy_asset` `handle_drops_to_skip`), so it is laid out on its first frame; the test pins that.

## Review Focus

1. A disconnection noticed in the frame the player chose to leave — Cancel, Escape, "Save and quit" — never shows an error, and a failure in the frame of a Cancel shows none either. (Task 2: `the_first_end_of_a_session_wins`; Task 4: `a_cancel_in_the_frame_of_a_failure_shows_no_error`; Task 6: `leaving_on_purpose_is_never_an_error`)
2. Chunks of a cancelled or failed session never leak into the next one (the count would be wrong and another world's blocks would show). (Task 3: `cancelling_the_loading_screen_forgets_the_chunks_it_received`)
3. Try again after a world failed never starts a second server on the same save, and a local server left running by a give-up is still told to stop. (Task 5: `a_world_that_cannot_load_says_why_and_returns_to_its_card`, `a_server_left_running_is_stopped_even_after_the_connection_gave_up`)
4. The card the player came from was deleted while the error card showed: Back lands on the list's usual focus, no panic. (Task 5: the second half of `a_world_that_cannot_load_says_why_and_returns_to_its_card`)
5. A 32-character name and a long reason (a path in a server error) stay inside the error card at 800×600. (Task 5: `a_long_error_stays_inside_its_card_at_800x600`)
6. The Escape that cancels the connecting card is not seen again by the list it lands on (which would go on to the main menu). (Task 2: `escape_cancels_the_connection_and_returns_to_the_server`)

## Defects of the e2e spec fixed here

"The connection screen never gives up and has no error" (Tasks 4, 5, 6); "Escape does not cancel it" (Task 2); "failures return to the main menu, not to the list" (Tasks 2, 4). Parked items: an old client (another `PROTOCOL_ID`) waits forever on "Connecting" (Task 4, the netcode give-up then the status verdict); the flaky connection test (Task 2, Decision 12); Escape in the frame a join resolves (Task 2, Decision 11); Join for Offline / Unknown host (Task 4, Decision 10).

## Files

- `shared/src/constants.rs`, `server/src/world/{broadcast_world,simulation}.rs`, `client/src/network/{session,mod}.rs` — the session model (Task 1).
- `client/src/ui/widgets/{progress,mod}.rs`, `client/src/ui/menus/{connecting,mod,worlds,home}.rs`, `client/src/ui/menus/{solo,multi}/mod.rs`, `client/src/game.rs`, `client/src/ui/assets.rs`; deleted `client/src/ui/menus/server_connect_loading.rs`, `data/fonts/FiraSans-SemiBold.ttf` — the card (Task 2).
- `client/src/network/{mod,session}.rs`, `client/src/world/mod.rs`, `client/src/game.rs` — chunks before the game (Task 3).
- `client/src/ui/menus/connect_error.rs` (new), `client/src/ui/widgets/dialog.rs`, `client/src/network/{session,setup,cleanup}.rs` — failures and the error card (Tasks 4, 5); `client/src/network/session.rs` — in game (Task 6).
- `client/src/e2e/{connecting,raw_client}.rs` (new), `client/src/e2e/{game,multi,solo,seed,mod,review}.rs`, `client/tests/golden/connect_error.png`, `README.md`, `docs/ARCHITECTURE.md`.

---

### Task 1: The session in plain functions — steps, progress, reasons, chunks

**Files:**
- Create: `client/src/network/session.rs`
- Modify: `client/src/network/mod.rs` (`pub mod session;`), `shared/src/constants.rs`, `server/src/world/broadcast_world.rs` (`BROADCAST_RENDER_DISTANCE` goes), `server/src/world/simulation.rs` (the literal `1` of `generate_missing_chunks` goes)

**Interfaces:**
- Produces, in `shared/src/constants.rs`:
  ```rust
  /// The chunks within this many chunks of a player are generated and sent to its client, which
  /// waits for those around its spawn before entering the game: bump `PROTOCOL_ID` with it.
  pub const BROADCAST_CHUNK_RADIUS: i32 = 1;
  ```
- Produces, in `client/src/network/session.rs`:
  ```rust
  /// Where the session being opened or played was started from.
  #[derive(Resource, Clone, Debug, PartialEq, Eq)]
  pub enum SessionOrigin {
      World { name: String },
      /// `ip` as typed in the server list; `address` what the join resolved it to.
      Server { name: String, ip: String, address: SocketAddr },
  }
  impl SessionOrigin { pub fn name(&self) -> &str; pub fn is_world(&self) -> bool; }
  /// How a session ended other than by the pause menu; read by the menus when they open.
  /// `error: None` is a cancel.
  #[derive(Resource, Clone, Debug, PartialEq, Eq)]
  pub struct SessionEnd { pub origin: SessionOrigin, pub error: Option<ConnectError> }
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum ConnectStep { StartingWorld, Connecting, LoadingChunks { received: u32, total: u32 }, Spawning }
  #[derive(Clone, Debug, PartialEq, Eq)]
  pub enum ConnectError { NoAnswer, Incompatible, Full, Refused { name: String }, InvalidName { name: String },
                          LostWhileLoading, WorldFailed(String), WorldBusy, NoConnection(String),
                          ServerStopped, ConnectionLost }
  /// How far a session got before its connection dropped.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Reached { Connecting, Authenticated, InGame }
  pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
  /// What a dropped connection means (Decision 5); `name` is the player's.
  pub fn failure_of(reached: Reached, reason: Option<NetcodeDisconnectReason>, name: &str) -> ConnectError;
  /// The step of a session; `chunks` is `None` while the local player has not spawned.
  pub fn connect_step(origin: &SessionOrigin, authenticated: bool, chunks: Option<(u32, u32)>) -> ConnectStep;
  pub fn progress(step: ConnectStep) -> f32;                                   // Decision 3
  /// How many of the chunks within `radius` of the chunk `center` `map` holds, and how many there are.
  pub fn chunks_around(map: &ChunkMap, center: IVec3, radius: i32) -> (u32, u32);
  /// Whether a dropped connection is opened again: a world still starting whose server runs.
  pub fn reconnect_to_world(origin: &SessionOrigin, step: ConnectStep, local_server_running: bool) -> bool;
  ```
  `connect_step`: not authenticated → `StartingWorld` (world) / `Connecting` (server); authenticated with `None` → `LoadingChunks { received: 0, total: (2r+1)³ }`; `Some((n, t))` with `n < t` → `LoadingChunks`; else `Spawning`.

- [ ] **Step 1: Write the failing tests** (unit, `session.rs`)
  - `a_dropped_connection_reads_as_what_the_player_can_do` — `(Connecting, ConnectionRequestTimedOut | ConnectionResponseTimedOut | ConnectTokenExpired | None, "Tester") → NoAnswer`; `(Connecting, ConnectionDenied) → Full`; `(Connecting, DisconnectedByServer, "Tester") → Refused { name: "Tester" }`; `(Connecting, DisconnectedByServer, "") → InvalidName { name: "" }`; `(Authenticated, DisconnectedByServer | ConnectionTimedOut | None) → LostWhileLoading`; `(InGame, DisconnectedByServer) → ServerStopped`; `(InGame, ConnectionTimedOut | None) → ConnectionLost`.
  - `the_steps_follow_the_session` — world, not authenticated → `StartingWorld`; server → `Connecting`; authenticated, `None` → `LoadingChunks { 0, 27 }`; `Some((12, 27))` → `LoadingChunks { 12, 27 }`; `Some((27, 27))` → `Spawning`.
  - `progress_only_moves_forward` — `Connecting` 0.1 < `LoadingChunks{0,27}` 0.2 < `{13,27}` < `{27,27}` 0.9 < `Spawning` 0.95 ≤ 1.
  - `the_chunks_around_the_spawn_are_counted` — chunks at `(0,0,0)`, `(1,0,-1)`, `(5,5,5)` → `chunks_around(.., IVec3::ZERO, 1) == (2, 27)`.
  - `a_world_still_starting_is_reconnected` — `(World, StartingWorld, true) → true`; `(World, StartingWorld, false)`, `(World, LoadingChunks{..}, true)`, `(Server, Connecting, true) → false`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client network::session` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces; the server's two uses read `shared::BROADCAST_CHUNK_RADIUS`. `dead_code` on what Tasks 2–6 use first, each naming its task.

- [ ] **Step 4: Run the tests and the suite** → PASS (the server's broadcast tests unchanged); full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): what a session reached and why it ended, as plain functions"`

---

### Task 2: The connecting card — steps, progress, Cancel and Escape back to the list

**Files:**
- Create: `client/src/ui/widgets/progress.rs`, `client/src/ui/menus/connecting.rs`, `client/src/e2e/connecting.rs`
- Modify: `client/src/ui/widgets/mod.rs`, `client/src/ui/menus/mod.rs`, `client/src/ui/menus/{worlds,home}.rs`, `client/src/ui/menus/solo/mod.rs`, `client/src/ui/menus/multi/mod.rs`, `client/src/network/{session,mod}.rs`, `client/src/game.rs`, `client/src/ui/assets.rs`, `client/src/e2e/{mod,game,multi,solo}.rs`
- Delete: `client/src/ui/menus/server_connect_loading.rs`, `data/fonts/FiraSans-SemiBold.ttf`

**Interfaces:**
- Consumes: Task 1.
- Produces, in `widgets/progress.rs`:
  ```rust
  /// The value of a progress bar, 0 to 1, set by its screen.
  #[derive(Component)] pub struct ProgressBar(pub f32);
  /// A 12 px sunken `SLOT` track with an `OK` fill, `Role::ProgressIndicator` labelled `label`.
  pub fn progress_bar(label: &str) -> impl Bundle;
  pub(super) fn plugin(app: &mut App);   // the fill's width and the numeric value (0–100) follow, clamped
  ```
- Produces, in `network/session.rs`:
  ```rust
  /// The session being opened: its step and when it last changed. Exists in `PreGameLoading` only.
  #[derive(Resource)] pub struct ConnectWatch { pub step: ConnectStep, since: Instant }
  /// Ends the session as `error` (`None`: cancelled) unless it already ended this frame: inserts
  /// `SessionEnd` and sets `GameState::Menu`. Returns whether it did.
  pub fn end_session(commands: &mut Commands, game: &mut NextState<GameState>, origin: &SessionOrigin,
                     error: Option<ConnectError>) -> bool;
  fn start_watching(...);     // OnEnter(PreGameLoading); OnExit removes ConnectWatch
  fn track_connection(...);   // Update, PreGameLoading: connect_step(origin, target.state is ConnectionEstablished|FullyReady, None)
  pub fn session_plugin(app: &mut App);   // added by network_plugin
  ```
- Produces, in `ui/menus/connecting.rs`:
  ```rust
  #[derive(Component)] pub struct ConnectingCard;
  #[derive(Component)] pub struct StepRow(pub usize);
  #[derive(Component)] pub struct CancelConnect;
  pub const CONNECT_HINTS: [Hint; 1];                       // Esc Cancel
  pub fn connecting_title(origin: &SessionOrigin) -> String; // Opening "Survie" | Joining "Friends"
  pub fn step_labels(origin: &SessionOrigin) -> [&'static str; 3];
  pub fn setup_connecting_card(...);   // OnEnter(PreGameLoading), in game.rs's chain in place of the legacy setup
  fn show_step(...);                   // Update, PreGameLoading, when ConnectWatch changed: markers, labels, 12 / 27, ProgressBar
  fn blink_current_step(...);          // Update, PreGameLoading
  fn cancel_on_activate(...); fn cancel_on_escape(...);   // end_session(.., None)
  pub fn connecting_plugin(app: &mut App);
  ```
  Root: `menu_background`, `DespawnOnExit(GameState::PreGameLoading)`, `TabGroup::new(0)`; the card of Decision 4 with `ConnectingCard` (`AccessibilityNode(Role::Group)`, `AccessibleLabel` = the title); `progress_bar("Progress")`; `button("Cancel", Default)` + `CancelConnect` + `AutoFocus`; `hint_bar(&CONNECT_HINTS, ..)`. Text wraps with `LineBreak::WordOrCharacter`.
- Changes: `pub fn start_world(name: &str, selected: &mut SelectedWorld, commands: &mut Commands, game: &mut NextState<GameState>, menu: &mut NextState<MenuState>)` inserts `SessionOrigin::World`; `pub fn join_server(server: &ServerItem, address: SocketAddr, target: &mut TargetServer, commands: &mut Commands, game: &mut NextState<GameState>, menu: &mut NextState<MenuState>)` inserts `SessionOrigin::Server`, and `PendingJoin` keeps its `ServerItem`; both do nothing while `NextState<GameState>` is pending. `finish_join` drops a finished join without handing it over while `NextState<MenuState>` is pending; `escape_button` does nothing while `NextState<GameState>` is pending (Decision 11). In `menus/mod.rs`:
  ```rust
  /// The first screen of the menus: the list a cancelled session came from, else the main menu
  /// (the error card from Task 4).
  pub fn first_menu(end: Option<&SessionEnd>) -> MenuState;
  /// The card a list screen focuses on opening: its session's origin, which it then forgets.
  fn returning_card(end: Option<&SessionEnd>, world: bool) -> Option<String>;
  ```
  `menu_setup` sets `first_menu(..)`; `solo_setup` / `multi_setup` trigger `WorldsChanged` / `ServersChanged` with `focus: returning_card(..)` and remove `SessionEnd`. `assets.rs` loses `SANS_FONT_PATH`; `menus/mod.rs` loses the legacy module and its `Update` system.
- Produces, in `e2e/connecting.rs`: `fn nowhere() -> GameFolder` (`with_servers(&[("Nowhere", "127.0.0.1:9")])`); `fn join_nowhere() -> Harness` (Multiplayer, `join_server_card("Nowhere")`, `wait_for_game_state(PreGameLoading, SCREEN)`); `fn card_title(title: &str) -> L` (`L::text(title).within(L::marker::<ConnectingCard>())`). `selected_server` (multi) and `selected_world` (solo) become `pub(super)`.

- [ ] **Step 1: Write the failing tests**

Unit:
- `progress.rs`: `the_fill_follows_the_value` — `ProgressBar(0.25)` → the fill's `Node.width == percent(25.)`, the node's role `ProgressIndicator` with numeric value `25.0`; `1.5` → `100`; `-1.0` → `0`.
- `menus/mod.rs`: `the_menus_open_where_a_cancelled_session_came_from` — `None` → `Main`; `SessionEnd { World, None }` → `Solo`; `{ Server, None }` → `Multi`.
- `menus/mod.rs`: `escape_stands_down_in_the_frame_the_menus_are_left` — bare `App` with states and `escape_button`, `MenuState::Multi`, Escape just pressed, `NextState<GameState>` = `Pending(PreGameLoading)` → `NextState<MenuState>` is `Unchanged`.
- `multi/mod.rs`: `a_join_resolved_in_the_frame_escape_leaves_hands_nothing_over` — as Lot 3's `a_join_resolved_after_the_screen_closed…`, with `NextState<MenuState>` = `Pending(Main)` → `TargetServer.address == None`, `NextState<GameState>` unchanged, no `PendingJoin` left.
- `session.rs`: `the_first_end_of_a_session_wins` — one system calls `end_session(.., Some(NoAnswer))` then `end_session(.., None)` → `true`, `false`; `SessionEnd.error == Some(NoAnswer)`, `NextState<GameState> == Pending(Menu)`.

E2e, `e2e/connecting.rs`:
- `the_connecting_card_is_drawn_on_its_first_frame` (the flake's guard, Decision 12) — `join_nowhere()`, `frames(1)` → `L::role(Role::Button, "Cancel")` and `card_title("Joining \"Nowhere\"")` visible; every `TextFont` under `ConnectingCard` has a font that `AssetServer::is_loaded`.
- `the_connecting_card_shows_its_steps_and_cancel_returns_to_the_server` (replaces `e2e::game::the_connection_screen_shows_then_cancel_returns_to_the_menu`, journey kept) — within `ConnectingCard`: `"127.0.0.1:9"`, `"Connecting"`, `"Loading chunks"`, `"Spawning"`; `L::role(Role::ProgressIndicator, "Progress")` visible; `focus_label() == Some("Cancel")`; click Cancel → `GameState::Menu`, `menu_state() == Multi`, `selected_server == Some("Nowhere")`, `focus_label() == Some("Nowhere")`.
- `escape_cancels_the_connection_and_returns_to_the_server` (e2e defect; Review Focus 6) — `join_nowhere()`, Escape → `Multi` with Nowhere selected; `frames(3)` → still `Multi`.
- `e2e/multi.rs`'s `join_hands_a_host_name_over_with_the_default_port`: its `[Cancel]` wait and click become `click(L::role(Role::Button, "Cancel"))`; it ends with `menu_state() == Multi` and `"Home"` selected.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- widgets::progress menus:: network::session e2e::connecting e2e::multi::join_hands` → compile errors, then no `Cancel` button.

- [ ] **Step 3: Implement** as in Interfaces; delete the legacy screen, `SANS_FONT_PATH` and the FiraSans file; the "loading chunks" row stays pending until Task 3.

- [ ] **Step 4: Run the tests and the suite** — run `e2e::connecting::the_connecting_card_is_drawn_on_its_first_frame` 5 times and the full suite twice: green each time; no golden changed.

- [ ] **Step 5: Commit** (the report lists the replaced test and the changed locators) — `git commit -m "feat(client): a connecting card with its steps, and a cancel that returns to the list"`

---

### Task 3: Chunks before the game

**Files:**
- Modify: `client/src/network/{mod,session}.rs`, `client/src/world/mod.rs`, `client/src/game.rs`, `client/src/e2e/connecting.rs`

**Interfaces:**
- Consumes: `chunks_around`, `connect_step`, `ConnectWatch`, `BROADCAST_CHUNK_RADIUS`.
- Produces:
  ```rust
  // network/mod.rs
  /// Whether the server accepted the player: `ConnectionEstablished` or `FullyReady`.
  pub fn authenticated(target: Res<TargetServer>) -> bool;
  // poll_network_messages runs in FixedPreUpdate in Game, and in PreGameLoading once authenticated
  // world/mod.rs
  /// Asks a mesh for every chunk held on entering the game: requests written while loading were not read.
  fn remesh_every_chunk(world: Res<ClientWorldMap>, mut ev_render: MessageWriter<WorldRenderRequestUpdateEvent>); // OnEnter(Game)
  // game.rs
  /// The ways out of a session: OnExit(Game) and OnTransition { PreGameLoading → Menu } both run
  /// (clear_resources, terminate_server_connection).chain().
  pub(crate) fn add_session_exits(app: &mut App);
  ```
  `track_connection` passes `Some(chunks_around(&world.blocks, world_position_to_chunk_position(local player translation), BROADCAST_CHUNK_RADIUS))` once a `CurrentPlayerMarker` exists. `check_pre_loading_complete` enters the game once `textures_loaded`, `FullyReady` and `ConnectWatch.step == Spawning`.

- [ ] **Step 1: Write the failing tests**

Unit:
- `session.rs`: `the_step_counts_the_chunks_around_the_local_player` — bare `App` with `track_connection`, `SessionOrigin::World`, `TargetServer` `FullyReady`, a `CurrentPlayerMarker` at `(8., 40., 8.)` (chunk `(0, 2, 0)` for `CHUNK_SIZE` 16), 12 of the 27 chunks around it → `LoadingChunks { received: 12, total: 27 }`; the 15 others added → `Spawning`.
- `world/mod.rs`: `chunks_received_while_loading_are_meshed_in_game` — states in `PreGameLoading`, `ClientWorldMap` with chunks `(0,0,0)` and `(0,-1,0)`, `remesh_every_chunk` on `OnEnter(Game)`; set `Game` → the drained `WorldRenderRequestUpdateEvent`s include `ChunkToReload` of both.
- `game.rs`: `cancelling_the_loading_screen_forgets_the_chunks_it_received` (Review Focus 2) — `MinimalPlugins`, `StatesPlugin`, `insert_state(PreGameLoading)`, the resources `terminate_server_connection` needs, a `ClientWorldMap` with one chunk, `add_session_exits`; set `Menu` → no chunk left.

E2e, `e2e/connecting.rs`: `a_world_opens_once_the_chunks_around_the_player_are_loaded` — `with_world("Survie")`, Singleplayer, `play_world("Survie")` → `card_title("Opening \"Survie\"")` and `"Starting the world"` within the card; then `wait_until("the game", SERVER, …)` checks on the first frame of `GameState::Game` that `chunks_around(blocks, player's chunk, 1) == (27, 27)`; within `SCREEN`, some chunk of that cube has `ClientWorldMap.render[pos].entity.is_some()`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- session world:: game:: e2e::connecting::a_world_opens` → the count is 0 on entering the game.

- [ ] **Step 3: Implement** as in Interfaces; `game_plugin` registers its exits through `add_session_exits`.

- [ ] **Step 4: Run the tests and the suite** → PASS; every journey through a server still green (`enter_world` waits for `GameState::Game` as before).

- [ ] **Step 5: Commit** — `git commit -m "feat(client): enter the game once the chunks around the spawn are loaded"`

---

### Task 4: Failures while joining a server, and the error card

**Files:**
- Create: `client/src/ui/menus/connect_error.rs`, `client/tests/golden/connect_error.png`
- Modify: `client/src/network/{session,setup}.rs`, `client/src/ui/widgets/dialog.rs`, `client/src/ui/menus/mod.rs` (`MenuState::ConnectError`, `first_menu`), `client/src/game.rs` (the name check first in the `OnEnter(PreGameLoading)` chain), `client/src/e2e/{connecting,multi}.rs`

**Interfaces:**
- Consumes: Tasks 1–3, `StatusProbe`, `InFlight`, `spawn_dialog`'s `Dialog`, `DialogClosed`.
- Produces, in `network/session.rs`:
  ```rust
  #[derive(Resource)] pub struct ConnectTimeout(pub Duration);      // Default: CONNECT_TIMEOUT
  // ConnectWatch gains `verdict: Option<InFlight<ServerStatus>>`: the status asked once a server gave no answer
  /// Ends a session that failed: the transport dropped (failure_of), CONNECT_TIMEOUT without a new
  /// step while Connecting or LoadingChunks, then for a server's NoAnswer the status verdict.
  fn watch_connection(...);                    // Update, PreGameLoading, after track_connection
  pub fn player_name_accepted(profile: Res<CurrentPlayerProfile>) -> bool;
  fn refuse_an_invalid_name(...);              // first in the OnEnter chain: end_session(InvalidName)
  ```
  `launch_local_server_system` and `init_server_connection` run only if `player_name_accepted`; `init_server_connection`'s failures (no address, no socket) call `end_session(NoConnection(reason))` instead of `GameState::Menu`.
- Produces, in `network/setup.rs`: `pub fn open_connection(addr: SocketAddr, client_id: u64) -> Result<(RenetClient, NetcodeClientTransport), String>` (what `init_server_connection` did; Task 5 reuses it).
- Produces, in `widgets/dialog.rs`: `pub fn spawn_dialog_with_hints(commands: &mut Commands, dialog: &Dialog, hints: &[Hint], font: Handle<Font>, focus: &InputFocus) -> Entity`.
- Produces, in `ui/menus/connect_error.rs`:
  ```rust
  pub const TRY_AGAIN: &str = "Try again";
  pub const ERROR_HINTS: [Hint; 3];                                    // Tab Move, Enter Select, Esc Back
  pub fn back_label(origin: &SessionOrigin) -> &'static str;           // "Back to Worlds" | "Back to Servers"
  /// The title and the body of the error card (Decision 7).
  pub fn error_text(error: &ConnectError, origin: &SessionOrigin) -> (String, String);
  fn connect_error_setup(...);   // OnEnter(MenuState::ConnectError): menu_background root (DespawnOnExit), the dialog, Try again focused
  fn act_on_error_card(...);     // DialogClosed: Some(1) → remove SessionEnd, start_world / join_server again; else → Solo / Multi
  pub fn connect_error_plugin(app: &mut App);
  ```
  `first_menu`: an error → `MenuState::ConnectError`. Back keeps `SessionEnd` so the list focuses the card (Task 2's `returning_card`).

- [ ] **Step 1: Write the failing tests**

Unit:
- `connect_error.rs`: `every_error_says_what_happened_and_what_to_do` — each row of Decision 7 for `Server { "Nowhere", "127.0.0.1:9", .. }` and the world rows for `World { "Survie" }`: exact titles; bodies as in the table (`WorldFailed("bad ron")` → `"The world could not be loaded: bad ron"`); `back_label` both ways.
- `session.rs` (bare `App`, states, `watch_connection`, a `RenetClient` still connecting without a transport, `ConnectWatch` built with `since` back-dated):
  - `a_server_silent_for_too_long_times_out` — `Connecting`, `since` 11 s ago, `StatusProbe(|_| Offline)` → updated until `SessionEnd.error == Some(NoAnswer)` and `Pending(Menu)`; with 9 s → nothing.
  - `a_silent_server_of_another_version_says_so` — `StatusProbe(|_| Incompatible)` → `Incompatible`.
  - `a_stall_while_loading_chunks_is_a_lost_connection` — `LoadingChunks { 3, 27 }`, authenticated, 11 s → `LostWhileLoading`, the probe never called (it panics).
  - `the_spawning_step_never_times_out` — `Spawning`, 60 s → no `SessionEnd`.
  - `a_cancel_in_the_frame_of_a_failure_shows_no_error` (Review Focus 1) — `NextState<GameState>` already `Pending(Menu)`, stalled → no `SessionEnd`.
- `menus/mod.rs`: `the_menus_open_where_a_cancelled_session_came_from` gains `{ Server, Some(NoAnswer) } → ConnectError`.

E2e:
- `e2e/connecting.rs`: `a_server_that_never_answers_times_out_then_says_it_runs_another_version` (e2e defect "never gives up"; parked "old client waits forever") — `nowhere()` with the real probe: `"Offline"` within its card and Join enabled (Decision 10); join at `t0` → `L::role(Role::Dialog, "Cannot reach \"Nowhere\"")` within `SERVER`, `t0.elapsed() >= 4 s`; `L::text_containing("127.0.0.1:9")` within it; `focus_label() == Some(TRY_AGAIN)`; `menu_state() == ConnectError`, `game_state() == Menu`; insert `StatusProbe(|_| ServerStatus::Incompatible)`, Enter → `card_title("Joining \"Nowhere\"")`, then `L::role(Role::Dialog, "\"Nowhere\" runs another version")` within `SERVER`; Escape → `Multi`, `selected_server == Some("Nowhere")`, `focus_label() == Some("Nowhere")`.
- `e2e/connecting.rs`: `an_invalid_player_name_is_said_before_connecting` — `Options { player_name: "".into(), ..default() }`, `nowhere()`, join → `L::role(Role::Dialog, "Your name is not accepted")` within `SCREEN`; no `NetcodeClientTransport` resource; `hover_at(Vec2::ZERO)`; golden `connect_error`; click `"Back to Servers"` → `Multi`, Nowhere selected.
- `e2e/multi.rs`: `join_stays_enabled_for_offline_and_unknown_host_servers` (parked item) — `status_list()`: select `"Friends"` (Offline) and `"Typo"` (Unknown host) → Join has no `InteractionDisabled`; `"Old friends"` → disabled.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- connect_error network::session menus:: e2e::connecting e2e::multi::join_stays` → compile errors, then the connecting card never gives up.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests; generate the golden on purpose** — read `connect_error.actual.png`: the dirt under the dialog backdrop, title `Your name is not accepted`, the body on two or three lines, `Back to Servers` then a green `Try again` with its ring, the three hints, nothing else. `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- an_invalid_player_name`, rerun → PASS, no leftovers. Full suite green, no other golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a connection that fails says why, with Try again and the way back to its list"`

---

### Task 5: When a world cannot start

**Files:**
- Modify: `client/src/network/{setup,cleanup,session}.rs`, `client/src/e2e/{connecting,seed}.rs`

**Interfaces:**
- Consumes: `open_connection`, `reconnect_to_world`, `end_session`, `watch_connection`.
- Produces:
  ```rust
  /// The thread of the embedded solo server, with the error it stopped on.
  #[derive(Resource)] pub struct LocalServerHandle(pub Option<JoinHandle<Result<(), String>>>);
  ```
  `launch_local_server_system`: the previous server still running → `end_session(WorldBusy)`; no socket → `NoConnection(reason)`; no world selected → `WorldFailed("No world was selected")`. `watch_connection`, for a world: the thread finished → joined → `Err(e)` → `WorldFailed(e)`, `Ok(())` or a panic → `WorldFailed("The world stopped while opening")`; the client disconnected and `reconnect_to_world` → `open_connection` replaces `RenetClient` and the transport, `since` reset, no end. `finish_stopping_local_server`: its transport disconnected while the thread runs → `open_connection` again so `Exit` can be sent (Decision 9). `wait_for_local_server` logs an `Err` it joins. In `e2e/seed.rs`: `GameFolder::with_broken_world(self, name: &str) -> Self` (`saves/<name>/world.ron` = `"not ron"`).

- [ ] **Step 1: Write the failing tests**

Unit:
- `cleanup.rs`: `a_server_left_running_is_stopped_even_after_the_connection_gave_up` (Review Focus 3) — `SoloSession::connecting("gave-up-test", 12)`, `transport.disconnect()`, `finish_stopping_local_server` in `Update` → `update_until` the handle is `None`; `world.ron` saved. The existing tests spawn `server::init(..)` returning its `Result`.
- `setup.rs`: `a_second_local_server_is_refused_while_the_first_one_runs` now also asserts `SessionEnd.error == Some(WorldBusy)`.
- `session.rs`: `a_world_whose_server_stopped_says_why` — a finished thread returning `Err("Failed to load world Broken: bad")` → `WorldFailed` with that text; `a_world_still_starting_gets_a_new_connection` — a thread parked on a channel (alive), a disconnected client → a new transport resource, no `SessionEnd`; release the thread.

E2e, `e2e/connecting.rs`:
- `a_world_that_cannot_load_says_why_and_returns_to_its_card` — `with_world("Alpha").with_broken_world("Broken")`; Singleplayer, `play_world("Broken")` → `L::role(Role::Dialog, "Cannot open \"Broken\"")` within `SERVER`, `"could not be loaded"` within it; Enter (Try again) → the same dialog again, `"still closing"` not shown; Escape → `Solo`, `selected_world == Some("Broken")`; Play again, then `fs::remove_dir_all(saves/Broken)` while the card shows, Escape → `Solo`, `selected_world == Some("Alpha")` (Review Focus 4).
- `a_long_error_stays_inside_its_card_at_800x600` (Review Focus 5) — 800×600, `with_broken_world` named 32 × `W` → title and body `rect_of` inside the dialog panel's rect.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- network:: e2e::connecting::a_world e2e::connecting::a_long` → the broken world shows `Opening "Broken"` until the timeout.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a world that cannot start says why, and a slow one is waited for"`

---

### Task 6: Refused, then disconnected in game — the server journey

**Files:**
- Create: `client/src/e2e/raw_client.rs`
- Modify: `client/src/network/session.rs`, `client/src/e2e/{connecting,mod}.rs`

**Interfaces:**
- Consumes: `failure_of(Reached::InGame, ..)`, `end_session`, the error card.
- Produces, in `network/session.rs`: `fn watch_session(...)` — `Update`, `in_state(Game)`: the `RenetClient` disconnected and `NextState<GameState>` not pending → `end_session(failure_of(InGame, transport.disconnect_reason(), name))`.
- Produces, in `e2e/raw_client.rs`:
  ```rust
  /// Another player's game: a bare netcode client on its own thread, pumped every 16 ms.
  pub struct RawClient { .. }
  impl RawClient {
      /// Connects to `server` as `client_id`; authenticates as `name` when given.
      pub fn join(server: SocketAddr, client_id: u64, name: Option<&str>) -> Self;
      /// Waits until connected (and authenticated, when a name was given). Panics after `timeout`.
      pub fn wait_ready(&self, timeout: Duration);
      /// Sends `Exit` — which stops a solo server — then disconnects.
      pub fn exit(self);
      /// Disconnects without a word.
      pub fn leave(self);
  }
  ```

- [ ] **Step 1: Write the failing tests**

Unit, `session.rs`:
- `leaving_on_purpose_is_never_an_error` (Review Focus 1) — `watch_session` with a disconnected `RenetClient` and `NextState<GameState>` already `Pending(Menu)` → no `SessionEnd`.
- `a_connection_dropped_in_game_ends_the_session` — `RenetClient::disconnect()`, no transport → `SessionEnd.error == Some(ConnectionLost)`, `Pending(Menu)`.

E2e, `e2e/connecting.rs`: `a_refused_join_says_why_and_a_stopped_server_sends_the_player_back_to_its_card` (the server journey) — a server from `acquire_local_socket_pair` + `init_with_status` with `GameServerConfig { is_solo: true, .. }` (so that any client's `Exit` stops it); an intruder `RawClient::join(addr, 77, Some("Tester"))`, `wait_ready`; folder `with_servers(&[("Local test", "127.0.0.1:{port}")])`, real probe; Multiplayer → `"1 / 64"` within its card; join → `L::role(Role::Dialog, "\"Local test\" refused you")`, `"\"Tester\""` within it, `focus_label() == Some(TRY_AGAIN)`; `intruder.leave()`; Enter → `card_title("Joining \"Local test\"")`, `GameState::Game` within `SERVER`; `RawClient::join(addr, 78, None)` then `exit()` → `L::role(Role::Dialog, "\"Local test\" stopped")` within `SERVER`, `game_state() == Menu`; Escape → `Multi`, `selected_server == Some("Local test")`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- network::session e2e::connecting::a_refused` → the stopped server leaves the player in the game.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green and under ~2 minutes more than before the lot.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): a server that refuses or stops sends the player back to its card with the reason"`

---

### Task 7: Review snapshots, legacy left behind, positions, docs

**Files:**
- Modify: `client/src/e2e/review.rs`, `client/src/audio/mod.rs` (doc of `click_sounds`), `README.md`, `docs/ARCHITECTURE.md`, plus whatever the review finds in the files of Tasks 2–6

- [ ] **Step 1: Extend the review journey** — with `nowhere()` plus a world named 32 × `W` made broken: per resolution, join Nowhere → `"connecting"`, Cancel. Then, once (one netcode give-up), join Nowhere and wait for the error card; per resolution resize → `"connect_error_no_answer"`. Then play the broken world; per resolution → `"connect_error_world_long"`.

- [ ] **Step 2: Run and look** — `UI_SNAPSHOTS=1 timeout 600 cargo test -q -p client review -- --ignored`, then read every `target/ui-snapshots/every_screen_at_three_resolutions/{connecting,connect_error_*}_{800x600,1280x720,1920x1080}.png`. Check: the card fits 800×600 with its three rows, the bar and Cancel above the hint bar; markers and the bar crisp at 1.5; no `ACCENT` but the focus ring; `DANGER` nowhere on the connecting card; the long title and the path in the body wrap inside the dialog; Try again green and ringed. Fix any defect in its task's file with a test first.

- [ ] **Step 3: Legacy left behind** — `rg -n "server_connect_loading|CancelButtonMarker|LoadingTextMarker|SANS_FONT_PATH|FiraSans|Connecting to server|\[Cancel\]|YELLOW_500" client/src data` finds nothing; `rg -n "Interaction\b" client/src --type rust` finds only `pause.rs` and `audio/mod.rs` (Lot 6); `click_sounds`' doc names the pause menu as its last user. Clippy without allowance proves every item left has a user.

- [ ] **Step 4: Positions from the window** — `rg -n "cursor_position|world_to_viewport" client/src`: every result that ends in a `Val::Px` divides by `UiScale`; fix any that does not, with a test.

- [ ] **Step 5: Docs** — ARCHITECTURE "Network": a session's origin and end (`SessionOrigin`, `SessionEnd`, the first end wins), its steps, the 27 chunks before the game (`BROADCAST_CHUNK_RADIUS` is part of the protocol), how a failure is read without a protocol change (Decision 5's table, netcode's 5 s and `CONNECT_TIMEOUT`). ARCHITECTURE "UI widgets": `progress.rs`, `spawn_dialog_with_hints`. README "Multiplayer": what each error card means and what to do (no answer → port and firewall, both the game port and port + 1; another version; full; name in use → `--player-name`).

- [ ] **Step 6: Full suite** — `timeout 900 cargo test --workspace --no-fail-fast` green; fmt and clippy clean; `git status` shows no `.actual.png` / `.diff.png`.

- [ ] **Step 7: Commit** — `git commit -m "test(client): review snapshots of the connecting and error cards, and docs of a session's end"`
