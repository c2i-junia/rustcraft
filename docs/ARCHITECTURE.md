# Architecture

Rustcraft is a workspace of three crates. This page says what each one holds, how a client and
a server talk, and in which order things happen. The details live in the doc comments of the
files named here.

## The three crates

| Crate | Role | Depends on |
|-------|------|------------|
| `shared` | What both sides must agree on: the network messages and their encoding, the block containers, the player simulation. | — |
| `server` | The authority on the world: it simulates the inputs of the players, generates and saves the world, and sends the changes. A library, plus the `server` binary for a dedicated server. | `shared` |
| `client` | The game window: menus, rendering, inputs, and the prediction of the local player. In solo mode it starts the server library in a thread of its own process. | `shared`, `server` |

Solo and multiplayer use the same code path: the client always talks to a server over UDP, on
the loopback interface in solo. The only difference is `GameServerConfig::is_solo`: a solo
server stops when its player leaves and saves on request.

## `shared`

- `messages/`: `ClientToServerMessage` and `ServerToClientMessage`, and the structs they carry.
- `codec.rs`: a message is encoded with bincode, then compressed with lz4. The decoders bound
  the decompressed size before allocating.
- `channels.rs`: the renet channels and the channel of each message.
- `world/`: `Chunk` (a dense cube of `CHUNK_SIZE`³ cells), `ChunkMap` (the loaded chunks and
  the log of the blocks that changed), blocks, items, mobs and the raycast.
- `players/`: `simulate_player_actions`, made of the block interactions (`blocks.rs`) and the
  movement (`movement.rs`).
- `paths.rs`: where the game keeps its saves and assets on each platform.

The enums that travel in messages or are written in chunk files are append only: their
position is their encoding. Tests pin every value (`world/blocks.rs`, `world/items.rs`,
`messages/mod.rs`), and `PROTOCOL_ID` must be bumped with any change.

## Network

The game goes through netcode (renet) on one UDP port, the game port (`shared::DEFAULT_PORT`,
8000, by default). The server list asks a server for its status before anyone joins it, on a
second UDP port, the game port + 1 (`shared::status::status_port`), where a thread of the
dedicated server answers (`server/src/network/status.rs`); the integrated server of solo opens
none. The status is not on the game socket because `NetcodeServerTransport` reads that socket
itself and drops whatever is not netcode: a second reader would race it for packets.

A request and a reply are both `STATUS_PACKET_LEN` = 32 bytes (`shared/src/status.rs`): the
magic `RCST`, `STATUS_FORMAT`, the kind and a nonce, and in a reply the server's `PROTOCOL_ID`,
its players and its maximum. A reply is never larger than its request, so the port cannot
amplify a spoofed request, and the server drops anything but a well-formed request of exactly
32 bytes. The client (`client/src/network/status.rs`) asks from a connected socket, sends a new
nonce every 500 ms, takes only a reply that carries one of them, measures the latency from the
send of that nonce, and says "Offline" after 2 s, or at once when the port is refused. The
exchange is versioned by its own `STATUS_FORMAT`, not by `PROTOCOL_ID`, which it leaves
unchanged: a reply whose protocol differs from the client's makes the server "Incompatible",
and the client never joins it. Format 1 never changes; a new layout gets a new format, and
servers keep answering the older ones.

A session (`client/src/network/session.rs`) goes from the Singleplayer or Multiplayer screen to
the game and back. `SessionOrigin` says where it was started from: a world, or a server with
its address as typed. `SessionEnd` says why it ended, and the menus read it when they open: no
error is a cancel, which lands on the list the session came from with its card selected; an
error opens the error card (`ui/menus/connect_error.rs`), with the way back to that list and,
when trying again may succeed, Try again. Every writer of an end goes through `end_session`,
which does nothing while the game state already changes in the frame: the first end of a
session wins, whatever the order of the systems.

While the connecting card shows (`GameState::PreGameLoading`), `ConnectWatch` holds the step of
the session: Starting the world (the local server loads, then accepts the player) or Connecting
(netcode, then the authentication), then Loading chunks `n / 27`, then Spawning (the textures
and the local player). From the authentication on, the client reads the server's messages and
counts the chunks within `shared::BROADCAST_CHUNK_RADIUS` (1) of the local player's chunk, the
(2r + 1)³ = 27 chunks the server sends around a player; it enters the game once all of them are
in, so a player never spawns over the void. The radius is therefore part of the protocol: a
change of it bumps `PROTOCOL_ID`. Entering the game asks a mesh for every chunk held, and a
cancel forgets them. The mob and player updates and the `BlockChanged` messages that arrive
while loading are dropped: the server sends the mobs and the players again on every broadcast,
and a changed block is written into the map anyway, only its sound and particles are lost.

A failure is read from what netcode and the status already say, without a protocol change
(`failure_of`). Before the authentication, a denied connection is a full server; a disconnect by
the server is a refusal (the server drops a player whose name is in use), or an invalid name
when `sanitize_username` refuses ours; anything else is no answer, after which the status of a
server is asked once, and a server of another protocol says so. After the authentication and
before the game, a dropped connection is lost while loading; in the game, a disconnect by the
server is a server that stopped (a dedicated server only disconnects a player when it shuts
down), anything else a lost connection. A name no server accepts ends the session before a local
server starts or a packet leaves. `CONNECT_TIMEOUT` (10 s) without a new step ends a session
that is Connecting or Loading chunks. It counts from the last change of step, not from the start:
a busy server may send the chunks of other players first, and each chunk that comes in is a new
step. Netcode itself gives up on a silent server only after 15 s, the timeout of renetcode's
unsecure connect token, so the 10 s are what end a silent connect. Starting the world and
Spawning have no timeout.

The local server of a world runs on a thread of the client (`LocalServerHandle`), which is the
watchdog of Starting the world: a thread that finishes says why the world could not be opened,
and, in the game, that the world stopped (with its error, if any) without waiting for netcode;
a connection that gives up while the thread still loads (a world slower than netcode's
15 s) is opened again, and the authentication sent again. A local server is told to stop
whenever its session ends: on Cancel, on an error, and when the player leaves the game. One that
cannot be told yet, because its connection is not up, is stopped from the menus, the error card
included, once it is (`finish_stopping_local_server`); opening a world while the last one is still closing says so
(`WorldBusy`) rather than starting a second server on the same save.

## Prediction and reconciliation

The player simulation in `shared/src/players` runs on both sides, and both must get the same
result from the same input.

1. Each rendered frame, the client gathers the inputs of the frame in a `PlayerFrameInput`,
   applies it to its own copy of the player and of the blocks, and records the movement state
   it predicts.
2. At the tick rate (20 Hz), the client uploads the inputs it has gathered.
3. The server applies each input to its own state, which is the truth, and broadcasts a
   `PlayerUpdate` holding the state reached and the time of the last input it processed.
4. The client compares that state with what it predicted for that input. If they differ, it
   takes the server's state and replays the inputs the server has not processed yet
   (`client/src/player/update.rs`).

Blocks follow the same idea: the client applies its own clicks at once, and the server sends
`BlockUpdates` for every block that changed. When the server refuses an interaction, it
restates the block so that a wrong prediction is corrected. The flow of water is not
predicted: the client only receives the cells it changes.

## One server tick

The server runs at 20 ticks per second. `ServerSet` (`server/src/network/dispatcher.rs`)
orders the systems of `Update`:

1. **Receive**: connection events, then the messages of each client, one handler per kind of
   message (`network/handlers.rs`). A client that is not authenticated may only ask to be.
2. **Simulate**: the inputs are applied to the world. Chunks are generated around the players
   before and after each input. Then the water flows (`world/flow.rs`): every 5 ticks, the
   cells next to a block that changed take the level their neighbours give them
   (`shared/src/world/liquid.rs`).
3. **Broadcast**: block changes, new chunks and mobs are sent to each client.
4. **Save**: on request or every 60 seconds, the metadata and the changed chunks are handed
   to the writer thread.

Mobs move in `FixedUpdate`, which runs before `Update`.

## One client frame

`GameSet` (`client/src/state.rs`) orders the in-game systems of `Update`; `game_plugin`
(`client/src/game.rs`) chains them:

1. **UiMode**: is a dialog open, or is the player typing?
2. **Input**: the camera, the selected hotbar slot and the view mode of the frame.
3. **Simulation**: block interactions, then movement, predicted on the local player.
4. **Reconciliation**: the states received from the server are applied.
5. **Camera**: the camera is placed on the player.
6. **Meshing** (in `PostUpdate`): the chunks whose blocks changed get a new mesh, built on the
   task pool.

The network is polled in `FixedPreUpdate` and the inputs are uploaded in `FixedUpdate`.

The client has one plugin per domain, registered by `game_plugin`: `camera`, `input`, `mob`,
`network`, `player`, `ui::hud` and `world`. The menus have their own plugin
(`ui::menus::menu_plugin`), and so do the sounds (`audio::audio_plugin`), heard in the menus
too. The gameplay sends messages (`BlockChanged`, `LocalPlayerMoved`) that `audio` turns into
sounds; the sound files in `data/sounds/` are made by `scripts/generate_sounds.py`. `GameState` goes from `Menu` to `PreGameLoading` (connection and
textures) to `Game`.

## UI widgets

`ui/theme.rs` holds the tokens of the visual system: colours, spacing, font sizes and bevels.
`ui/widgets/` holds one file per widget, plus `focus.rs` (Tab navigation, the focus ring, arrow
navigation) and `scale.rs` (the UI scale that follows the window height); `widgets_plugin` adds
them all, in `app::add_client`. A widget is a headless `bevy::ui_widgets` widget, or
`EditableText` for a text field, dressed with our visuals, a `TabIndex`, an accessibility role
and a label. A text field triggers `Submit` (Enter) and `Cancel` (Escape) on its input, after
`EditableTextSystems` has applied the edits of the frame. `UIMode::Typing` means that the focus
is on an `EditableText`. `slot.rs` is an item slot: its icon at twice the size of its texture,
its count, its number in the hotbar and its selection. `tooltip.rs` is the one tooltip by the
cursor, for any hovered entity with a `Tooltip`. The inventory is one panel whose bottom row is
the hotbar, and every change to it is an `InventoryAction` predicted by the client and applied
by the server. `dialog.rs` is a modal over the screen: its focused action (Cancel, in a
confirmation) takes the focus, Escape closes it, the focus comes back where it was, and it
closes in `PostUpdate`. `hints.rs` is the key-hint bar at the bottom of a menu and its key caps,
whose arrows are drawn in Fira Mono. `tabs.rs` and `segmented.rs` are rows of options on a
`RadioGroup`; `slider.rs` moves by 5 % steps with the arrows and the wheel, which only moves the
slider under the cursor (`scroll.rs` turns the wheel and the trackpad into whole steps).
`list.rs` is a scroll list with its scrollbar, shown only while the items overflow, and its
selectable cards: a card is selected by taking the focus, the list is one tab stop (the selected
card), and ↑ ↓ Home End move between the cards. `dialog.rs` also builds form dialogs, with text
fields above the buttons: Enter in a field or the primary action triggers `DialogSubmit`, and
the caller confirms the submission by closing the dialog with `close_dialog`; Escape in a field
closes the dialog. Once its fields hold their first values, and after each change, a form
dialog triggers `FormChanged` with their values and which ones were touched; the screen checks
them and hands the messages and whether the form can be submitted to `apply_form_check`. A field
built with `reserve_error` (`text_field.rs`) keeps the room of its
one-line error message, so that a message appearing moves nothing. The Singleplayer screen
(`ui/menus/solo/`) rebuilds its world list from the disk after every change, with
`list_worlds`, which the main menu's Continue card reads too. The Multiplayer screen
(`ui/menus/multi/`) rebuilds its server list from `servers.ron` after every change, and asks
the statuses and resolves the host names on threads (`InFlight`), so a frame never waits for the
network. A dialog and a key capture take
Escape through `FocusedInput<KeyboardInput>` and finish in `PostUpdate`; the handlers that
read `ButtonInput<KeyCode>` in `Update` (the Escape of the main menu, of the world and server
lists, of the settings and of the pause menu, and the New world opener, Ctrl+N) skip while a
dialog is open, a text field has the focus, a key is being captured or, for the pause menu, the
settings are open. The connecting card reads Escape the same way: no dialog or field exists
there. The settings are one screen (`ui/menus/settings/`): a `MenuState` in the menus, an overlay above the
game from the pause menu. `progress.rs` is a progress bar: a sunken track whose `OK` fill
follows the value its screen sets on `ProgressBar`, told as a percentage to accessibility. The
connecting card (`ui/menus/connecting.rs`) and the error card (`ui/menus/connect_error.rs`)
share a width and the centre of the window, so that the one takes the place of the other; the
error card is a dialog spawned with `spawn_dialog_with_hints`, whose hints replace the dialog's
own (no `Tab Move` when the card has a single action). The pause menu is the last screen still
built on `Interaction` buttons; it moves over to the widgets in the last lot.

## Saves

A world is a folder under `saves/`:

| Path | Content | Written by |
|------|---------|------------|
| `world.ron` | Name, seed, time, mobs and the format version | the writer thread |
| `chunks/<x>_<y>_<z>.bin` | One chunk, encoded like a network message | the writer thread |
| `players/<id>.ron` | Position, orientation and inventory of one player | the tick thread |

Every file is written to a temporary file and renamed, so a crash never leaves a half-written
file (`server/src/world/format.rs`). The writer thread (`world/writer.rs`) keeps the disk out
of the tick; only the changed chunks are written. Worlds saved by older versions are converted
on their first load and the old files are kept: `world.ron.legacy` for the single-file formats,
`chunks.v1/` for the chunks of format version 1, whose blocks had no level (`world/legacy.rs`,
`world/load_from_file.rs`).

The Singleplayer screen manages the folders (`client/src/ui/menus/worlds.rs`): it writes
`world.ron` when a world is created, so that the world is listed at once; a rename renames the
folder and the name in `world.ron`, keeping its format version and its date; a delete removes
the folder, after a confirmation.

## End-to-end tests

`main.rs` builds the `App` and hands the wiring of the client to `app::add_client` (`client/src/app.rs`). The end-to-end tests (`client/src/e2e/`, compiled for tests only) call the same function on an `App` with a surfaceless window, so they run the client that players run: a `Harness` finds elements by their text, `Name`, marker or accessibility role, injects the mouse (including the window events of picking) and keyboard as messages, and draws the cameras into an image for the golden screenshots and the review snapshots. In game, the tests go through the integrated server, on wall-clock time. How to write one is in the README.

## Known limits

- The transport is not authenticated (`ServerAuthentication::Unsecure`): a client announces
  its own id, which is the hash of its player name, and the save of a player is found by that
  id. This is fine among friends, not for a server open to anyone.
- Block interactions are processed once per input, so once per rendered frame of the client:
  a client with a higher frame rate breaks blocks faster.
- Mobs are a prototype: one fox, spawned at a fixed tick.
- An item that does not fit in a full inventory is lost.
- Quitting the game while a world is still starting stops its server thread wherever it is in
  the load, with the process: the server cannot be told to stop before it accepts the player.
