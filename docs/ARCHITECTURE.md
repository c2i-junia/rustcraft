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
- `channels.rs`: the renet channels and the channel of each message, how many bytes a
  connection may send per tick (`AVAILABLE_BYTES_PER_TICK`), and the size of the socket
  buffers, enlarged by both ends so that a burst of chunks is not dropped by the kernel.
- `world/`: `Chunk` (a cube of `CHUNK_SIZE`³ cells, held as the one cell they all are when
  they are of one kind, as the air above the ground and the stone below it, else as the kinds
  of cells it holds and a byte per cell naming its kind, else, past 256 kinds, as a dense
  array; its copies share the cells until one of them changes: meshing, sending and saving a
  chunk copy it for nothing), `ChunkMap` (the
  loaded chunks and the log of the blocks that changed), blocks, items, mobs and the raycast.
  A chunk is encoded as its runs of equal cells, half the bytes of its cells and ten
  times faster to decode; a sequence of exactly `CHUNK_SIZE`³ elements is read as cells, which
  is how RON and the saves of format version 3 hold a chunk.
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
and the local player). The authentication is sent as soon as netcode is connected. From then
on, the client reads the server's messages and
counts the chunks within `shared::SPAWN_CHUNK_RADIUS` (1) of the local player's chunk, the
(2r + 1)³ = 27 chunks nearest to it; it enters the game once all of them are in, so a player
never spawns over the void. Every render distance reaches that radius and the server sends the
nearest chunks first, so they come before the rest. The radius is therefore part of the
protocol: a change of it bumps `PROTOCOL_ID`. Entering the game asks a mesh for every chunk held, and a
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
included, once it is (`finish_stopping_local_server`). Opening a world while the server of the
last one, told to stop, still saves waits for it on the connecting card ("Closing the last
world...") up to `STOP_TIMEOUT` (10 s), then says so (`WorldBusy`) rather than starting a second
server on the same save; one never told to stop says so at once.

Leaving a world never blocks a frame (`client/src/network/leave.rs`). Save and quit to title
triggers `LeaveGame`: `Exit` is sent to the local server at once, `ExitSent` records it so that
the end of the session does not send it again, and `Leaving` keeps the game running behind the
pause menu's "Saving..." until the server has saved and stopped, or for `STOP_TIMEOUT`, when the
title opens anyway and the server finishes in the background. Quitting saves a world first: the
window's close button and the main menu's Quit trigger `QuitRequested` (`ui/quit.rs`), and with
a world being played or still starting, a card says the world is being saved, the world is left
as by `LeaveGame` (a world still starting is cancelled, then told to stop once it accepts the
connection), and the app exits once its server stopped, or after `STOP_TIMEOUT`. A second close
quits at once. "Save now" knows when the world is written from a `SaveCounter`
(`shared/src/lib.rs`), counters of the saves written and failed that the embedded server bumps
and the client of the same process reads: nothing goes over the network, so the protocol is
unchanged, and a dedicated server's saves are not counted.

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

The contents of a chest live in the `ChunkMap`, at its position (`shared/src/world/container.rs`):
the server holds those of every chest, a client only those of the chest its player has open,
which the server sends with each `PlayerUpdate`. While a chest is open, its 27 stacks are lent
to the inventory as slots 45 to 71 for the length of one `InventoryAction`, so that a click on
them is a click on any slot (`shared/src/players/inventory.rs`). The client inserts the contents
the server sent before replaying its unacknowledged actions on them, as it does for the
inventory. A broken chest drops what it held on the ground with itself.

A tool (`ItemId::tool`, `shared/src/world/items.rs`) stacks to one, and its stack carries its
wear in its `ItemType::Tool`, so that the wear follows it into the cursor, a chest or the
ground. Held in the selected hotbar slot, a tool made for the aimed block
(`BlockId::is_mined_faster_by`) counts each mining hit as its speed, and each block it breaks
wears it, until it breaks itself (`shared/src/players/blocks.rs`). A block that needs a tool
(`BlockId::get_required_tool`), like stone and a pickaxe, still breaks without it, but its
`BrokenBlock` is not `harvested`, and the server drops none of its items. The client predicts
the wear like any change of the inventory.

The stance of a player is part of the `MovementState` both sides carry over
(`shared/src/players/movement.rs`): sprinting starts with `NetworkAction::Sprint` while moving
forward and lasts until the player stops going forward or sneaks, sneaking is the sneak key on
foot, and a sprint in a liquid is a swim, which goes where the camera looks. A sneaking player on
the ground refuses a step after which nothing would be under its box. The vertical move is cut
into steps of at most half a block, and a player that runs into a block is put against it: on
top when it falls, which lands it, right under when it rises, which does not. The blocks fallen
since the player last stood, swam or flew are carried over too, and `simulate_player_movement`
returns them when it lands.

The game mode (`shared/src/players/game_mode.rs`), the health and the cause of a death are the
server's: they are saved with the player, sent in every `PlayerUpdate`, and the client takes
them as they come, before it checks its prediction. The simulation reads the mode, which keeps
a survival player from flying, but changes neither: the server hurts a player that landed
(`fall_damage`, one point per block beyond 3) or fell below `FALL_LIMIT`, in survival only, and
gives a living player a point back every `REGENERATION_TICKS` (`world/simulation.rs`). A dead
player is not simulated: the server acknowledges its inputs without applying them, and the client
neither predicts nor sends any, and shows the death screen (`ui/menus/death.rs`) until the
server, asked by `Respawn`, brings the player back. Players spawn and respawn at
`ServerWorldMap::spawn`, computed once from the seed (`world/generation.rs`, `spawn_position`): on
the column nearest x = 0, z = 0 that no tree or cactus tops. `/gamemode` changes the mode of its
author.

## One server tick

The server runs at 20 ticks per second. Its loop turns every 4 ms (`LOOP_PERIOD`): the
transport sends and receives on every turn, so that the round trips of a connection do not
each wait for a tick, and the tick, the systems of `FixedUpdate`, runs on the turns its time
has come. A request to join brings the next tick forward to the turn it arrives on
(`take_join_requests_system`), so that the player gets its chunks at once. `ServerSet`
(`server/src/network/dispatcher.rs`) orders the systems of the tick:

1. **Receive**: connection events, then the messages of each client, one handler per kind of
   message (`network/handlers.rs`). A client that is not authenticated may only ask to be.
   A chat message that starts with `/` is a command (`network/commands.rs`): the server runs
   it and answers its author alone. Their syntax is in `shared/src/commands.rs`, which the
   client completes them from.
2. **Simulate**: the inputs are applied to the world. Chunks are loaded within the render
   distance of the players before the inputs, in the background (`ChunkLoader`): a tick
   inserts the chunks made since the last one and asks for the missing ones, nearest first,
   at most `CHUNKS_IN_FLIGHT` (8192) at once, which the server's own pool, of up to 16
   threads (`world/pool.rs`), reads or generates between the ticks. A chunk next to an input
   is made at once, on the tick. The generator computes the columns of the terrain once per
   column of chunks (`ColumnsCache`) and samples the noises of the caves on a coarse grid it
   interpolates (`world/caves.rs`). While no player changes chunk, the chunks around them are not looked at again
   (`LoadedAround`). A chunk is read from its file in
   the world's folder the first time a player needs it, else generated: opening a world reads
   no chunk. Then the water flows (`world/flow.rs`): every 5 ticks, the
   cells next to a block that changed take the level their neighbours give them
   (`shared/src/world/liquid.rs`).
3. **Broadcast**: block changes, new chunks and mobs are sent to each client.

   Each client gets the chunks within the render distance of its own player along x and z,
   and within at most `MAX_VERTICAL_RENDER_DISTANCE` (8) of it along y, as the ground lies
   within a few chunks of height (`within_render_distance`): nearest first
   (`render_distance_offsets`) and at most 1024 per tick, not counting the chunks of one kind
   of cell, in messages of 32 chunks encoded in parallel; the first one carries the mobs.
   The player chooses it, from 1 to `MAX_CHUNK_RENDER_DISTANCE_RADIUS` (64), in the Video
   tab of the settings or with the O and P keys (`RenderDistance`, saved in `video.ron`): the
   authentication request carries it and `SetRenderDistance` changes it. The answer to the
   authentication carries the server's maximum (`ServerRenderDistanceLimit`), which the Video
   tab names in game when it is below the choice; a world of Singleplayer grants 64. The server keeps it between
   `MIN_RENDER_DISTANCE` (1) and its `--max-render-distance` (`DEFAULT_MAX_RENDER_DISTANCE`,
   6), in `RenderDistances`. `sent_chunks` records what each client has. A chunk more than
   the render distance plus `UNLOAD_MARGIN` away is unloaded, after a move as after a lowered
   render distance: the server forgets it sent it and tells the client in a
   `ChunkUnloads`, which removes it and despawns its meshes. A chunk of the margin is kept but
   never sent first, so that walking to and fro across a chunk border neither unloads nor
   sends anything. The server decides, as the client's predicted position runs ahead of the
   server's: a client that unloaded on its own could drop a chunk the server believes sent,
   and never get it back. `ChunkUnloads`, `BlockUpdates` and `WorldUpdate` share one ordered
   channel and go in that order, so that no update is sent for a chunk being unloaded.
4. **Save**: on request or every 60 seconds, the metadata, the players and the changed chunks
   are handed to the writer thread. A generated chunk is saved too, but not one of a single
   kind of cell, which is generated again at once. Then, every 5 seconds, the chunks more than
   `KEPT_MARGIN` beyond the reach of every player are forgotten (`world/unload.rs`), but those
   not saved yet, and none while the writer has saves to write: their file could be read back
   before it is written. A mob over chunks that are not loaded stays where it is.

The systems of the server run one after the other on its thread: an integrated server would
otherwise share the compute pool with its client, whose frames would wait for its ticks.

The mobs and the dropped items move first, before the inputs of the tick are read.

## One client frame

`GameSet` (`client/src/state.rs`) orders the in-game systems of `Update`; `game_plugin`
(`client/src/game.rs`) chains them:

1. **UiMode**: is a dialog open, or is the player typing?
2. **Input**: the camera, the selected hotbar slot and the view mode of the frame.
3. **Simulation**: block interactions, then movement, predicted on the local player.
4. **Reconciliation**: the states received from the server are applied.
5. **Camera**: the camera is placed on the player.
6. **Meshing** (in `PostUpdate`): the chunks are drawn by groups of 4 × 4 columns of chunks
   (`GROUP_WIDTH`), each one entity, and a second one for its liquids: a frame costs the CPU
   for each entity, far more than for each face. A group whose chunks changed gets a new
   mesh, built on the async compute pool, which has up to 8 threads
   (`app::task_pool_plugin`), with a task per chunk whose faces are put together. A chunk of
   air, or buried in stone, is not meshed. While chunks arrive, a group waits until its chunks
   and those of the columns along its sides are in (`WaitingChunks`), which would each mesh it
   again, so that a group is meshed once; at most 128 groups are
   meshed at once, the nearest to the player first, and about 200 000 vertices are shown a
   frame. A vertex holds its normal and colour in bytes and its atlas tile in a `u16`
   (`meshing.rs`), 32 bytes in all. The GPU culls the groups (`NoCpuCulling`); only those within 12 chunks of the player
   cast shadows; and a fog of the colour of the sky hides the edge of the render distance.

The network is polled in `PreUpdate`, every frame, and the inputs are uploaded in
`FixedUpdate`. A message of chunks of 4 KiB or more is decoded on the compute pool, and the
messages are applied in the order they came (`DecodingChunkMessages`), about 1024 chunks a
frame at most.

The client has one plugin per domain, registered by `game_plugin`: `camera`, `input`, `mob`,
`network`, `player`, `ui::hud` and `world`. The menus have their own plugin
(`ui::menus::menu_plugin`), and so do the sounds (`audio::audio_plugin`), heard in the menus
too. The gameplay sends messages (`BlockChanged`, `LocalPlayerMoved`) that `audio` turns into
sounds; the sound files in `data/sounds/` are made by `scripts/generate_sounds.py`. `GameState`
goes from `Menu` to `PreGameLoading` (connection and textures) to `Game`.

## UI widgets

`ui/theme.rs` holds the tokens of the visual system: colours, spacing, font sizes and bevels.
`ui/widgets/` holds one file per widget, plus `focus.rs` (Tab navigation, the focus ring, arrow
navigation) and `scale.rs` (the UI scale that follows the window height); `widgets_plugin` adds
them all, in `app::add_client`. A widget is a headless `bevy::ui_widgets` widget, or
`EditableText` for a text field, dressed with our visuals, a `TabIndex`, an accessibility role
and a label. A text field triggers `Submit` (Enter) and `Cancel` (Escape) on its input, after
`EditableTextSystems` has applied the edits of the frame. `UIMode::Typing` means that the focus
is on an `EditableText`. `slot.rs` is an item slot: its icon at twice the size of its texture,
its count, its number in the hotbar, the durability bar of a worn tool and its selection. `tooltip.rs` is the one tooltip by the
cursor, for any hovered entity with a `Tooltip`. The inventory is one panel whose bottom row is
the hotbar, and every change to it is an `InventoryAction` predicted by the client and applied
by the server. While a chest is open, its slots take the place of the crafting row. `dialog.rs` is a modal over the screen: its focused action (Cancel, in a
confirmation) takes the focus, Escape closes it, the focus comes back where it was, and it
closes in `PostUpdate`. `hints.rs` is the key-hint bar at the bottom of a menu and its key caps,
whose arrows are drawn in Fira Mono. `tabs.rs` and `segmented.rs` are rows of options on a
`RadioGroup`; `slider.rs` moves by 5 % steps with the arrows and the wheel, which only moves the
slider under the cursor (`scroll.rs` turns the wheel and the trackpad into whole steps).
`list.rs` is a scroll list with its scrollbar, shown only while the items overflow, and its
selectable cards: a card is selected by taking the focus, the list is one tab stop (the selected
card), and ↑ ↓ Home End move between the cards. A notch of the wheel moves a list by its own
`WheelStep` rather than Bevy's 100 px: one card in the world and server lists, one row in the
controls, three lines in the chat; a trackpad's pixels pass through unchanged. `dialog.rs` also
builds form dialogs, with text fields above the buttons: Enter in a field or the primary action
triggers `DialogSubmit`, and the caller confirms the submission by closing the dialog with
`close_dialog`; Escape in a field closes the dialog. Once its fields hold their first values,
and after each change, a form dialog triggers `FormChanged` with their values and which ones
were touched; the screen checks them and hands the messages and whether the form can be
submitted to `apply_form_check`. A field built with `reserve_error` (`text_field.rs`) keeps the
room of its one-line error message, so that a message appearing moves nothing. The Singleplayer
screen (`ui/menus/solo/`) rebuilds its world list from the disk after every change, with
`list_worlds`, which the main menu's Continue card reads too. The Multiplayer screen
(`ui/menus/multi/`) rebuilds its server list from `servers.ron` after every change, and asks the
statuses and resolves the host names on threads (`InFlight`), so a frame never waits for the
network. A dialog and a key capture take Escape through `FocusedInput<KeyboardInput>` and finish
in `PostUpdate`; the handlers that read `ButtonInput<KeyCode>` in `Update` (the Escape of the
main menu, of the world and server lists, of the settings and of the pause menu, and the New
world opener, Ctrl+N) skip while a modal is open (`modal_open`: a dialog, or the quitting card),
a text field has the focus, a key is being captured or, for the pause menu, the settings are
open. The connecting card reads Escape the same way: no dialog or field exists there. The
settings are one screen (`ui/menus/settings/`): a `MenuState` in the menus, an overlay above the
game from the pause menu. `progress.rs` is a progress bar: a sunken track whose `OK` fill
follows the value its screen sets on `ProgressBar`, told as a percentage to accessibility. The
connecting card (`ui/menus/connecting.rs`) and the error card (`ui/menus/connect_error.rs`)
share a width and the centre of the window, so that the one takes the place of the other; the
error card is a dialog spawned with `spawn_dialog_with_hints`, whose hints replace the dialog's
own (no `Tab Move` when the card has a single action). `button.rs` builds every button, and
`set_label` changes the text of one together with its accessible label. `toast.rs` shows short
messages at the top-right of the window (`show_toast`, an `Info` or an `Error`), which go by
themselves after 3 s, take no click and no focus, and outlive the screen that showed them.

The pause menu (`ui/menus/pause.rs`) is a modal, spawned when Escape opens it in the game and
despawned when it closes, so that it always opens on Back to game and Tab stays inside it. Its
title is "Game menu", not "Paused": the server keeps ticking beneath it. A world has Back to
game, Settings, Save now and Save and quit to title; a server has Back to game, Settings and
Disconnect, as a server is not the player's to save. The hotbar, its item name and the reticle
are hidden beneath it and beneath the settings it opens over the game. Save now sends the save
request and waits for the embedded server's `SaveCounter` to move, then says "World saved" in
a toast, or "Could not save the world" after a failure or 10 s; an autosave that fails says so
too, once. While the game is left, every button is disabled and the leaving one reads
"Saving...".

The top-right corner of the game is one column (`ui/hud/corner.rs`): the minimap, then the FPS
counter, so that they never overlap; the toasts keep below it (`ToastsBelow`). The minimap
(`ui/hud/minimap/`) is drawn on the CPU from `ClientWorldMap`, with no message of its own:
`MinimapSurface` keeps the top visible block of each column (glass and plants are seen through,
water keeps its depth over its floor), one tile of 16 × 16 per column of chunks. A column is
scanned again when one of its chunks arrives or goes (`ChunkLoadEvent`, written by
`network/world.rs`), at most `COLUMNS_PER_FRAME` columns a frame and the nearest to the player
first; a `BlockChanged` scans its own cell only. `compose` turns the surface into the 128 × 128
pixels of an `Image`, a block per pixel (or 2, or 4, zoomed out), north up, in the colours of the
top textures of the blocks (`MinimapColors`), the relief shaded from the north; it is drawn again
only when the player steps onto another column, the zoom changes or the surface does. The arrow
of the player, turned by the camera, the dots of the other players and of the mobs and the
coordinates are UI nodes over it. Like the hotbar, it hides under the pause menu and the
settings; the Interface tab shows or hides it (`InterfaceSettings::minimap`).

`M` opens the world map (`ui/hud/minimap/world_map.rs`), a dialog over the whole window: the
same surface, all of it, drawn by `compose_world` tile by tile into one image at a block a pixel,
again at most every 250 ms while the surface changes. The UI scales that image (1 to 8 px a
block) and moves it, so that dragging, the wheel (around the cursor) and `=` / `-` never draw it
again; the arrow and the dots are its children, placed in percent of it. It follows the player
until dragged, and Space centres it again. It joins `Overlays`, so the HUD hides under it, and
closes on `M` or Escape.

The chat (`ui/hud/chat.rs`) is a log of the last 100 messages with their times, over the field
to write in, at the bottom-left above the hotbar; closed, the recent lines stay over the game
for 7 s, then fade. The pixel font lacks many accented letters (é, è...): a message with a
character the pixel font lacks is drawn whole in Fira Mono, never with a missing letter in the
middle, and so is the text being written (`font_for`, `match_the_field_font`).

A command being typed is completed (`ui/hud/chat_completion.rs`): a panel over the field shows
the usage of the command with the argument being typed, or what the server would answer to a
wrong line, and lists the values the word may become; the field shows the rest of the chosen
one, or the arguments left, in grey after the caret. Tab takes it, Up and Down choose another
while the panel lists some (else they recall the sent messages), and Escape hides the panel
before it closes the chat. The panel is spawned once and only written to: a node spawned in
`PostUpdate` is laid out a frame late, and the panel would flicker at each letter.

## Saves

A world is a folder under `saves/`:

| Path | Content | Written by |
|------|---------|------------|
| `world.ron` | Name, seed, time, mobs and the format version | the writer thread |
| `chunks/<x>_<y>_<z>.region` | The chunks of a region of 8×8×8 chunks, each encoded like a network message, after an index of where each starts | the writer thread |
| `chunks/<x>_<y>_<z>.bin` | One chunk, encoded the same way, as worlds saved before version 5 hold them: read when its region does not hold it | worlds saved before |
| `containers/<x>_<y>_<z>.bin` | The contents of the chests of one chunk, encoded the same way; only a chunk holding a chest has one | the writer thread |
| `players/<id>.ron` | Position, orientation and inventory of one player | the writer thread |

Every file is written to a temporary file and renamed, so a crash never leaves a half-written
file (`server/src/world/format.rs`). The writer thread (`world/writer.rs`) keeps the disk out
of the tick. It writes the files of a save aside on 8 threads, syncs them all at once (one
`syncfs` on Linux, where syncing each file costs milliseconds; a save of 16 files or fewer syncs
each of them instead), then puts them in place, the metadata first. Only the changed chunks are written, into their regions, which keep the other chunks they held, with the file of their chests, which is
removed once they hold none (`world/container_files.rs`). Worlds saved by older versions are converted
on their first load and the old files are kept: `world.ron.legacy` for the single-file formats,
`chunks.v1/` for the chunks of format version 1, whose blocks had no level (`world/legacy.rs`,
`world/load_from_file.rs`). Format version 2 differs from 3 only by having no `containers/`,
3 from 4 only by writing a chunk cell by cell, and 4 from 5 only by a file per chunk: all load
as they are.

The Singleplayer screen manages the folders (`client/src/ui/menus/worlds.rs`): it writes
`world.ron` when a world is created, so that the world is listed at once; a rename renames the
folder and the name in `world.ron`, keeping its format version and its date; a delete removes
the folder, after a confirmation.

## End-to-end tests

`main.rs` builds the `App` and hands the wiring of the client to `app::add_client`
(`client/src/app.rs`). The end-to-end tests (`client/src/e2e/`, compiled for tests only) call
the same function on an `App` with a surfaceless window, so they run the client that players
run: a `Harness` finds elements by their text, `Name`, marker or accessibility role, injects the
mouse (including the window events of picking) and keyboard as messages, and draws the cameras
into an image for the golden screenshots and the review snapshots. In game, the tests go through
the integrated server, on wall-clock time. How to write one is in the README.

## Known limits

- The transport is not authenticated (`ServerAuthentication::Unsecure`): a client announces
  its own id, which is the hash of its player name, and the save of a player is found by that
  id. This is fine among friends, not for a server open to anyone.
- Block interactions are processed once per input, so once per rendered frame of the client:
  a client with a higher frame rate breaks blocks faster.
- Mobs are a prototype: one fox, spawned at a fixed tick.
- An item that does not fit in a full inventory is lost.
- A player who dies keeps its inventory, and nothing hurts it but falls.
- Any player may change its own game mode with `/gamemode`: there are no operators yet.
- A world still loading 10 s after the quit is stopped with the process: its server cannot be
  told to stop before it accepts the player. Its files are written atomically, so the save stays
  readable.
- Chinese, Japanese and Korean characters and emoji are in neither font, the pixel font nor Fira
  Mono: they show as missing glyphs in the chat.
- An accented letter pasted into the chat field may show for one frame in the pixel font, as a
  missing glyph, before the field switches to Fira Mono.
