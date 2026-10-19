# UI Redesign Lot 3 (Multiplayer) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the legacy server list with the Multiplayer screen of the spec: server cards with a live status (online with players and latency, offline, incompatible version, unknown host, pinging), a detail pane with Join, Edit and Remove, Add server (Ctrl+N) and Refresh (F5), Add/Edit dialogs that check the name and the address as you type (host names, default port 8000, duplicates), a confirmed Remove, an empty state and key hints, usable with the keyboard alone or the mouse alone. The status comes from a small status request the server answers.

**Architecture:** The status exchange is a fixed 32-byte UDP request and reply defined in `shared/src/status.rs`, answered by a thread of the dedicated server on its own port (game port + 1), because renet's `NetcodeServerTransport` owns the game socket and drops every packet that is not netcode. The client never blocks its frame: name resolution and status requests run on short-lived threads (`InFlight<T>`) whose results an `Update` system collects. What touches the disk or decides a value (parsing and resolving addresses, the server list in `servers.ron`, names, duplicates) lives in plain, unit-tested functions (`client/src/network/address.rs`, `client/src/ui/menus/servers.rs`). The form dialog widget learns to tell its caller when its fields changed (`FormChanged`), which Singleplayer's world dialogs and the new server dialogs both use. The screen in `ui/menus/multi/` only assembles widgets and rebuilds its cards from `servers.ron` after every change.

**Tech Stack:** Rust nightly, Bevy `=0.19.1` (`bevy::ui_widgets::{Button, Activate, ScrollArea}`, `bevy::ui::{Selected, InteractionDisabled}`, `bevy::input_focus`, `bevy::text::{EditableText, TextEdit}`, `SelectAllOnFocus`), `bevy_renet` 5 / `renet_netcode` 2 (unchanged), `std::net::UdpSocket`, `accesskit` 0.24, RON 0.6, lavapipe for goldens and e2e.

**Spec:** `docs/superpowers/specs/2026-10-18-ui-redesign-design.md` — only "Lot 3 — Multiplayer", the "Multiplayer" paragraph of "Screens" and decision 10 (server status); "Visual system", "Architecture", "Testing" and the anti-goals bind every task. Defects to fix: `docs/superpowers/specs/2026-10-18-ui-e2e-tests-design.md`, "Non-goals" (the server-list lines). API reference: `docs/superpowers/specs/2026-10-18-bevy-ui-0.19-notes.md`. Conventions and products: the Lot 0, 5, 1 and 2 plans next to this file; Lot 2 is the model (cards, pane, form dialogs, confirmed delete, hints, golden rules).

## Global Constraints

- English for code, comments, docs and commit messages; Conventional Commits (`feat(client): …`, `feat(server): …`, `feat(shared): …`, `fix(…)`, `test(…)`, `refactor(…)`, `docs: …`).
- Match the surrounding style: doc comments in full sentences, tests named as sentences, comment density like the neighbouring code.
- Every test command runs under `timeout` (e.g. `timeout 300 cargo test -q -p client e2e::multi`). The full suite `timeout 900 cargo test --workspace --no-fail-fast` is green at the end of every task.
- `cargo fmt --all -- --check` and `timeout 900 cargo clippy --workspace --all-targets -- -D warnings` are clean at every commit.
- The dev profile builds the client with Cranelift, which runs no destructor while a panic unwinds: do not rely on `Drop` in a failing path.
- Every e2e test is `#[test]` + `#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]`. Multiplayer tests go in `e2e/multi.rs`, driven only through `click`/`hover`/`press`/`type_text`/`scroll`. Every dedicated server is a thread left running: this lot has **one** server journey (Task 6, which replaces `connecting_to_a_server_enters_the_game`) and chains its checks.
- **No fixed network port in tests**: servers bind with `server::acquire_local_socket_pair`; nothing listens on 8000. A test may *send* to `127.0.0.1:8000` (the default port) only to check the address handed over, and cancels at once.
- Never a bare `git stash`; set work aside with a WIP commit.
- `bevy::prelude::Button` is the legacy widget: `use bevy::ui_widgets::Button as UiButton;`. The new screen uses no `Interaction`. The widget component or the explicit `AccessibilityNode(Role::…)` goes **before** `AccessibleLabel` in a bundle.
- A widget item no screen uses yet allows `dead_code` on the item with a comment naming its first user; the allowance goes in the task that uses it. Never crate-wide.
- **Escape.** A dialog consumes Escape through `FocusedInput<KeyboardInput>` and closes in `PostUpdate`; a field's `Cancel` closes its form dialog. `escape_button`'s Multi arm stays the screen's Escape; it and the `Update` handlers of Ctrl+N and F5 skip while `dialog_open` or `text_field_focused` holds.
- **Fonts.** The RustCraft font has no `← ↑ → ↓`, `…`, `×`, `−`, backtick or accented letters: write `...`; arrows only in key caps (Fira Mono).
- **The UI thread never waits on the network.** No `to_socket_addrs`, `recv` or `sleep` in a system or an observer: they run in `InFlight::spawn` threads; systems only `take()` results.
- **Protocol and compatibility.**
  - `PROTOCOL_ID` stays `5`: no game message changes. The status exchange lives outside netcode, on UDP port `game port + 1` (`status_port`), and is versioned by its own `STATUS_MAGIC = *b"RCST"` and `STATUS_FORMAT = 1`. Format 1's layout never changes; a new layout bumps `STATUS_FORMAT` and servers keep answering the older ones.
  - A request and a reply are both exactly `STATUS_PACKET_LEN = 32` bytes (a reply is never larger than its request: the port cannot amplify a spoofed request). The server answers only a well-formed format-1 request of exactly 32 bytes; anything else is dropped silently, never panics, never stops the thread.
  - The client accepts only a reply from the address it asked (connected socket), of the right magic, format and kind, carrying one of the nonces it sent. A reply whose `protocol != PROTOCOL_ID` makes the server "Incompatible"; the client never joins it.
  - Old server (no status port) seen by a new client: "Offline" (documented in the README). Old client and new server: unchanged, the old client never asks and netcode still refuses a different `PROTOCOL_ID`. The integrated (solo) server opens no status port.
- **Server list file.** `servers.ron` keeps its format, a list of `(name: "…", ip: "…")` (`ServerItem`); `ip` holds the address as typed, trimmed (`play.example.com`, `10.0.0.2:8000`). It is written at once, atomically (`servers.ron.tmp` then rename), after every add, edit and remove, never "on leaving". A missing file lists `localhost` (`127.0.0.1:8000`) and writes nothing; an unreadable file is never written. After every change the cards are rebuilt from `read_servers`.
- **Ownership.** Lot 4 owns `server_connect_loading.rs` (its `[Cancel]` and `Interaction` stay). Lot 6 owns `pause.rs` (its own `Button` + `Interaction` styling, nothing from `ui/button.rs` or `style.rs`), `hud/chat.rs` (which keeps `style::text_font`) and `audio`'s `click_sounds` (clicks of those two legacy screens). They stay.
- **Goldens.** `menu_multi` changes in Task 5 (interim: the fields give way to an "Add server" button) and Task 6; `menu_multi_empty` is new in Task 6. Each is regenerated with `UPDATE_GOLDEN=1` only after reading the `.actual.png`. Goldens hold nothing live: the statuses come from `fixed_status` (no network, latency fixed at 42 ms and 180 ms), the cursor sits in a corner (`hover_at`), no field has the focus. `menu_solo`, `menu_solo_empty` must stay byte-identical in Task 4. Any other golden that changes is a bug.

## Decisions (where the spec is silent)

1. **Status transport: a separate UDP port, game port + 1.** Decided because `NetcodeServerTransport::update` reads the socket itself and hands every datagram to `process_packet`, which drops what is not netcode; a cloned socket would race it for packets. Game port 65535 has no status port (the card says "Offline"). The server answers from a dedicated thread (blocking `recv_from` with a 250 ms timeout, stopping when the server app is dropped), so the measured latency is the network's, not the 20 Hz tick's.
2. **What the status says:** players online, max players (`MAX_CLIENTS` = 64) and the protocol; latency is measured by the client from the send of the nonce that was answered. No name or MOTD (the spec lists none). Up to 4 requests 500 ms apart (`RESEND_EVERY`), then "Offline" after `STATUS_TIMEOUT` = 2 s; a refused port (ICMP) is "Offline" at once.
3. **When it pings:** on opening the screen, after an Add or an Edit that changed the address (that server only), and on Refresh / F5 (every server). No periodic polling. A Refresh drops the probes in flight: a stale result can never land.
4. **Addresses.** `host` or `host:port`, trimmed, port 1–65535, default 8000 (`shared::DEFAULT_PORT`, also the server binary's default). A host is an IPv4 literal or a DNS name (labels of `A-Z a-z 0-9 -`, 1–63 chars, not starting or ending with `-`, at most 253 chars). IPv6 is refused with a message, because the client's game socket is IPv4-only (`init_server_connection` binds `0.0.0.0`) and changing networking is a non-goal; resolution keeps the first IPv4 address. Max 64 characters typed.
5. **Duplicates** are both: the same server (same host ignoring case, same port after the default) and the same name ignoring case. A card is named by its server's name for players and screen readers: two "Home" cards could not be told apart.
6. **Names.** Trimmed, 1–32 characters, printable ASCII without the backtick (the font). The name field opens filled with a free name ("My server", "My server 2", …) entirely selected, as Lot 2's New world: Ctrl+N, type a name, Tab, type an address, Enter. The address field opens empty: an untouched empty address shows no message but keeps Add disabled; once typed into and emptied it says "Enter an address".
7. **Card and pane.** Card: a 10 px status dot (`OK` online, `MUTED` pinging/offline, `DANGER` incompatible/unknown host/bad address), the name over the address (`MUTED`), and on the right either `3 / 20` with four latency bars (≤ 60 ms 4, ≤ 120 ms 3, ≤ 250 ms 2, else 1; filled `OK`, empty `BEVEL_HI`) or the status word (`Pinging...`, `Offline`, `Incompatible`, `Unknown host`, `Bad address`). Pane (280 px, as Singleplayer): name (`FONT_L`), address, status headline and one line (`Online` · `3 / 20 players` · `42 ms`; `Offline` · `No answer. Check the address or refresh.`; `Incompatible version` · `This server runs another version of Rustcraft.`; `Unknown host` · `Check the address.`; `Bad address` · `Edit the server to fix it.`; `Pinging...`), then Join (Primary), under it Edit | Remove side by side, both Default (as Rename | Delete in Singleplayer: `DANGER` only in the Remove confirmation), then the error line. Bars on the card, exact ms in the pane: the list compares at a glance, the pane gives the number.
8. **Join.** Enter on a card or Join in the pane resolves the address on a thread, then hands over to the existing connect path (`TargetServer.address`, `TargetServerState::Initial`, `GameState::PreGameLoading`, `MenuState::Disabled`) exactly as the legacy Connect did; the connecting screen is Lot 4's. While a host name resolves, the error line says `Looking up play.example.com...` (`MUTED`); a failure says `Cannot find play.example.com` (`DANGER`). Join is disabled for "Incompatible" and "Bad address" (it would only time out); Enter on such a card does nothing. Leaving the screen cancels a pending join (it lives on the screen root). No double-click (Lot 2 decision 2). No "Direct connect": the spec does not ask for it, and Add then Enter is two keystrokes longer.
9. **Title and frame.** The title is "Servers" (the main menu button is "Multiplayer", which tests use to find the main menu); "Refresh" and "Add server" on the right of the title bar; "Back" bottom-left. Empty state: "No servers yet" and "Add one with Add server (Ctrl+N)."; pane hidden; Add server focused. Unreadable `servers.ron`: "Cannot read servers.ron" and "Fix or delete the file, then come back."; Add server and Refresh disabled, Ctrl+N does nothing, Back focused.
10. **Dialogs.** Add: title "Add server", fields "Name" and "Address" (placeholder `host:port`), actions Cancel, Add. Edit: title `Edit "Friends"`, both fields filled, actions Cancel, Save (disabled while nothing changed, without a message). Remove: title `Remove "Friends"?`, body `192.168.1.20:8000 will leave your server list.`, actions Cancel (focused), Remove (Danger). A write failure shows under the address field (dialogs) or on the pane's error line (Remove), in a few words; never only in the log.
11. **Hints per state** (five each, the most that fit 800×600 on one line). A card focused: `↑↓ Select`, `Enter Join`, `Del Remove`, `F5 Refresh`, `Esc Back`. Anything else focused: `Tab Move`, `Enter Select`, `Ctrl+N Add server`, `F5 Refresh`, `Esc Back`. The empty state: the same without `F5 Refresh` (four hints), and the Refresh button is disabled, as in the unreadable state: a key or a button that does nothing is not offered; both come back once a server is added. An unreadable `servers.ron`, where Ctrl+N and Refresh do nothing: `Tab Move`, `Enter Select`, `Esc Back`. A form dialog: `Tab Move`, `Enter Add` (or `Save`), `Esc Cancel`.
12. **Order.** The file order; a new server is appended, an edited one keeps its place (as the legacy list did).
13. **Legacy left behind.** `ui/button.rs` goes entirely: `MenuState` moves to `ui/menus/mod.rs`. `style.rs` keeps only `text_font` (the chat's, Lot 6). `assets.rs` loses the button images' paths and loaders and `load_background_image`; `data/graphics/{button_background,button_background_large,dark_button_background,dark_button_background_large,play,trash}.png` are deleted (`docs/images/button_background_base.png` is a doc image and stays).

## Review Focus

1. A stale status never replaces a newer one: a probe started before a Refresh, or before an Edit changed the address, finishing last. (Task 6: `a_refresh_drops_the_probes_in_flight`)
2. A hostile packet on the status port — empty, 1 byte, 31 or 33 bytes, wrong magic, format 2, a reply sent as a request — gets no answer and the next good request still does; a reply with an unknown nonce is ignored by the client. (Task 1: `malformed_packets_decode_to_nothing`, `the_status_port_counts_the_players_and_ignores_garbage`; Task 3: `a_reply_with_another_nonce_is_ignored`)
3. `" LOCALHOST "` next to `localhost:8000` is refused as the same server, and joining `localhost` connects to `127.0.0.1:8000`. (Task 2: `two_addresses_are_the_same_server_whatever_the_case_and_the_default_port`; Task 6: `join_hands_a_host_name_over_with_the_default_port`)
4. An unreadable `servers.ron` is never overwritten, whatever the player tries. (Task 2: `an_unreadable_list_is_never_written`; Task 6: `an_unreadable_server_list_is_said_and_never_overwritten`)
5. A join still resolving when the player leaves the screen hands nothing over. (Task 6: `a_join_resolved_after_the_screen_closed_hands_nothing_over`)
6. Ctrl+N, F5 and Del typed while a dialog field has the focus act on the field only. (Task 6: `list_keys_do_nothing_while_a_dialog_is_open`)

## Defects of the e2e spec fixed here

Enter and Tab in the server fields, no keyboard activation on the list (Tasks 5, 6); removing a server asks no confirmation (Task 6); invalid address, host names refused and unreadable `servers.ron` only logged (Tasks 2, 5, 6); empty server names and duplicate servers accepted (Tasks 2, 5); the fields not cleared after Add (Task 5); localhost comes back after being deleted (Tasks 2, 6); no hover feedback on the row icons (Task 6); the wheel scrolls every list at once (Task 6). The connection screen's defects are Lot 4's.

## Files

- `shared/src/status.rs` (new), `shared/src/{lib,constants}.rs`; `server/src/network/status.rs` (new), `server/src/network/mod.rs`, `server/src/{init,lib,main}.rs`, `server/tests/session.rs` — the status exchange (Task 1).
- `client/src/network/{address,status}.rs` (new), `client/src/network/mod.rs`, `client/src/ui/menus/servers.rs` (new) — addresses, the list on disk, the probe (Tasks 2, 3).
- `client/src/ui/widgets/dialog.rs`, `client/src/ui/menus/solo/dialogs.rs` — form checks (Task 4).
- `client/src/ui/menus/multi/{mod,dialogs,status}.rs` (`multi.rs` becomes `multi/mod.rs` in Task 5), `client/src/ui/menus/mod.rs`, `client/src/ui/menus/list_screen.rs` (deleted in Task 6), `client/src/ui/{button,style,assets,mod}.rs`, `data/graphics/*.png` — the screen (Tasks 5–7).
- `client/src/e2e/{multi,seed,helpers,game,main_menu,capture,review}.rs` and every e2e file importing `crate::ui::button::MenuState`, `client/tests/golden/menu_multi{,_empty}.png`, `README.md`, `docs/ARCHITECTURE.md`.

---

### Task 1: The status exchange — codec in shared, a responder in the dedicated server

**Files:**
- Create: `shared/src/status.rs`, `server/src/network/status.rs`
- Modify: `shared/src/lib.rs` (`pub mod status;`), `shared/src/constants.rs` (`DEFAULT_PORT`), `server/src/network/mod.rs`, `server/src/init.rs`, `server/src/lib.rs`, `server/src/main.rs`, `server/tests/session.rs`

**Interfaces:**
- Produces, in `shared`:
  ```rust
  pub const DEFAULT_PORT: u16 = 8000;                       // constants.rs
  // status.rs
  pub const STATUS_MAGIC: [u8; 4] = *b"RCST";
  pub const STATUS_FORMAT: u8 = 1;
  pub const STATUS_PACKET_LEN: usize = 32;
  #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct StatusRequest { pub nonce: u64 }
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub struct StatusReply { pub nonce: u64, pub protocol: u64, pub players: u32, pub max_players: u32 }
  impl StatusRequest { pub fn encode(&self) -> [u8; STATUS_PACKET_LEN]; pub fn decode(packet: &[u8]) -> Option<Self>; }
  impl StatusReply   { pub fn encode(&self) -> [u8; STATUS_PACKET_LEN]; pub fn decode(packet: &[u8]) -> Option<Self>; }
  /// The UDP port of the status of a server listening on `game_port`.
  pub fn status_port(game_port: u16) -> Option<u16>;       // game_port.checked_add(1)
  ```
  Layout, little-endian, zero padded to 32: magic (4), format (1), kind (1: `0` request, `1` reply), nonce (8); a reply then has protocol (8), players (4), max_players (4). `decode` requires exactly 32 bytes, the magic, format 1 and its own kind.
- Produces, in `server`:
  ```rust
  // init.rs (re-exported from lib.rs)
  /// Like `init`, and answers status requests on `status` while the server runs.
  pub fn init_with_status(socket: UdpSocket, status: UdpSocket, config: GameServerConfig,
                          game_folder_paths: GameFolderPaths) -> Result<(), String>;
  /// A game socket on an ephemeral port of `ip` and its status socket on the next port; retries
  /// up to 16 times when that port is taken.
  pub fn acquire_local_socket_pair(ip: IpAddr) -> std::io::Result<(UdpSocket, UdpSocket)>;
  // network/status.rs
  #[derive(Resource)] pub(crate) struct StatusCounter(pub Arc<AtomicU32>);
  /// The reply to `packet`, or nothing when it is not a well-formed status request.
  pub(crate) fn answer(packet: &[u8], players: u32, max_players: u32) -> Option<[u8; STATUS_PACKET_LEN]>;
  /// Answers on `socket` until `players` can no longer be upgraded (the server app is gone).
  pub(crate) fn spawn_status_responder(socket: UdpSocket, players: Weak<AtomicU32>, max_players: u32)
      -> std::io::Result<JoinHandle<()>>;
  fn publish_player_count(lobby: Res<ServerLobby>, counter: Res<StatusCounter>);   // Update
  ```
  `init` and `init_with_status` share one private `run(socket, status: Option<UdpSocket>, …)`; `init`'s callers (solo server, tests) are untouched. `main.rs`: `--port` defaults to `shared::DEFAULT_PORT`; binds the status socket on `status_port(port)` of the same IP; on failure logs `Could not listen on UDP port {p} for the status: {err}; the server list will show this server offline` and runs with `init`.

- [ ] **Step 1: Write the failing tests**

Unit, `shared/src/status.rs`:
- `a_request_and_a_reply_read_back_what_was_written` — `StatusRequest { nonce: 7 }` and `StatusReply { nonce: 7, protocol: 5, players: 3, max_players: 64 }` round-trip; both encodings are 32 bytes.
- `malformed_packets_decode_to_nothing` — `[]`, `[0]`, 31 and 33 bytes of a good request, a wrong magic, format `2`, a reply given to `StatusRequest::decode`, a request given to `StatusReply::decode` → `None`.
- `the_status_port_is_next_to_the_game_port` — `8000 → Some(8001)`, `65535 → None`.

Unit, `server/src/network/status.rs`: `only_a_good_request_is_answered` — `answer(&StatusRequest { nonce: 9 }.encode(), 2, 64)` decodes to `{ nonce: 9, protocol: PROTOCOL_ID, players: 2, max_players: 64 }`; every malformed packet above → `None`.

Unit, `server/src/init.rs`: `the_socket_pair_is_two_neighbouring_ports` — `status.local_addr().port() == game.local_addr().port() + 1`.

Integration, `server/tests/session.rs` (`TestServer::start_dedicated_with_status(world)` uses `acquire_local_socket_pair` and `init_with_status`, keeps `status_addr`; `fn ask_status(addr, nonce) -> Option<StatusReply>` on a fresh socket, 500 ms read timeout):
- `the_status_port_counts_the_players_and_ignores_garbage` — send `b"hello"`, 32 zero bytes and a `StatusReply` encoding → no datagram within 300 ms; `ask_status(42)` → `nonce == 42`, `protocol == PROTOCOL_ID`, `players == 0`, `max_players == 64`; a `TestClient` authenticates → within `WAIT_TIMEOUT` a reply says `players == 1`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p shared status` and `timeout 600 cargo test -q -p server -- status socket_pair` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces. `init_with_status` inserts `StatusCounter`, adds `publish_player_count`, and spawns the responder with `Arc::downgrade`; `MAX_CLIENTS` is the max.

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(server): answer status requests on the port next to the game port"`

---

### Task 2: Addresses and the server list on disk

**Files:**
- Create: `client/src/network/address.rs`, `client/src/ui/menus/servers.rs`
- Modify: `client/src/network/mod.rs` (`pub mod address;`), `client/src/ui/menus/mod.rs` (`pub mod servers;`), `client/src/ui/menus/multi.rs` (the legacy screen reads and writes through `servers.rs`; `ServerItem`, `read_servers`, `write_servers` and the list tests move), `client/src/e2e/seed.rs` (imports `servers::ServerItem`)

**Interfaces:**
- Produces, in `network/address.rs`:
  ```rust
  #[derive(Clone, Debug, PartialEq, Eq)] pub struct ServerAddress { pub host: String, pub port: u16 }
  impl Display for ServerAddress { … }                     // "host:port"
  impl ServerAddress {
      /// The same for two addresses of the same server: the host in lower case and the port.
      pub fn key(&self) -> String;
      pub fn is_ip(&self) -> bool;
  }
  pub const MAX_ADDRESS_LENGTH: usize = 64;
  pub const EMPTY_ADDRESS: &str = "Enter an address";
  pub const INVALID_ADDRESS: &str = "Use host:port, like 10.0.0.2:8000";
  pub const INVALID_PORT: &str = "The port must be 1 to 65535";
  pub const IPV6_ADDRESS: &str = "IPv6 is not supported yet";
  pub fn parse_server_address(typed: &str) -> Result<ServerAddress, &'static str>;   // Decision 4
  /// The first IPv4 address of `address`. Blocks on DNS: call it from a thread.
  pub fn resolve(address: &ServerAddress) -> io::Result<SocketAddr>;
  ```
- Produces, in `ui/menus/servers.rs` (plain functions; `path` is `game_folder/servers.ron`):
  ```rust
  #[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)] pub struct ServerItem { pub name: String, pub ip: String }   // moved, unchanged
  pub fn localhost() -> ServerItem;                        // ("localhost", "127.0.0.1:8000")
  /// The saved servers in file order. Missing: `[localhost()]`, nothing written. Empty: `[]`.
  /// Unreadable: `Err` of kind `InvalidData`.
  pub fn read_servers(path: &Path) -> io::Result<Vec<ServerItem>>;
  pub fn add_server(path: &Path, server: ServerItem) -> io::Result<()>;              // appended
  pub fn edit_server(path: &Path, name: &str, server: ServerItem) -> io::Result<()>; // in place; NotFound
  pub fn remove_server(path: &Path, name: &str) -> io::Result<()>;                   // NotFound
  pub const MAX_SERVER_NAME_LENGTH: usize = 32;
  pub const EMPTY_SERVER_NAME: &str = "Enter a name";
  pub const INVALID_SERVER_NAME: &str = "Use letters, digits and punctuation";
  pub const SERVER_NAME_EXISTS: &str = "Another server has this name";
  pub const SERVER_EXISTS: &str = "Another server has this address";
  /// The trimmed name, or the message to show. `others` are the names of the other servers.
  pub fn check_server_name(typed: &str, others: &[String]) -> Result<String, &'static str>;
  /// The parsed address, or the message to show. `others` are the other servers.
  pub fn check_server_address(typed: &str, others: &[ServerItem]) -> Result<ServerAddress, &'static str>;
  pub fn free_server_name(others: &[String]) -> String;    // "My server", "My server 2", …
  ```
  `add`/`edit`/`remove` read the file first (so an unreadable one fails with `InvalidData` and is never written), match names with `eq_ignore_ascii_case`, and write through one private `write_servers(path, &[ServerItem])` — the legacy pretty config, to `servers.ron.tmp`, then `fs::rename`. An `ip` that does not parse is skipped by `check_server_address`'s duplicate check.
- The legacy screen for Tasks 2–4: `load_server_list` uses `read_servers` (no more localhost added to an existing file); `save_server_list` writes through `servers::write_servers` made `pub(super)` until Task 5. Its own `parse_server_address` stays private (it joins IP literals only) until Task 6.

- [ ] **Step 1: Write the failing tests** (unit, with `tempfile::tempdir()`)

`address.rs`:
- `an_address_is_a_host_and_an_optional_port` — `"10.0.0.2:8000" → ("10.0.0.2", 8000)`; `" play.Example.com " → ("play.Example.com", DEFAULT_PORT)`, displayed `"play.Example.com:8000"`; `"localhost:25565" → 25565`.
- `bad_addresses_say_why` — `""`, `"   "` → `EMPTY_ADDRESS`; `"a b"`, `"host:"`, `"host:x"`, `":8000"`, `"-bad.com"`, `"a..b"`, a 64-char label, `"a_b.com"` → `INVALID_ADDRESS`; `"host:0"`, `"host:65536"` → `INVALID_PORT`; `"::1"`, `"[::1]:8000"` → `IPV6_ADDRESS`.
- `two_addresses_are_the_same_server_whatever_the_case_and_the_default_port` — `key(" LOCALHOST ") == key("localhost:8000")`; `key("a:8001") != key("a")`.
- `a_host_name_resolves_to_an_ipv4_address` — `resolve(localhost) == 127.0.0.1:8000`; `resolve("10.0.0.2:9") == 10.0.0.2:9`; `resolve("nowhere.invalid")` is `Err`.

`servers.rs`:
- `a_missing_list_offers_localhost_and_writes_nothing` — `[localhost()]`, the file still missing.
- `an_empty_list_is_empty_and_an_unreadable_one_is_an_error` — `""`, `" \n"`, `"[]"` → `[]`; `"not ron"` → `Err` of kind `InvalidData`.
- `an_old_server_list_still_loads` — the literal the legacy writer produced: `[\n    /*[0]*/ (\n        name: "localhost",\n        ip: "127.0.0.1:8000",\n    ),\n    /*[1]*/ (\n        name: "Friends",\n        ip: "192.168.1.20:8000",\n    ),\n]` → two items.
- `adding_editing_and_removing_write_the_list_at_once_in_order` — file `[zulu, alpha]`: add `bravo` → `[zulu, alpha, bravo]`; `edit_server(.., "ALPHA", (Alpha2, 10.0.0.9))` → `[zulu, Alpha2, bravo]`; remove `zulu` → `[Alpha2, bravo]`, each read back with `read_servers`; no `servers.ron.tmp` left; `remove_server(.., "nobody")` → `NotFound`.
- `a_removed_localhost_stays_removed` — missing file, `remove_server(.., "localhost")` → `read_servers == []`.
- `an_unreadable_list_is_never_written` — `"not ron"`: `add`, `edit`, `remove` → `Err(InvalidData)`, the file still `"not ron"`.
- `server_names_are_checked_and_trimmed` — others `["Home"]`: `" Base " → Ok("Base")`; `""`, `"  "` → `EMPTY_SERVER_NAME`; `"home "` → `SERVER_NAME_EXISTS`; 33 × `x`, `"a`b"`, `"é"` → `INVALID_SERVER_NAME`; 32 × `x` → `Ok`.
- `a_server_address_is_checked_against_the_others` — others `[("Friends", "192.168.1.20:8000"), ("Old", "garbage")]`: `"192.168.1.20"` → `SERVER_EXISTS`; `"192.168.1.20:8001"` → `Ok`; `"bad host"` → `INVALID_ADDRESS`.
- `a_free_server_name_is_offered` — `[] → "My server"`; `["my server"] → "My server 2"`; `["My server", "My server 2"] → "My server 3"`.
- The legacy tests of `multi.rs` that still hold move here as `servers.rs` tests; `localhost_is_added_first_when_missing_and_only_once` is replaced by the two localhost tests above (the defect fix).

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- network::address menus::servers menus::multi` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces; `dead_code` on `check_server_name`, `check_server_address`, `free_server_name`, `add_server` ("the Add server dialog, Task 5"), `edit_server` ("the Edit dialog, Task 7"), `remove_server`, `resolve`, `ServerAddress::is_ip` ("the Multiplayer screen, Task 6") as needed.

- [ ] **Step 4: Run the tests and the suite** → PASS; `e2e::multi` green; full suite green; no golden changed.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): server addresses with host names, and a server list written at once"`

---

### Task 3: The status probe — asked on a thread, never on the frame

**Files:**
- Create: `client/src/network/status.rs`
- Modify: `client/src/network/mod.rs` (`pub mod status;`)

**Interfaces:**
- Consumes: `shared::status::*`, `parse_server_address`, `resolve`.
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq)]
  pub enum ServerStatus { Pinging, Online { players: u32, max_players: u32, latency: Duration },
                          Incompatible, Offline, UnknownHost, BadAddress }
  pub const STATUS_TIMEOUT: Duration = Duration::from_secs(2);
  pub const RESEND_EVERY: Duration = Duration::from_millis(500);
  /// Asks the server at `typed` (a `ServerItem::ip`) for its status. Blocks up to STATUS_TIMEOUT
  /// plus the name resolution: call it from a thread.
  pub fn query_status(typed: &str) -> ServerStatus;
  /// Asks the status port `status` itself: a connected socket, a new random nonce every
  /// RESEND_EVERY, the latency from the send of the nonce answered.
  pub fn query_status_at(status: SocketAddr, timeout: Duration) -> ServerStatus;
  pub fn latency_bars(latency: Duration) -> u8;            // Decision 7
  /// How the screen asks: `query_status`, replaced by tests with fixed answers.
  #[derive(Resource, Clone, Copy)] pub struct StatusProbe(pub fn(&str) -> ServerStatus);
  impl Default for StatusProbe { … }                       // StatusProbe(query_status)
  /// The result of `f` run on its own thread, taken once. Dropping it abandons the result.
  pub struct InFlight<T>(Arc<Mutex<Option<T>>>);
  impl<T: Send + 'static> InFlight<T> { pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Self;
                                        pub fn take(&self) -> Option<T>; }
  ```
  `query_status`: `parse_server_address` fails → `BadAddress`; `resolve` fails → `UnknownHost`; no `status_port` → `Offline`; else `query_status_at`. A reply with `protocol != PROTOCOL_ID` → `Incompatible`. `ConnectionRefused` from `recv` → `Offline` at once. Decided: one short-lived `std::thread` per probe because DNS resolution blocks and Bevy's task pools would be starved by blocking calls.

- [ ] **Step 1: Write the failing tests** (unit, `status.rs`; `fn fake_server(reply: impl Fn(StatusRequest, usize) -> Option<StatusReply> + Send + 'static) -> SocketAddr` binds `127.0.0.1:0` and answers on a thread, given each request and its index)

- `a_server_that_answers_is_online_with_its_players_and_latency` — answers `{protocol: PROTOCOL_ID, players: 3, max_players: 20}` → `Online { players: 3, max_players: 20, latency }` with `latency < 1 s`.
- `a_server_of_another_protocol_is_incompatible` — `protocol: PROTOCOL_ID + 1` → `Incompatible`.
- `a_lost_request_is_sent_again` — ignores request 0, answers request 1 → `Online`, `latency < RESEND_EVERY`.
- `a_reply_with_another_nonce_is_ignored` — answers every request with `nonce + 1` → `Offline`, after at least `STATUS_TIMEOUT`.
- `nothing_listening_is_offline_soon` — a bound-then-dropped port → `Offline` in less than `STATUS_TIMEOUT + 500 ms`.
- `an_unknown_host_or_a_bad_address_is_said_so` — `query_status("nowhere.invalid") == UnknownHost`; `query_status("bad host") == BadAddress`.
- `latency_reads_as_bars` — 10 ms → 4, 60 → 4, 61 → 3, 120 → 3, 200 → 2, 251 → 1.
- `in_flight_gives_its_result_once` — `InFlight::spawn(|| 5)`: polled until `Some(5)`, then `None`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client network::status` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces; `dead_code` on `StatusProbe`, `InFlight`, `latency_bars` with "the Multiplayer screen (Task 6)".

- [ ] **Step 4: Run the tests and the suite** → PASS; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): ask a server for its status without blocking the frame"`

---

### Task 4: Form dialogs say when their fields changed

Lifts out of `WorldForm` the rule "check again when the texts change, never while edits are pending" (`EditableText` is marked `Changed` every frame, so the values themselves tell), so the server dialogs reuse it.

**Files:**
- Modify: `client/src/ui/widgets/dialog.rs`, `client/src/ui/menus/solo/dialogs.rs`

**Interfaces:**
- Produces, in `dialog.rs`:
  ```rust
  /// Triggered on the root of a form dialog once its fields hold their first values, then each
  /// time a value changed, never while a field has pending edits. `values` and `touched` follow
  /// the order of the fields; a field is touched once its value differed from its first value.
  #[derive(EntityEvent)]
  pub struct FormChanged { pub entity: Entity, pub values: Vec<String>, pub touched: Vec<bool> }
  /// Shows `errors` under the fields of the form `root` (one per field, `None`: no message), and
  /// disables its primary action unless `submittable`. Changes only what differs.
  pub fn apply_form_check(commands: &mut Commands, root: Entity,
                          errors: Vec<Option<&'static str>>, submittable: bool);
  ```
  Private: `#[derive(Component)] struct FormFields { inputs: Vec<Entity>, primary: Option<Entity>, first: Option<Vec<String>>, checked: Option<Vec<String>>, touched: Vec<bool> }` on the root of every form dialog, and `fn notice_form_changes(...)` in `Update`. A message set by a failed submit stays until a value changes (as today).
- `solo/dialogs.rs`: `WorldForm` loses `checked`; `validate_world_forms` becomes the observer `check_world_form(On<FormChanged>, …)` on each world dialog's root, calling `apply_form_check`; the world dialogs ignore `touched` (their behaviour is unchanged).

- [ ] **Step 1: Write the failing tests** (unit, `dialog.rs`, a form built with `spawn_form_dialog` in a bare `App` with `notice_form_changes`; an observer records `FormChanged`)

- `a_form_is_checked_once_its_fields_hold_their_first_values` — fields `""` and one holding a queued `TextEdit::Insert("Fresh")`: no event on the first update (the bare app applies no edits); replace the second field's text with `EditableText::new("Fresh")` (nothing pending) → exactly one event with `values == ["", "Fresh"]`, `touched == [false, false]`; two more updates with nothing changed → no new event.
- `a_changed_value_is_reported_and_stays_touched` — set field 1 to `"Fresh2"` → event with `touched == [false, true]`; set it back to `"Fresh"` → event, `touched == [false, true]`.
- `apply_form_check_sets_the_errors_and_the_primary` — `apply_form_check(root, vec![Some("Bad"), None], false)` → `FieldError(Some("Bad"))` on input 0, `FieldError(None)` on input 1, `InteractionDisabled` on the primary; `(vec![None, None], true)` → none.
- Move: `solo/dialogs.rs`'s `a_form_opened_with_an_empty_name_is_checked_at_once` and `a_name_still_being_filled_is_not_checked_yet` now drive `check_world_form` through `FormChanged` and keep their assertions.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- ui::widgets::dialog menus::solo` → compile errors.

- [ ] **Step 3: Implement** as in Interfaces.

- [ ] **Step 4: Run the tests and the suite** — `e2e::solo` and `e2e::widgets` green unchanged (this is the refactor's proof), `menu_solo` and `menu_solo_empty` byte-identical; full suite green.

- [ ] **Step 5: Commit** — `git commit -m "refactor(client): form dialogs report their changed fields to their screen"`

---

### Task 5: The Add server dialog — checked as you type, saved at once

**Files:**
- Create: `client/src/ui/menus/multi/dialogs.rs`
- Move: `client/src/ui/menus/multi.rs` → `client/src/ui/menus/multi/mod.rs`
- Modify: `client/src/ui/menus/multi/mod.rs`, `client/src/ui/menus/mod.rs`, `client/src/ui/menus/list_screen.rs` (its text inputs go: `InputField`, `spawn_input`, `ListScreen::inputs`, `ListScreenParts::inputs`), `client/src/e2e/helpers.rs`, `client/src/e2e/multi.rs`, `client/tests/golden/menu_multi.png`

**Interfaces:**
- Consumes: `spawn_form_dialog`, `FormChanged`, `apply_form_check`, `DialogSubmit`, `close_dialog`, `set_text`, `spawn_text_field` (`reserve_error: true`), `CloseRequest`, Task 2's `servers.rs` and `address.rs`.
- Produces, in `multi/dialogs.rs`:
  ```rust
  pub const ADD_SERVER_TITLE: &str = "Add server";
  #[derive(Component)] pub struct ServerNameInput;
  #[derive(Component)] pub struct ServerAddressInput;      // replaces ServerIpInput
  /// The fields of an open server dialog and what they are checked against, on its root.
  #[derive(Component)] struct ServerForm { name: Entity, address: Entity,
                                           others: Vec<ServerItem>, editing: Option<ServerItem> }
  /// Triggered once `servers.ron` changed: the screen lists the servers again and focuses the
  /// card of `focus` (none, or gone: the first card, else Add server); `error` shows in the pane.
  #[derive(Event)] pub struct ServersChanged { pub focus: Option<String>, pub error: Option<&'static str> }
  pub fn open_add_server_dialog(commands: &mut Commands, assets: &AssetServer, focus: &InputFocus,
                                others: Vec<ServerItem>) -> Entity;
  pub fn open_add_server_on_ctrl_n(...);   // Update, in_state(Multi), not(dialog_open), not(text_field_focused)
  fn check_server_form(...);               // observer of FormChanged: check_server_name, check_server_address
  fn add_server_on_submit(...);            // observer of DialogSubmit, Without<CloseRequest>
  /// What a field says when servers.ron could not be written, in a few words.
  fn save_failure(error: &io::Error) -> &'static str;
  pub(super) fn plugin(app: &mut App);
  ```
  The dialog as in Decisions 6 and 10: Name (`max_characters: 32`, `SelectAllOnFocus`, filled with `free_server_name`), Address (`max_characters: MAX_ADDRESS_LENGTH`, placeholder `"host:port"`), hints `[Tab Move, Enter Add, Esc Cancel]`. The address error shows only when `touched[1]` or the value is not empty; Add is disabled while either check fails. Submit: checks again, `add_server(path, ServerItem { name, ip: typed.trim() })`; `Ok` → `close_dialog(root, false)` and `ServersChanged { focus: Some(name), error: None }`; `Err(e)` → `FieldError(save_failure(&e))` on the address (`"The disk is full"`, `"No permission to write here"`, `"Cannot read servers.ron"` for `InvalidData`, else `"Could not save the server list"`), dialog open.
- The legacy screen for this one task: `ListScreen` of Multi has no inputs; its "Add server" button opens the dialog with the names of the saved servers; a `ServersChanged` observer rebuilds the legacy rows from `read_servers`; a legacy Delete calls `remove_server` at once; `save_server_list`, `OnExit(MenuState::Multi)`, `MultiplayerButtonAction::Add`, `add_typed_server`, `add_server_on_submit` (legacy) and `new_server` go. Legacy Connect still joins IP literals only, until Task 6.
- Produces, in `e2e/helpers.rs`: `pub fn ctrl(h: &mut Harness, key: KeyCode)` (Control held around `press(key)`); `ctrl_n` calls it.

- [ ] **Step 1: Write the failing tests** (e2e, `e2e/multi.rs`; `fn add_dialog(h)` = `ctrl(h, KeyN)` then wait for `L::role(Role::Dialog, ADD_SERVER_TITLE)`; `fn saved_names(h) -> Vec<String>` reads the file through `read_servers`)

- `add_server_opens_a_dialog_with_a_free_name_ready_to_replace` — click `"Add server"` → the dialog, `focus_label() == Some("Name")`, `value_of::<ServerNameInput>() == "My server"`; `type_text("Home")` → `"Home"`.
- `tab_and_enter_add_a_server` (kept journey) — `add_dialog`, `type_text("Home")`, Tab, `type_text("10.0.0.2:8000")`, Enter, `frames(3)` → no dialog; `"Home"` listed once; **without leaving**, `saved_names == ["localhost", "Friends", "Home"]`.
- `a_new_server_is_listed_and_saved` (kept journey, by mouse; absorbs `clicking_a_field_focuses_it_alone`) — click Add server, click `L::role(Role::TextInput, "Address")` → it alone has the focus; click `"Name"` → it alone; type; click Address, type `"10.0.0.2:8000"`; click `L::role(Role::Button, "Add")` → listed and saved with `ip == "10.0.0.2:8000"`.
- `enter_in_the_name_field_adds_the_server` (kept journey) — fill both, Shift+Tab, Enter → added.
- `the_server_is_checked_as_you_type_and_nothing_moves` (replaces `an_invalid_address_adds_no_server`, journey kept) — `add_dialog`, `y = center_of(Add).y`; the empty address shows no message, Add has `InteractionDisabled`; Tab, `type_text("not an ip")` → `INVALID_ADDRESS`; Enter → still open, file unchanged; Backspace × 9 → `EMPTY_ADDRESS`; `"host:0"` → `INVALID_PORT`; clear, `"192.168.1.20"` → `SERVER_EXISTS`; clear, `"play.example.com"` → no message visible, Add enabled; Shift+Tab, clear the name → `EMPTY_SERVER_NAME`; `"friends"` → `SERVER_NAME_EXISTS`; Add's `y` never changed.
- `escape_in_a_field_closes_only_the_dialog` (replaces `escape_in_a_field_leaves_the_field_then_the_screen`) — `add_dialog`, `type_text("Ho")`, Escape → no dialog, `menu_state() == Multi` (and after 2 frames), file unchanged; Escape → `MenuState::Main`.
- `after_an_add_the_dialog_opens_fresh` (the "fields not cleared" defect) — add "One" at `10.0.0.5`, `add_dialog` → name `"My server"`, address `""`.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client e2e::multi` → compile errors, then no dialog.

- [ ] **Step 3: Implement** as in Interfaces; remove the Task 2 allowances this task uses.

- [ ] **Step 4: Run the tests; regenerate `menu_multi` on purpose (interim)** — read `menu_multi.actual.png`: the legacy rows, then "Add server" and "Back to menu" where the fields were; nothing else moved. `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_server_list_matches_its_golden`, rerun → PASS, no leftovers. Full suite green.

- [ ] **Step 5: Commit** (the report lists replaced tests and changed locators) — `git commit -m "feat(client): an Add server dialog that checks the address as you type and saves at once"`

---

### Task 6: The Multiplayer screen — cards with their status, detail pane, Join, Remove, keyboard

**Files:**
- Create: `client/src/ui/menus/multi/status.rs`
- Modify: `client/src/ui/menus/multi/{mod,dialogs}.rs` (rewrite of `mod.rs`), `client/src/ui/menus/mod.rs` (receives `MenuState`; `menu_action`, `button_system`, `mouse_scroll` and the legacy Multi systems go; `multi::multi_plugin(app)`), `client/src/ui/mod.rs`, `client/src/ui/style.rs` (only `text_font` stays), `client/src/ui/assets.rs`, every `use crate::ui::button::MenuState` (→ `crate::ui::menus::MenuState`), `client/src/e2e/{multi,seed,helpers,game,main_menu,capture,review}.rs`, `client/tests/golden/menu_multi.png`, `client/tests/golden/menu_multi_empty.png` (new)
- Delete: `client/src/ui/menus/list_screen.rs`, `client/src/ui/button.rs`, `data/graphics/{button_background,button_background_large,dark_button_background,dark_button_background_large,play,trash}.png`

**Interfaces:**
- Consumes: `spawn_scroll_list`, `spawn_list_card`, `cards_of`, `CardSelected`, `ListCard`, `button`, `hint_bar`, `HINT_BAR_HEIGHT`, `menu_background`, `spawn_dialog`, `DialogClosed`, `ArrowNav`, Tasks 2, 3 and 5.
- Produces, in `multi/mod.rs`:
  ```rust
  pub const SERVERS_TITLE: &str = "Servers";
  #[derive(Component)] pub struct ServerCard(pub ServerItem);
  #[derive(Component)] pub struct ServerList;              // the ScrollArea of the cards (Role::ListBox, "Servers")
  #[derive(Component)] pub struct ServerDetail;
  #[derive(Component, Clone, Copy)] pub enum ServerAction { Join, Remove }   // Edit in Task 7
  #[derive(Component)] pub struct ServerError;             // the pane's line: DANGER, or MUTED while looking up
  #[derive(Component)] pub struct AddServerButton;
  #[derive(Component)] pub struct RefreshButton;
  /// A join waiting for its address to resolve, on the screen's root: leaving the screen drops it.
  #[derive(Component)] struct PendingJoin { host: String, address: InFlight<io::Result<SocketAddr>> }
  #[derive(Component, PartialEq, Eq)] enum MultiHints { Cards, Elsewhere }
  pub const CARD_HINTS: [Hint; 5];                         // Decision 11
  pub const SCREEN_HINTS: [Hint; 5];
  pub fn multi_plugin(app: &mut App);                      // init_resource::<StatusProbe>()
  /// Hands `address` over to the connection, as the legacy Connect did.
  pub fn join_server(address: SocketAddr, target: &mut TargetServer,
                     game: &mut NextState<GameState>, menu: &mut NextState<MenuState>);
  fn show_servers(commands: &mut Commands, list: Entity, servers: &[ServerItem],
                  board: &StatusBoard, font: &Handle<Font>) -> Vec<Entity>;
  fn finish_join(...);                                     // Update: takes PendingJoin's result
  ```
  In `multi/status.rs`:
  ```rust
  /// The status of each server by `ServerAddress::key` (the `ip` itself when it does not parse),
  /// on the screen's root: kept across the rebuilds of the cards, dropped with the screen.
  #[derive(Component, Default)] pub struct StatusBoard(HashMap<String, Probe>);
  struct Probe { status: ServerStatus, in_flight: Option<InFlight<ServerStatus>> }
  impl StatusBoard {
      /// Starts asking `ip` with `probe`, replacing (and so abandoning) a probe in flight.
      pub fn ping(&mut self, ip: &str, probe: StatusProbe);
      pub fn status(&self, ip: &str) -> ServerStatus;      // Pinging when unknown
      /// The keys whose probe finished since the last call.
      pub fn collect(&mut self) -> Vec<String>;
  }
  #[derive(Component)] pub struct CardStatus;              // the right side of a card, rebuilt on change
  #[derive(Component)] pub struct LatencyBars(pub u8);
  /// The word and colour of a status on a card, and the two lines of the pane (Decision 7).
  pub fn card_status(status: &ServerStatus) -> (String, Color);
  pub fn pane_status(status: &ServerStatus) -> (String, Color, String);
  fn collect_statuses(...);                                // Update, in_state(Multi): card and pane follow
  fn refresh_on_f5(...);                                   // Update, not(dialog_open), not(text_field_focused)
  ```
  Layout as Singleplayer's (Decisions 7 and 9): root `menu_background`, `DespawnOnExit(MenuState::Multi)`, `TabGroup::new(0)`, `StatusBoard`; title bar (`"Servers"` `FONT_XL`, spacer, `button("Refresh")` + `RefreshButton`, `button("Add server")` + `AddServerButton`), body row (scroll list with `ServerList`, the pane with an `ArrowNav` column of Join (Primary), Remove (Danger), then `ServerError`), footer `button("Back")`, hint bar. Text wraps with `LineBreak::WordOrCharacter`, text columns `min_width: 0`. Behaviour: on setup, `ServersChanged { focus: None }` and `ping` every server; on `ServersChanged` → `show_servers(read_servers(path))` (`Err` → the unreadable state of Decision 9), focus as in Lot 2, ping what the board does not know; `CardSelected` fills the pane; Enter on a focused card (no Control) or Join → `PendingJoin` (refused for `Incompatible` / `BadAddress`; `Looking up …` when `!is_ip()`), `finish_join` → `join_server` or `ServerError(Cannot find {host})`; Del on a card or Remove → the Remove dialog (`open_remove_server_dialog(commands, assets, focus, server: &ServerItem, next: Option<String>) -> Entity` in `dialogs.rs`), whose `Some(1)` calls `remove_server` and triggers `ServersChanged { focus: next, .. }` or shows `save_failure`; Refresh / F5 → `ping` every card; Add server / Ctrl+N → `open_add_server_dialog`; Back → `MenuState::Main`; Escape stays in `escape_button`. A system keeps the hint bar on `CARD_HINTS` while a `ListCard` has the focus, else `SCREEN_HINTS`. Everything legacy goes (Decision 13); `rg -n "MenuButtonAction|button_system|ScrollingList|mouse_scroll|spawn_list_screen|spawn_row|fill_image_node|load_button_background|load_dark_button|load_background_image|wait_for_icons|play_rows|ServerIpInput|MultiplayerButtonAction" client/src` finds nothing.
- Produces, in `e2e/seed.rs`: `pub fn fixed_status(ip: &str) -> ServerStatus` — `"10.0.0.1:8000"` Online 3/20 42 ms, `"10.0.0.2"` Online 18/20 180 ms, `"10.0.0.3:8000"` Incompatible, `"10.0.0.4:8000"` Offline, `"nowhere.invalid"` UnknownHost, `"10.0.0.9"` Online 1/8 20 ms, else Offline; `pub fn slow_status(ip: &str) -> ServerStatus` (sleeps 300 ms, then `fixed_status`); `GameFolder::with_status_servers(self) -> Self` seeding `Survival 10.0.0.1:8000`, `Creative 10.0.0.2`, `Old friends 10.0.0.3:8000`, `Friends 10.0.0.4:8000`, `Typo nowhere.invalid`. In `e2e/multi.rs`: `fn open_server_list_with(folder, probe: StatusProbe) -> Harness` (inserts the probe before clicking Multiplayer, waits for `SERVERS_TITLE`), `fn card(name) -> L`, `fn card_names(h)`, `fn selected_server(h) -> Option<String>`, and `impl Harness { pub(super) fn join_server_card(&mut self, name: &str) }` (click the card, click `L::role(Role::Button, "Join")`), which `e2e/game.rs`'s connection-screen test uses instead of `wait_for_icons` + `"Play Nowhere"`.

- [ ] **Step 1: Write the failing tests**

Unit, `multi/status.rs`:
- `each_status_reads_as_a_word_and_a_colour` — `card_status`: Online → `("3 / 20", TEXT)`, Pinging → `("Pinging...", MUTED)`, Offline → `MUTED`, Incompatible / UnknownHost / BadAddress → `DANGER`; `pane_status(Online{.., 42 ms}).2 == "42 ms"` with `"3 / 20 players"` in the headline line.
- `a_refresh_drops_the_probes_in_flight` — `ping("a", slow Offline probe)`, then `ping("a", instant Online probe)`; poll `collect` until both threads are done → `status("a")` is Online, and stays so after the slow one ends.

Unit, `multi/mod.rs`: `a_join_resolved_after_the_screen_closed_hands_nothing_over` — bare `App` with `finish_join` and states; a root with `PendingJoin` of a finished `Ok(10.0.0.1:8000)` is despawned before the update → `TargetServer.address == None`, `GameState` unchanged.

E2e, `e2e/multi.rs` (`status_list()` = `open_server_list_with(GameFolder::new().with_status_servers(), StatusProbe(fixed_status))`, waiting until no `"Pinging..."` is visible):
- `the_server_list_matches_its_golden` — `hover_at` a corner; golden `menu_multi`. New `the_empty_server_list_matches_its_golden` — `servers.ron` = `"[]"`; golden `menu_multi_empty`.
- `the_servers_are_listed_with_their_status` (replaces `localhost_and_the_saved_servers_are_listed`) — `card_names == ["Survival", "Creative", "Old friends", "Friends", "Typo"]`; within `card("Survival")`: `"10.0.0.1:8000"`, `"3 / 20"`, a `LatencyBars(4)`; `card("Creative")`: `"18 / 20"`, `LatencyBars(2)`; `"Incompatible"`, `"Offline"`, `"Unknown host"` within their cards.
- `the_first_server_is_selected_and_shown_in_the_detail_pane` — `focus_label() == Some("Survival")`; within `ServerDetail`: `"Survival"`, `"3 / 20 players"`, `"42 ms"`; click `card("Old friends")` → `"Incompatible version"`, Join has `InteractionDisabled`; Enter on it, `frames(3)` → `menu_state() == Multi`.
- `refresh_pings_every_server_again` — probe `slow_status`, wait for `"3 / 20"`; F5 → `"Pinging..."` within `card("Survival")` and `card("Friends")`, then `"3 / 20"` again; click `"Refresh"` → same.
- `nothing_listening_shows_offline` — default probe, one server `"Nowhere"` at `127.0.0.1:9` → `"Offline"` within its card in less than 5 s.
- `the_keyboard_alone_adds_a_server_sees_it_online_and_joins` (the server journey; replaces `connecting_to_a_server_enters_the_game`) — a dedicated server from `acquire_local_socket_pair` + `init_with_status`; empty game folder; from the main menu: ArrowDown → `focus_label() == Some("Multiplayer")`, Enter → `"Servers"`, `focus_label() == Some("localhost")` and, after 3 frames, `game_state() == Menu`; `ctrl(KeyN)`, `type_text("Local test")`, Tab, `type_text(&format!("localhost:{port}"))`, Enter → `focus_label() == Some("Local test")`, `saved_names == ["localhost", "Local test"]`; `"0 / 64"` within `card("Local test")` within 5 s; Enter → `GameState::Game` within `SERVER`, `TargetServer.address == Some(127.0.0.1:{port})`.
- `join_hands_a_host_name_over_with_the_default_port` — `fixed_status`, one server `"Home"` at `"localhost"`; click Join → `TargetServer.address == Some(127.0.0.1:8000)`, `GameState::PreGameLoading`; click `"[Cancel]"` → `GameState::Menu`.
- `remove_asks_first_and_the_focus_moves_to_the_next_server` (replaces `deleting_a_server_removes_it`) — click `card("Creative")`, Del → `L::role(Role::Dialog, "Remove \"Creative\"?")`, `"10.0.0.2 will leave your server list."`, `focus_label() == Some("Cancel")`; Escape → nothing removed; Remove in the pane, Remove in the dialog → `saved_names` without Creative, `card_names` too, focus and selection on `"Old friends"`; remove the four others → `"No servers yet"` visible, no `ServerDetail` shown, `focus_label() == Some("Add server")`.
- `a_removed_localhost_stays_removed` — empty folder: `card_names == ["localhost"]`; remove it; Escape, Multiplayer → `"No servers yet"`; the file reads `[]`.
- `an_unreadable_server_list_is_said_and_never_overwritten` — `servers.ron` = `"not ron"` → `"Cannot read servers.ron"` visible, Add server and Refresh have `InteractionDisabled`, `focus_label() == Some("Back")`; `ctrl(KeyN)`, `frames(2)` → no dialog; Escape → main menu; the file still `"not ron"`.
- `list_keys_do_nothing_while_a_dialog_is_open` — `add_dialog`; F5, Delete, `ctrl(KeyN)` → still one dialog, no `"Pinging..."`, `card_names` unchanged.
- `the_key_hints_follow_the_focus` — a card focused → `"Del"` and `"F5"` within `HintBar`; Tab → `"Del"` gone, `"Ctrl+N"` shown; `add_dialog` → `"Add"` within the dialog's bar.
- `hovering_a_card_or_a_button_lightens_it` — hover `card("Creative")` → `RAISE_HOVER`; hover `"Add server"` → `RAISE_HOVER`.
- `the_wheel_scrolls_only_the_list_under_the_cursor` — 15 servers `server 01…15` at `10.0.1.n`; hover `card("server 01")`, `scroll(-5.)` → its centre moved up; hover the pane, `scroll(-5.)` → nothing moved.
- `back_and_escape_return_to_the_main_menu` (kept) — click `"Back"`; Multiplayer, Escape (a card focused) → `MenuState::Main` both times.
- `a_long_name_and_address_stay_inside_their_card_at_800x600` — name 32 × `W`, address `a…a.example.com` of 64 characters, 800×600 → both texts inside `rect_of(card(..))` and inside `rect_of(ServerDetail)`.
- Locators elsewhere: `e2e/main_menu.rs` and `e2e/capture.rs` wait for `L::text(SERVERS_TITLE)` instead of `"Server list"`; `e2e/review.rs`'s Multiplayer line waits for `"Servers"` and goes back with `"Back"`; `e2e/game.rs` uses `join_server_card("Nowhere")`; `wait_for_icons` and `play_rows` go.

- [ ] **Step 2: Run them to verify they fail** — `timeout 300 cargo test -q -p client -- e2e::multi menus::multi` → compile errors, then no `"Servers"`.

- [ ] **Step 3: Implement** as in Interfaces; remove the allowances of Tasks 2 and 3 and every orphan (clippy without allowance proves each item left has a user).

- [ ] **Step 4: Run the tests; regenerate the goldens on purpose** — `menu_multi`: the dirt background, "Servers", Refresh and Add server in the title bar, five cards (green dot and four bars on Survival, two on Creative, red dots and words on Old friends and Typo, grey "Offline"), the first with its accent border, the pane with "42 ms" and green Join, Back, the hint bar, no ring, no hover. `menu_multi_empty`: "No servers yet" and its line, no pane, Add server without ring. `UPDATE_GOLDEN=1 timeout 300 cargo test -q -p client -- the_server_list_matches the_empty_server_list_matches`, rerun → PASS, no leftovers. Full suite green.

- [ ] **Step 5: Commit** (the report lists replaced tests and changed locators) — `git commit -m "feat(client): the Multiplayer screen with server status, a detail pane and a confirmed remove"`

---

### Task 7: Edit server

**Files:**
- Modify: `client/src/ui/menus/multi/{mod,dialogs}.rs`, `client/src/e2e/multi.rs`, `client/tests/golden/menu_multi.png` (the third button moves the pane: regenerate after a look and say so)

**Interfaces:**
- Consumes: `ServerForm` with `editing: Some(server)`, `edit_server`, `ServersChanged`, `StatusBoard::ping`.
- Produces: `ServerAction::Edit` (a `button("Edit")` between Join and Remove); `pub fn open_edit_server_dialog(commands: &mut Commands, assets: &AssetServer, focus: &InputFocus, server: &ServerItem, others: Vec<ServerItem>) -> Entity` — Decision 10, both fields filled, hints `[Tab Move, Enter Save, Esc Cancel]`, Save disabled (no message) while name and address equal `editing`'s; `check_server_form` compares against `others` without the edited server; submit → `edit_server(path, &old.name, new)` → `close_dialog(root, false)`, `ServersChanged { focus: Some(new.name) }`, and `ping` when the address key changed; `Err` → `FieldError(save_failure)` on the address.

- [ ] **Step 1: Write the failing test** (e2e, `e2e/multi.rs`)

`edit_changes_the_server_in_place_and_pings_a_new_address` — `status_list()`; select `"Friends"`, click `L::role(Role::Button, "Edit")` → `L::role(Role::Dialog, "Edit \"Friends\"")`, values `"Friends"` / `"10.0.0.4:8000"`, Save has `InteractionDisabled`, no message; `type_text("Survival")` → `SERVER_NAME_EXISTS`; select all, `type_text("friends")` → no message, Save enabled (its own name, another case); `type_text("Pals")` over it, Tab, select all, `type_text("10.0.0.1")` → `SERVER_EXISTS`; select all, `type_text("10.0.0.9")`, Enter → `saved_names[3] == "Pals"` with `ip == "10.0.0.9"`, `card_names[3] == "Pals"`, focus on `"Pals"`, `"1 / 8"` within its card; Edit again, Escape → file unchanged.

- [ ] **Step 2: Run it to verify it fails** — `timeout 300 cargo test -q -p client e2e::multi::edit` → no "Edit".

- [ ] **Step 3: Implement** as in Interfaces; remove the allowance on `edit_server`.

- [ ] **Step 4: Run the tests and the suite** → PASS, full suite green.

- [ ] **Step 5: Commit** — `git commit -m "feat(client): edit a server's name and address"`

---

### Task 8: Review snapshots, positions from the window, docs

**Files:**
- Modify: `client/src/e2e/review.rs`, `README.md`, `docs/ARCHITECTURE.md`, plus whatever the review finds in the files of Tasks 5–7

- [ ] **Step 1: Extend the review journey** — per resolution, with `StatusProbe(fixed_status)` inserted and `with_status_servers()` plus a server named 32 × `W`: Multiplayer → `"multi"`; Ctrl+N → `"multi_add"`; Tab, `type_text("10.0.0.2")` → `"multi_add_error"` (`SERVER_EXISTS`), Escape; Del → `"multi_remove"`, Escape; Edit → `"multi_edit"`, Escape; End (the long name) → `"multi_long_name"`; Back. A second folder with `"[]"` → `"multi_empty"`.

- [ ] **Step 2: Run and look** — `UI_SNAPSHOTS=1 timeout 600 cargo test -q -p client review -- --ignored`, then read every `target/ui-snapshots/every_screen_at_three_resolutions/multi*_{800x600,1280x720,1920x1080}.png`. Check: everything fits 800×600 (list, pane with three buttons and its error line, Back above the hint bar, five hints on one line, both dialogs with their reserved error lines); dots and bars crisp and aligned at 1.5; the selected card's accent border distinct from the focus ring; `DANGER` only on Remove, errors and failed statuses; nothing overflows a card, the pane or a button. Fix any defect in its task's file with a test first.

- [ ] **Step 3: Positions from the window** — `rg -n "cursor_position|world_to_viewport" client/src`: every result that ends in a `Val::Px` divides by `UiScale`; fix any that does not, with a test.

- [ ] **Step 4: Docs** — ARCHITECTURE "Network": the status exchange (UDP game port + 1, 32-byte request and reply, `STATUS_FORMAT`, why not on the netcode socket, no amplification, `PROTOCOL_ID` unchanged and "Incompatible" when it differs). ARCHITECTURE "UI widgets": `FormChanged` / `apply_form_check` in `dialog.rs`; one sentence: the server list is rebuilt from `servers.ron` after every change, and statuses and name resolution run on threads (`InFlight`). README "Multiplayer": a dedicated server also listens on UDP `port + 1` for the server list's status (open both in a firewall); an older server shows "Offline"; host names and the default port 8000 are accepted. README "Saves": `servers.ron` is written at once.

- [ ] **Step 5: Full suite** — `timeout 900 cargo test --workspace --no-fail-fast` green; fmt and clippy clean; `git status` shows no `.actual.png` / `.diff.png`.

- [ ] **Step 6: Commit** — `git commit -m "test(client): review snapshots of the Multiplayer screen, and docs of the server status"`
