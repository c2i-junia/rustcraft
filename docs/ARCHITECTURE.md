# Architecture

Rustcraft is a workspace of four crates. This page says what each one holds, how a client and
a server talk, and in which order things happen. The details live in the doc comments of the
files named here.

## The crates

| Crate | Role | Depends on |
|-------|------|------------|
| `crates/protocol` | What the game and the hub share, with no Bevy: device certificates and the join signature (`identity`), the JSON types of the hub API (`api`), the status packet codec (`status`) and `DEFAULT_HUB`. | — |
| `hub` (a separate Cargo project, not a member of the workspace) | The hub: accounts, device certificates and hosted worlds, served by `rustcraft-hub serve` over axum and Postgres; see `hub/README.md`. | `rustcraft-protocol` |
| `hub/realm-agent`, `hub/agent-protocol` (members of the hub's workspace) | The realm agent, `rustcraft-realm-agent`: the only part of the hub with Docker's socket, which runs the hosted worlds in containers on the hub's requests over a Unix socket; `rustcraft-agent-protocol` holds those requests and their responses. | `rustcraft-protocol`, `bollard` |
| `shared` | What both sides must agree on: the network messages and their encoding, the block containers, the player simulation. | `rustcraft-protocol` |
| `server` | The authority on the world: it simulates the inputs of the players, generates and saves the world, and sends the changes. A library, plus the `server` binary for a dedicated server. | `shared` |
| `client` | The game window: menus, rendering, inputs, and the prediction of the local player. In solo mode it starts the server library in a thread of its own process. | `shared`, `server` |

Solo and multiplayer use the same code path: the client always talks to a server over UDP, on
the loopback interface in solo. The only difference is `GameServerConfig::is_solo`: a solo
server stops when its player leaves and saves on request.
A dedicated server stops on SIGTERM or Ctrl-C: the `server` binary catches them (`ctrlc`) and
`shutdown_system` saves the world and the players, then sends `AppExit`; a second signal exits
with status 130.

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
  loaded chunks, the log of the blocks that changed and the portals), blocks, items, mobs, the
  furnace and the raycast. A portal (`world/portal.rs`) is 2 blocks wide and 3 high on any
  face of a block, its long side along its `up` (the vertical on a wall), each player owning a
  blue one and an orange one; `Portals::through`, a rigid transform, takes the space in front of
  a portal to the space in front of its twin, the other colour of the same owner.
  A chunk is encoded as its runs of equal cells, half the bytes of its cells and ten
  times faster to decode; a sequence of exactly `CHUNK_SIZE`³ elements is read as cells, which
  is how RON and the saves of format version 3 hold a chunk.
- `world/terrain.rs` and `world/biome.rs`: the shape of the world, which the server generates
  from and the client colours and rains by (see below).
- `players/`: `simulate_player_actions`, made of the block interactions (`blocks.rs`) and the
  movement (`movement.rs`).
- `paths.rs`: where the game keeps its saves and assets on each platform.

`TerrainNoises` gives the height and the biome of any column, a pure function of the seed, the
world type (`WorldGenSettings`: default, large biomes, archipelago, or a single biome) and the
biomes. Each column has a climate of five parameters, temperature, humidity, continentalness,
erosion and weirdness, each a fractal noise remapped so that it spreads evenly over [-1, 1]
(`Remap`): a biome's share of the climate is its share of the world. They are read where three
layers of warp lead, the coarsest first, which bend whole biomes and fray their borders, on
the corners of cells of 4 blocks, interpolated between (`column_shapes` reads an area,
sharing the corners). The continentalness and the erosion shape the ground, with the hills,
the mountain ranges, the rivers and the lakes.

A biome is data, a `BiomeDef` registered by its key (`rustcraft:jungle`) in a `BiomeRegistry`:
the boxes of the climate it holds, its priority and weight against the boxes it shares, its
ground, its plants and trees (`Flora`), its herds, its look (colours, precipitation, map
colour), its tags (`ocean`, `snowy`...) and the keys of the biomes it leads to: its variants
by altitude (the mountains), its sub-biomes, in patches deep inside it (the grove in the
plains), and the biome along its border (the jungle's edge). `BiomeRegistry::finish` gives
the ids: the biomes of the base game first, in the order of `vanilla::KEYS`, then the others
by key, so that a biome added or changed by code (`register`, `modify`) never moves another.
A column takes the biome of the highest priority whose box holds its climate, moved a little
by a white noise across the borders where both biomes allow it, one of several sharing a box
by a slow noise (`Biomes::pick`), else the nearest box; then its border or its sub-biome,
then the river over it, then its variant of altitude. The server sends its biomes and world
type to each client that joins (`AuthRegisterResponse`); biomes the client does not know are
drawn as sent, and the game warns about them. `worldgen_dump` and
`scripts/worldgen_preview.py` draw maps of the blocks, the biomes and the climate.

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

A request and a reply are both `STATUS_PACKET_LEN` = 32 bytes (`crates/protocol/src/status.rs`,
re-exported as `shared::status`): the magic `RCST`, `STATUS_FORMAT`, the kind and a nonce, and
in a reply the server's `PROTOCOL_ID`, its players and its maximum. A reply is never larger than
its request, so the port cannot amplify a spoofed request, and the server drops anything but a
well-formed request of exactly 32 bytes. The client (`client/src/network/status.rs`) asks from a
connected socket, sends a new nonce every 500 ms, takes only a reply that carries one of them,
measures the latency from the send of that nonce, and says "Offline" after 2 s, or at once when
the port is refused. The exchange is versioned by its own `STATUS_FORMAT`, not by `PROTOCOL_ID`,
which it leaves unchanged: a reply whose protocol differs from the client's makes the server
"Incompatible", and the client never joins it. Format 1 never changes; a new layout gets a new
format, and servers keep answering the older ones.

A session (`client/src/network/session.rs`) goes from the Singleplayer or Multiplayer screen to
the game and back. `SessionOrigin` says where it was started from: a world, or a server with
its address as typed, which may be a hosted world of the hub (`realm`). `SessionEnd` says why it ended, and the menus read it when they open: no
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
cancel forgets them. The mobs that arrive while loading are kept in `ClientMobs` and drawn
on entering the game, as the server sends a mob only when it comes within reach or changes.
The player updates and the `BlockChanged` messages that arrive while loading are dropped: the
server sends the players again on every broadcast, and a changed block is written into the map
anyway, only its sound and particles are lost.

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
restates the block so that a wrong prediction is corrected. The flow of the liquids is not
predicted: the client only receives the cells it changes.

The portals are placed with the portal gun (`ItemId::PortalGun`): held in the selected slot,
it shoots its owner's blue portal on a left click and the orange one on a right click, once
per press (`Player::portal_triggers_held`), up to `PORTAL_GUN_REACH` (32) blocks, and then
neither mines nor places blocks (`shared/src/players/blocks.rs`); a right click on a
container shoots too, where it would open it. It aims as any item but the empty bucket (`aim`),
through water sources, and further: its ray goes 36 blocks (`raycast_far`), every other item's
20, and only a block that collides stops it, so that it shoots through the vines, the flora and
the torches that may stand in front of a portal. The portal goes on the face aimed at, a wall, a floor or a ceiling, its long side up on a
wall and, on a floor or a ceiling, away from the shooter along the horizontal axis the shot
goes most along (`opening_up`), on the placement of the six holding the aimed cell nearest to the
point hit that fits: its wall cells solid full blocks (a furnace or a chest is one), its
front free of anything that collides (water, a vine or a fern does not; a cocoa pod, a furnace
or a chest does), no portal of another colour or player on those cells but on their opposite face, a
pair through a wall one block thick (`fit_portal`, `shared/src/world/portal.rs`). Placing a colour again moves it. The client predicts a shot like
a block, and the server's `PortalUpdates`, every portal of the world, replace its portals: the
server sends them to everyone in the tick they changed, and to a player when it joins. A
refused shot makes the server send them again, which corrects a wrong prediction. A player's
portals go when it leaves, and a portal goes when, after the block changes of a tick, its wall
or front no longer fits (`server/src/world/portals.rs`). Portals are not saved.
Every shot, placed or refused, is a `PortalShot`: from the origin of the aim ray to where it
meets the aimed face (or `PORTAL_GUN_REACH` along it when nothing is aimed at), its colour and
whether it placed the portal; the aim stops at the wall behind a portal, so a shot never goes
through one. The server keeps those of a tick (`ServerWorldMap::portal_shots`) and sends each to
every player but its shooter, who predicted it, as `ServerToClientMessage::PortalShot`, before
the `PortalUpdates` of the same tick on the same ordered channel. Its start is the camera of
the input, which comes from the network: the server tells of a shot only if its ends are numbers
and it starts within `SHOT_START_TOLERANCE` (2 blocks) of the shooter's eyes before the input
(`is_believable`), and a client flies no bolt for a shot whose ends are not numbers, nor any for
longer than `MAX_FLIGHT_TIME`.

Players go through the open portals in the movement simulation, which both sides run, so the
server decides and the client predicts (`shared/src/players/movement.rs`). When the centre of a
player goes, over one input, from in front of an open portal to behind it, through the opening
(`Portals::crossed`), its position, velocity and camera go through `Portals::through`. The
centre comes out within the twin's opening shrunk by the box's half extents, so that a box
falling out of a floor portal into a wall portal fits it, and no deeper in front of the twin
than half a block (`MAX_EXIT_DEPTH`, a player's longest step) nor than keeps the box within the
block in front, which a portal keeps clear: the rest of a fast move is dropped, never put inside
a block; the camera keeps where it looks, set
upright (`Look::upright`: no roll, the pitch clamped to `MAX_PITCH_DEGREES`), and the simulation
reports that look (`MovementOutcome::turned`). The client sets its camera to it when it predicts
a frame, never when it replays inputs after a correction, and its next inputs carry the turned
camera. A player keeps a horizontal velocity in the air, which only a portal gives it (falling
into a floor portal shoots it out of a wall as fast), blocked per axis by blocks, dropped when
it lands, flies or swims. Players, mobs and items fall no faster than `TERMINAL_VELOCITY` (73.5
blocks/s, what an item's drag tends to), so a fall between a floor and a ceiling portal settles
at that speed rather than growing for ever. While the centre of a player is in front of an open portal's opening,
shrunk by half its width, that portal's wall cells do not collide with it
(`Portals::passable_cells`, `check_player_collision`), so that it walks into the wall up to the
plane, or falls into a floor portal it stands on; the vertical step that takes its centre into
such a portal is made whatever lies behind the wall cells, and ends the input's fall. In front means at most a millimetre behind the plane (`PORTAL_SLACK`), so that no
rounding leaves a player that came out of a portal behind it, in its wall. A player left inside
blocks, as in the wall of a portal that closed or moved, may make the moves that reach no new
block and go no deeper into those it is in, so that it walks back out.

Mobs and dropped items go through the portals by the same rule, on the server, which simulates
them: `Portals` holds it for any entity, with its own box (`passable_cells`, `goes_into`, the
move into a portal made whatever lies behind its wall, and `pass`, the transform and the exit
point), and players, items (`step_dropped_item`) and mobs (`server/src/mob/behavior.rs`) call
it. An item's half-width is half its size, a mob's half the larger of its width and depth. An
entity that passes comes out with its velocity turned; a mob's body faces where it faced,
turned upright as a camera's look, and the spot it walks to goes through too, so it walks on
(through a floor and a wall it stops walking instead). A mob going faster than it goes by
itself, which only a portal does, flies by its velocity until it lands.
Out of a floor portal, gravity brings an entity back to it: one that comes out too slowly to
rise `REST_CLEARANCE` (half a block) clear of the portal rests on it (`Passage::resting`, which
players keep in `MovementState::portal_rest`, mobs and items in their own `portal_rest`, never
saved). That portal does not take it back, and its cells collide with it once its box is out
of them, so that it lands on it; one too slow to lift its box out of them is given just the
speed to. The portal takes it again once it has left its front or risen above the clearance,
as by a jump, or once it or its twin was shot again or went, as it may now lead elsewhere: the
rest keeps both portals as they were (`PortalRest`, `Portals::still_resting`). So an entity
resting on a floor portal whose twin is a floor too goes through once and stands on the twin,
rather than back and forth for ever; one that falls in from higher bounces from one to the
other as before. The rule holds for every pair whose exit is a floor: a player walking into a
wall portal whose twin is a floor comes up and stands on it. An item, whose gravity comes before
its move and its drag after, rises a little less high than it is lifted for: the cells of the
portal it rests on collide with it once it no longer rises, and it goes up out of them onto it.
Each counts the portals it went through (`DroppedItem::passes`, `ServerMob::passes`, sent with
it): a client that sees the count change puts the entity where it is told at once, instead of
easing it across the gap. Items are sent by the chunks a client has, those around the twins
included, and mobs by distance or in those chunks: what walks or falls to a far twin stays
known to a client that sees it there.

The contents of a chest live in the `ChunkMap`, at its position (`shared/src/world/container.rs`):
the server holds those of every chest, a client only those of the chest its player has open,
which the server sends with each `PlayerUpdate`. While a chest is open, its 27 stacks are lent
to the inventory as slots 45 to 71 for the length of one `InventoryAction`, so that a click on
them is a click on any slot (`shared/src/players/inventory.rs`). The item browser's actions are
inventory actions too: `TakeFromCatalog` and `DestroyFloating`, which `apply_inventory_action`
refuses unless its `creative` argument says the player is in Creative
(`simulate_inventory_actions` reads it from `Player::game_mode`, which the client's player follows
from the server, so that the prediction refuses what the server refuses), and
`FillCraftingGrid`, in both modes, which puts the grid back into the inventory and moves the
ingredients of a recipe into it, all or nothing. The client inserts the contents
the server sent before replaying its unacknowledged actions on them, as it does for the
inventory. A broken chest drops what it held on the ground with itself.

`simulate_inventory_actions` returns an `InventoryActionOutcome` for each action it applied: the
action, the floating stack before and after it, its result (`Ok` or the `InventoryRefusal`),
the stack it threw and the item it crafted. The server and the client's replays ignore it; only
the client's live prediction reports it, once, as `InventoryActionPredicted`, from which the
audio plays the pick, place and craft sounds and the item browser fills its Recent row. A
refusal goes through one door, the `ActionRefused` message of `ui/widgets/slot.rs`, whatever
made the action (`report_refused_actions` for a predicted one, the item browser for a refused
Ctrl+click): the audio plays the refused thud, and its slot shakes (`RefusalShake`). A refusal
of a slot already shaking shows nothing more, so that a burst of refusals shakes and thuds once.

What a player throws out of its inventory (Q, a click outside the panel with a held stack, or a
stack picked up from a cell and released outside the panel) comes back to the server in
`ActionsOutcome::thrown`, beside `dropped`, what it had no room for: `throw_from_player`
throws it faster, out of the player's reach, and it cannot be picked up for 40 ticks
(`THROWN_PICKUP_DELAY_TICKS`, 2 s, as in Minecraft) where an overflow waits 10. The delay is
not saved: a loaded item takes the shorter one.

A recipe cell is an `Ingredient` (`shared/src/world/crafting.rs`): an item, or a family,
`ItemTag`, whose members any one of fills it (`ItemTag::Planks`: every planks item), so that a
recipe of planks is written once and its species mix in the grid. Every fill of the grid has a
single entry point, `plan_grid_fill` (`shared/src/players/inventory.rs`): on a copy of the
inventory, it puts the grid back and plans the rounds and the item of each cell, a single
species of a family whenever one is enough, and which cells one round leaves without their
ingredient. `FillCraftingGrid` applies it, and the client's Ctrl+click on the item browser and
the ghost of a recipe over the grid (`ui/hud/inventory/ghost.rs`) call it too, so that they
agree with the server.

A furnace (`shared/src/world/furnace.rs`) holds an input, a fuel and an output stack, and how
long its fuel still burns and its input has cooked; `Furnace::tick` applies Minecraft's rules
to it, from the shared tables `smelting_result` and `fuel_ticks`, which the dialog and the item
browser read too. The `ChunkMap` keeps the chests' contents and the furnaces in one generic
store, `Held<T>`, by position, with the positions whose value changed. An open furnace
works as an open chest: its three stacks are lent to the inventory as slots 72 to 74 for the
length of an action, and the server sends what it holds with each `PlayerUpdateEvent`
(`open_furnace`), to its player only, 20 times a second, which the dialog draws its flame and
its arrow from. Only the server ticks the furnaces; the client predicts the clicks on their
slots, never the smelting. A lit furnace is a block of its own, `LitFurnace`, which the server
swaps with `Furnace` as it lights and goes out, so that the clients learn it from the block
updates and light it from `light_emission`.

A bucket is predicted like a placed block (`fill_bucket` and `pour_liquid` in
`shared/src/players/blocks.rs`). An empty bucket aims with `raycast_to_source`, which also
stops at the sources of water and lava, every other item through them (`aim`, which the client
outlines the aimed block with). Filled, it takes the source away; a water or lava bucket pours a
source against the aimed face, which the flow of the server then spreads. A full bucket filled
from a stack with no room left in the inventory is returned to the server in
`ActionsOutcome::dropped`, and dropped ahead of its player. A lava bucket burnt in a furnace
leaves its bucket in the fuel slot (`fuel_remains`).

A tool (`ItemId::tool`, `shared/src/world/items.rs`) stacks to one, and its stack carries its
wear in its `ItemType::Tool`, so that the wear follows it into the cursor, a chest or the
ground. Held in the selected hotbar slot, a tool made for the aimed block
(`BlockId::is_mined_faster_by`) counts each mining hit as its speed, and each block it breaks
wears it, until it breaks itself (`shared/src/players/blocks.rs`). The tier of a tool
(`ToolTier`: wood, stone, iron, diamond, gold) gives its speed, its durability and its mining
level. A block that needs a tool (`BlockId::get_required_tool`: a kind and a mining level),
like iron ore and a stone pickaxe, still breaks without it or with a lower tier, but its
`BrokenBlock` is not `harvested` (`BlockId::is_harvested_by`), and the server drops none of its
items. The client predicts
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
runs each player's metabolism every `REGENERATION_TICKS` (`metabolism_system`). Every hurt
goes through `Player::hurt(amount, DamageSource)`, which ignores a hit within
`INVULNERABILITY_TICKS` of the last one (the void and starvation pierce it) and turns the source
into the `DeathCause`, a deadly fall within `THROWN_OFF_TICKS` of a mob's hit being that mob's.
`Player::knock_back` sets a horizontal velocity, which the movement simulation carries and brakes:
being part of the `MovementState`, it reaches the client through the reconciliation. The client
sees its health drop and answers with a red vignette, a camera tilt, the hurt sound and blinking
hearts (`player/hurt.rs`). The air (`players/air.rs`) is the server's too, sent in every
`PlayerUpdate` but not saved: `breathe_system` takes a tick of it each tick the eyes
(`Player::eye_position`, where the client's camera is) are under the surface of a liquid
(`eye_in_liquid`, which the underwater fog reads as well), and with none left the player drowns,
2 damage every 20 ticks. The client draws it as bubbles over the right of the hotbar
(`ui/hud/air.rs`) while it is not full. Fire (`players/fire.rs`) works alike: `burn_system`
sets a player whose body touches a fire block burning for 80 ticks (`burning_ticks`, not saved),
1 damage every 20 ticks, until water around the body or rain on the head under the open sky puts
it out; the client draws flames over the bottom of the screen meanwhile (`ui/hud/burning.rs`).
Lava first (`bathe_in_lava`): a body in it burns for 300 ticks, which nothing puts out while it
stays there, and takes 4 damage every 10 ticks; it moves at 0.3 of its speed, and its eyes in
lava see an orange fog two blocks deep (`eye_liquid`).
Hunger (`players/hunger.rs`): `food` (0 to 20), `saturation` and `exhaustion` are saved with the
player, and only the food is sent. The efforts add to the exhaustion on the server only: the
movement simulation measures a sprint, a swim and a jump in `MovementOutcome.exhaustion`,
`simulate_player_actions` adds the broken blocks, and the server's input loop hands the sum to
`Player::exhaust`, as the blows (`mob/combat.rs`), the hits taken (`Player::hurt`) and the
health regenerated do; the client's prediction drops it. Every 4 of exhaustion take a point of
saturation, or of food once the saturation is gone. The metabolism (`Player::metabolise`) gives a
point of health back from 18 food, and starves a player with none. `Player::may_sprint` (more
than 6 food, or creative) is read by the movement simulation on both sides, the client with the
food of the last update. Eating (`players/blocks.rs`, `ItemId::food_value`) gives food and
saturation, refused at full food. The client draws drumsticks over the right of the hotbar
(`ui/hud/food.rs`), the bubbles above them.
A dead
player is not simulated: the server acknowledges its inputs without applying them, and the client
neither predicts nor sends any, and shows the death screen (`ui/menus/death.rs`) until the
server, asked by `Respawn`, brings the player back. Players spawn and respawn at
`ServerWorldMap::spawn`, computed once from the seed and the world type (`world/generation.rs`,
`spawn_position`): on the column nearest x = 0, z = 0, among one every 4 blocks, that stands deep
inside a biome that is not cold, wet or high, in a clearing where nothing grows. `/gamemode`
changes the mode of its author. `/locate biome <biome>` searches the nearest column of a biome
on the generation pool, and answers once it is found (`world/locate.rs`); `/locate village` the
nearest village, and `/locate ruin|crypt|sanctuary` the nearest dungeon, the same way.

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
   client completes them from. On a dedicated server every command but `/list` is an
   operator's (`commands::refusal`), the mods' included; a solo player is always an operator.
   `Permissions` (`network/permissions.rs`) holds the operators, the bans and the whitelist,
   read from `ops.ron`, `banned.ron` and `whitelist.ron` of the game folder when the server
   starts (a list it cannot read stops it, rather than forget its bans) and written atomically
   at each change; a solo world keeps them in memory only. `/op`, `/deop`, `/kick`, `/ban`,
   `/pardon` and `/whitelist` are run by `Permissions::apply`. The authentication refuses a
   banned player, and one off the whitelist while it is on (not an operator). A refused,
   kicked or banned client is sent `Dismissed` with its reason, on the authentication channel,
   and disconnected `DISMISS_DELAY` (500 ms) later (`Dismissals`), so that the message arrives
   first: the client ends its session on it, with the reason on the error card. Until then the
   server ignores what it sends. A name is taken, banned or an operator's whatever its case.
   In hub mode the lists hold `Entry`s, an account (`id`, `name`) or a name alone, and a
   player is compared by account when the entry has one; `Permissions::identify` rewrites a
   name entry with the account and the name of the player it matches when it joins. The
   account of a name given to `/op`, `/ban`, `/pardon`, `/whitelist` (`name_to_resolve`: not
   a name that a list to remove from holds) is the one of the player online, or comes from
   `GET /v1/profiles?name=` in a thread (`network/profiles.rs`, `ProfileLookups`, polled by
   `finish_lookups_system`, which runs the command and answers its author); `--op` asks it
   at start (`hub::resolve_names`). Tests replace the question with `set_profile_lookup`.
   A hosted world (`--realm`, `init_hosted`) has `Permissions::hosted`: the members of
   `GET /v1/realms/{id}/config` (`hub::watch_realm`, read at start and every minute by a thread
   that sends `RealmEvent`s) as the whitelist, its operator members and its owner as the
   operators, nothing read from or written to files, and `Permissions::refuses` answers the
   commands that change the lists. `realm_system` applies each new config, dismisses the
   players it no longer lets in and, on `Gone` (401 or 404), saves and exits.
   `ServerWorldMap::accounts` gives the files of the players their account for name.
   Joining starts with a challenge: as soon as a client connects, `send_challenges_system`
   (`network/join.rs`, in `PreUpdate`) sends it `AuthChallenge`, a nonce of its own, the
   server's key and the hub of a server in hub mode, and the client answers with its
   `AuthRegisterRequest`. A client that has not joined 5 s after connecting (`AuthTimeout`) is
   dismissed, in every mode. In hub mode (`ServerAuth::Hub`, `--auth hub`) the request carries
   a `JoinProof`: the certificate of the hub, the address the player dialed (normalized,
   `normalize_address`) and the signature of the device over the nonce, the server key
   (`server_key.ron`, hub mode only) and that address. `check_proof` verifies the certificate
   against the hub keys read at start (`hub.rs`, cached in `hub_keys.ron`) and its `hub`
   against the hub's URL, refuses those revoked (`/v1/revoked`, read every hour by a thread,
   every 5 minutes after a failure; `dismiss_revoked_players_system` dismisses the players
   online whose certificate is revoked since), a proof made for an address not among the
   server's `--public-address` (a server relaying the challenge of another), and the client
   whose announced id is not the account's (`identity::player_id`), each with its reason. The
   player then takes the name of its certificate, and only one connection per account is let
   in. In offline mode the proof is ignored; a solo world is always offline.
2. **Simulate**: the inputs are applied to the world. Chunks are loaded within the reach of
   the players (`ChunkReach`, see the broadcast: their render distance, then around the twins
   of the portals they see) before the inputs, in the background (`ChunkLoader`): a tick
   inserts the chunks made since the last one and asks for the missing ones, nearest first,
   at most `CHUNKS_IN_FLIGHT` (8192) at once, which the server's own pool, of up to 16
   threads (`world/pool.rs`), reads or generates between the ticks. A chunk next to an input
   is made at once, on the tick. The generator computes the columns of the terrain once per
   column of chunks (`ColumnsCache`) and samples the noises of the caves on a coarse grid it
   interpolates (`world/caves.rs`). While no player changes chunk and no portal changes, the
   chunks around them are not looked at again (`LoadedAround`). A chunk is read from its file in
   the world's folder the first time a player needs it, else generated: opening a world reads
   no chunk. Then every furnace of the loaded chunks ticks once (`world/furnaces.rs`), one in a
   chunk that is not loaded waiting, as in Minecraft; a burning furnace keeps its chunk to be
   saved. Then the blocks get their ticks (`world/ticks.rs`, the one system of the blocks that
   change by themselves): 4 random cells of each loaded chunk within `SIMULATION_DISTANCE`
   (4 chunks) of a player (`chunks_near_players`), but the chunks of air, get a random tick,
   what the shared rules make of them (`shared/src/world/ticks.rs`, `random_tick`): crops step
   up a stage one tick in 4, in 2 when the rain falls on them or water lies within 4 blocks of
   their tilled soil, berries ripen, the rain fills the cisterns and puts the campfires out
   (`shared/src/world/growth.rs`), and a sapling grows into the tree the server's generator
   shapes; fires get theirs on their own schedule instead, every 30 to 40 ticks, at
   most 64 a tick (`shared/src/world/fire.rs`: they age, spread to and burn what is flammable,
   and go out in the rain or without support); lava may set fire to the air by a flammable
   block near it (`shared/src/world/lava.rs`). Then the liquids flow (`world/flow.rs`): every
   5 ticks for water and 30 for lava (`Liquid::ticks_per_step`), the cells next to a block that
   changed take the level their neighbours give them, lava 2 levels down a cell so that it
   reaches 3 cells, and lava touched by water hardens into obsidian (a source) or cobblestone
   (`shared/src/world/liquid.rs`). A generated chunk changes no cell, so the lava lakes that
   fill the caves up to y = 10 (`LAVA_LEVEL`) are not scheduled until something touches them.
   Then the weather counts down (`world/weather.rs`): a clear
   spell of 10 to 100 minutes is followed by rain (2 in 3) or a thunderstorm (1 in 3) of 5 to
   15 minutes, then by a clear spell again (`shared/src/world/weather.rs`), and `/weather`
   sets one for a spell of the same length. A change is sent to the authenticated clients
   (`WeatherSet`), and the answer to the authentication carries the weather of the moment.
   During a thunderstorm a strike falls every 3 to 15 seconds, on the top block of a column
   16 to 160 blocks from a player drawn at random, and is sent with a seed that shapes its
   bolt (`LightningStrike`) to the clients within 1,500 blocks of it (`LIGHTNING_REACH`),
   beyond which it is neither seen nor heard; a column whose top is not loaded gets none.
   Last, the portals whose wall or front changed and no longer fits go.
3. **Broadcast**: block changes, new chunks and mobs are sent to each client, and the
   shots of the portal gun to the other players, then the portals when they changed.

   Each client gets the chunks within the render distance of its own player along x and z,
   and within at most `MAX_VERTICAL_RENDER_DISTANCE` (8) of it along y, as the ground lies
   within a few chunks of height (`within_render_distance`): nearest first
   (`render_distance_offsets`) and at most 1024 per tick, not counting the chunks of one kind
   of cell, in messages of 32 chunks encoded in parallel. A mob is sent (`MobUpdate`) when it
   comes within the render distance of a player, or into the chunks around the twin of a
   portal it sees, or changes, and `MobRemoved` when it leaves them or the world (`sent_mobs`
   holds what each client was sent).
   The player chooses it, from 1 to `MAX_CHUNK_RENDER_DISTANCE_RADIUS` (64), in the Video
   tab of the settings or with the O and P keys (`RenderDistance`, saved in `video.ron`): the
   authentication request carries it and `SetRenderDistance` changes it. The answer to the
   authentication carries the server's maximum (`ServerRenderDistanceLimit`), which the Video
   tab names in game when it is below the choice; a world of Singleplayer grants 64. The server keeps it between
   `MIN_RENDER_DISTANCE` (1) and its `--max-render-distance` (`DEFAULT_MAX_RENDER_DISTANCE`,
   64), in `RenderDistances`. `sent_chunks` records what each client has. A chunk more than
   the render distance plus `UNLOAD_MARGIN` away is unloaded, after a move as after a lowered
   render distance: the server forgets it sent it and tells the client in a
   `ChunkUnloads`, which removes it and despawns its meshes. A chunk of the margin is kept but
   never sent first, so that walking to and fro across a chunk border neither unloads nor
   sends anything. The server decides, as the client's predicted position runs ahead of the
   server's: a client that unloaded on its own could drop a chunk the server believes sent,
   and never get it back. `ChunkUnloads`, `BlockUpdates` and `WorldUpdate` share one ordered
   channel and go in that order, so that no update is sent for a chunk being unloaded.

   A client also gets the chunks within `PORTAL_VIEW_RADIUS` (4) of the twin of every open
   portal within its render distance, which the portal shows, after its own: one rule in
   `shared/src/world/reach.rs` (`ChunkReach`) says what a client is sent and keeps, and the
   server unloads, loads and forgets chunks by it. The chunk of a portal is that of the cells
   it opens onto. They take at most a quarter of the 1024 chunks of a tick, the player's own
   the rest, so that neither starves, and come after the player's own on the wire. With the
   margin, they are kept while the portal is within the render distance plus the margin, and
   unloaded once it is beyond, closed or moved: the reach, portals included, is the key of
   `SentChunks::progress`. A portal the player approaches has the twin's whole render distance
   sent instead (`TwinArea`), so that after crossing the player stands in chunks it already
   holds: about 4 900 chunks at a render distance of 8, 700 KiB over 7 broadcasts. A portal is
   approached 2 s before the player can reach it (`approach_radius`, from the speed of its
   stance, or its velocity past 12 blocks a second, which a jump never reaches): within 2
   chunks on foot, 6 in a flying sprint, 8 at most. With no portal within 8 chunks the
   approach is always 2, so that it never changes the reach, which keys what is walked again.
   The areas
   approached add at most `MAX_APPROACHED_EXTRA_CHUNKS` (10 625, a render distance of 12)
   chunks together to the player's own, the nearest portal first: past a render distance of 12
   an area keeps as many shells as fit, rounded down to a multiple of 4 so that moving near a
   twin seldom changes it (`CAPPED_RADIUS_STEP`), and kept a step wider, so that a step back
   across a chunk border unloads none of it; the client's fog follows what it holds.
   The chunks of an area go by shells, nearest the
   twin first and those behind its plane after (`twin_area_offsets`), merged across the areas
   (`ChunkReach::portal_chunks`, kept by `SentChunks::portal_progress` while the portals do not
   change, so that a broadcast goes on from where the last stopped); left past the widest
   approach (8) and the margin, the area shrinks back to the view radius, so that releasing a
   sprint or landing unloads nothing. The sorted offsets are kept for the 16 (radius, facing)
   last used (`twin_area_offsets`). A reach holds only the open portals within its render
   distance plus the widest margin it is asked about (`MAX_REACH_MARGIN`), so that a pair
   opened far off leaves every reach, and what it keys, as it was. The loader lists those
   chunks once per reach too (`LoadedAround::behind_portals`), each tested only against the
   reaches near its twins: 1.1 ms for 8 players each by a portal at a render distance of 8,
   each time one comes into another chunk (`bench_portal_players`).
4. **Save**: on request or every 60 seconds, the metadata, the players and the changed chunks
   are handed to the writer thread. A generated chunk is saved too, but not one of a single
   kind of cell, which is generated again at once. Then, every 5 seconds, the chunks more than
   `KEPT_MARGIN` beyond the reach of every player (`ChunkReach::kept`) are forgotten
   (`world/unload.rs`), but those
   not saved yet, and none while the writer has saves to write: their file could be read back
   before it is written. A mob over chunks that are not loaded stays where it is.

The animals are born with the chunks (`world/generation/animals.rs`): each column of chunks
may hold a herd, decided by its position and the biome of its centre, which belongs to the
chunk holding the surface there. That chunk is saved once generated, so the herd is never
born again. A spawn egg used on a block brings its mob there too (`place_block`).

The animals breed (`mob/breeding.rs`). A right click on an animal with its food in hand
(`MobKind::eats`: rye, carrots, potatoes, berries or meat by kind) feeds it, one item every
half second the button is held, rather than the player eating it (`combat::interact`). A fed
adult wants a mate for 30 s (`ServerMob::in_love`); `breed`, after the moves, walks two that
want one within 8 blocks to each other and, once 1.5 blocks apart, adds a young one between
them (`ServerMob::young`, half the size, `growing` for 10 minutes, then `grow_up`), and the
parents wait 5 minutes before they breed again. A young one fed grows up a tenth of its time
sooner, is not shorn and leaves no loot. A wolf or a cat fed its meat is tamed one time in
three (`TAME_CHANCE`): its `owner` is the player's name, as the ids change with each
connection. A tamed one runs to its owner farther than 6 blocks until 2.5 away, is brought to
its feet beyond 24 (`behavior::follow_owner`), and only tamed ones of the same owner breed,
their young belonging to them. The client draws a young one smaller (`MobModel`) and lets
hearts rise over one that wants a mate, and burst over one as it is tamed (`mob/hearts.rs`).

The villages (`world/generation/village/`) are built where `WorldGenSettings::villages` is set,
which a new world has and a world saved before them has not. The world is cut into regions of
512 blocks, each holding at most one village, whose centre is drawn from the seed and the
region and kept where its biome has a `VillageStyle`, deep inside it, on dry, rather flat land
(`placement.rs`). Its plan, a square around a well, streets and buildings along them, is laid
out from the seed and `TerrainNoises` alone (`layout.rs`, the buildings in `pieces.rs`), cached
by region in the `WorldGenerator`, and each chunk it touches places the blocks of the plan that
fall in it after the terrain and the plants: a village is the same whichever of its chunks is
generated first. Nothing grows on its columns (`Column::village`), and the caves stay under
them. The chests and barrels of a generated chunk come filled (`loot.rs`), and the animals of
its pens are born with it, like the herds.

The dungeons (`world/generation/dungeon/`) are built likewise where `WorldGenSettings::dungeons`
is set: a ruin with a cellar on flat land, a crypt off a cave (y 20 to 50), a sanctuary off a
cave by the lava (y 9 to 17), at most one of each per region of 256 blocks. `placement.rs` draws
a few places in the region and keeps the first that suits: a ruin reads the terrain and keeps
away from the village; the others carve one chunk (`Caves::carve`) and put their door in a
wall of the cave. `blueprints.rs` holds the layers of each, `loot.rs` its table (a new item goes
at the end). Their blocks are placed after the ores; `structure.rs` holds what they share with
the villages. The region `lair_region` holds no sanctuary, for the lair of a boss to come. Their
hostiles are not born with the chunks, which load beyond where a hostile despawns:
`dungeon_guards_system` (`mob/guards.rs`), every 40 ticks, 1 time in 4, adds one at a place of
the plan of each dungeon within 36 blocks of a player, up to 5 in it and within the caps of the
hostiles, out of the sight of the players and 8 blocks from them. A torch, lantern or lit
campfire within 3 cells of a place keeps it empty; a peaceful world has none.

The systems of the server run one after the other on its thread: an integrated server would
otherwise share the compute pool with its client, whose frames would wait for its ticks.

The mobs and the dropped items move first, before the inputs of the tick are read. The mobs are
then pushed out of the players and of one another (`push_mobs`): only the server does it, and it
never moves a player, so the prediction of the clients is left alone.

### The systems of a tick, in order

Before **Receive**: `villager_day_system` (night sends the villagers home), `mob_behavior_system` (the animals wander, flee, follow their owner and
breed, the hostiles hunt and strike, `push_mobs`), `cinder_system` (the embers fly), `arrow_system` (the arrows fly), `fishing_system` (the floats of the rods), `hostile_spawning_system`,
`wildlife_system` (the herds coming back), `dungeon_guards_system`, and
`dropped_items_physics_system`. In **Simulate**, chained: `tick_damage_timers_system` (the
invulnerability and the memory of the last mob hit count down), `handle_player_inputs_system`
(movement, blocks, blows, eating; a fall hurts here), `boss_system`, `breathe_system`, `burn_system` (lava
first, then fire), `tick_furnaces_system`, `metabolism_system` (every 80 ticks: regeneration or
starvation), `trade_system`, `drop_on_death_system`, `pickup_dropped_items_system`, `announce_depths_system`, `home_system` (bedrolls and
signs), `block_ticks_system` (the one random tick, above), `liquid_flow_system`,
`weather_system`, `remove_unfit_portals_system`. Every source of damage of a tick, the mobs'
included, comes before `drop_on_death_system`, and the pickups after it.

### Damage, death and respawn

Every hurt goes through `Player::hurt` (above), which only survival takes. A dead player is one
with no health. `drop_on_death_system` (`world/dropped_items.rs`, `DroppedItems::drop_on_death`)
empties the inventory of a dead survival player where it died, its slots, crafting grid and
cursor stack, each stack popping in a random direction as from a broken block, and closes its
table, chest or furnace; creative keeps everything. A dead player picks nothing up, so its
items lie there for the usual 5 minutes (`LIFETIME_TICKS`). `Respawn` (`handlers::respawn`)
brings it back at its bedroll while the bedroll stands (`home::respawn_position`), else at the
spawn, in full health and fed (food 20, saturation 5), breathing, put out and no longer
dragged (`Player::respawn`).

### Armour

`players/armour.rs` holds it: `ItemId::armour()` gives the `ArmourPiece` and `ArmourMaterial` of an
item, as `tool()` does a tool's. The pieces worn are stacks of the inventory in the slots 75 to 78
(`armour_slots`, helmet first), so they are predicted, saved, sent to their player and dropped at
death with the rest, and a save without them loads; `takes` refuses any other piece in a slot, and a
quick move puts one on when nothing is open. `Player::hurt` gives the hit to `Player::absorb`: each
point of armour absorbs 4 % (a suit is 7 points in leather, 11 in bronze, 15 in iron, 19 in diamond,
20 at most count) and each piece worn wears by one, then breaks. Health going by whole points, what
is left is counted in hundredths and the rest carried to the next hit (`armour_carry`).
`DamageSource::is_absorbed_by_armour` says which hits it stops: the mobs', embers included, the arrows, fire and
lava; not drowning, hunger, the void nor falls (armour does not soften a landing). Protection
(Enchantments, below) adds to what `absorb` takes, 90 % at most. `PlayerUpdateEvent::worn` (`Inventory::worn`) tells the
others what a player wears, and the client's `player/armour.rs` puts it on the model: boxes around
the parts of the body, children of them, cut from `graphics/armour/<material>.png`, laid out as a
skin with the leggings in the second layer. `scripts/generate_armour.py` draws every texture of the
armour. The bar (`ui/hud/armour.rs`) shows above the hearts.

### Combat

A left click on a mob within reach (3 blocks, 5 with the spear) is a blow
(`server/src/mob/combat.rs`, `interact`): `MobKind::blow_damage(held)` (the weapon's
`attack_damage`, 1 by hand, half again with an axe on a Stumpling), 10 ticks of invulnerability
(`ServerMob.hurt_ticks`), a knock back of 6 blocks/s away from the player, and the wear of the
weapon (1 for a sword, 2 for another tool). A hit animal flees for 5 s; a hit hostile takes the
player as its prey (`hostile::provoke`). A killed mob is removed and drops its loot
(`combat::loot`) as dropped items. The client tints a hurt mob red and plays its hurt sound when
`ServerMob::damage` rises in a `MobUpdate`.

The blows have a rhythm (`shared/src/players/melee.rs`). Each `WeaponClass` recharges in its own
time (hand 0.4 s, sword 0.5 s, pickaxe, shovel or hoe 0.65 s, spear 0.75 s, axe 1 s); a
`Recharge` counts it from the inputs alone: every press of the left button is a swing, whatever
it meets, and so is a full recharge while it stays down; changing the item held starts it again.
The server keeps one per player (`ServerWorldMap::recharges`) and is the only judge of the damage:
`melee::blow_damage` gives a fifth of the weapon's damage to a hurried blow and all of it to a
full one (0.2 + 0.8 × charge²), half as much again for a critical one (`is_critical`: full, while
falling, neither flying nor swimming). A full blow of a sword on the ground sweeps (`sweeps`,
`in_sweep`): the other mobs within 2 blocks of its target and in front of the player take a third
of its damage, but the tamed animals and the villagers. `ServerMob::struck` tells the clients what
made the last blow special; `MobHurt` carries it, for the sparks (`mob/strikes.rs`) and the sounds
(`Sound::CriticalHit`, `Sound::Sweep`). The client counts the same `Recharge` from its own inputs
for the gauge under the reticle (`ui/hud/recharge.rs`). The mace (`WeaponClass::Mace`, 1.4 s) is
the slowest and throws back 2.2 times as far (`WeaponClass::knock_back`).

### Shields, crossbows, throwing axes

The shield (`shared/src/players/shield.rs`) is raised by holding the right button, from the
selected slot or, as in the other hand, from anywhere in the hotbar while the hand holds a melee
weapon or nothing (`shield_slot`). `raise_shield` runs with `draw_bow` in the movement: the raise
(`Player::shield_ms`, up after 250 ms) and the time a knocked-down shield stays down
(`shield_disabled_ms`, 5 s of input) are part of the `MovementState`, as the shield halves the speed
and stops the sprint. Every source of a hit asks `Player::parry` first with where the hit comes from:
a raised shield parries what comes from the front half (`shield::in_front`), wears by the damage, and
counts `Player::parries`, which the clients hear (`Sound::ShieldBlock`). The hostiles' blows
(`hostile::strike_player`), the embers, the arrows and the Ember Mother's blow go through it; the
Roller's charge and the Ember Mother knock it down. A parried arrow falls as an item. The
Stumpling parries the player's blows from its front with its bark (`MobKind::guards`,
`ServerMob::parries`, heard the same); an axe (`WeaponClass::breaks_shields`) knocks it aside for
4 s (`ServerMob::guard_down`) and lands.

`players/bow.rs` draws three weapons (`full_draw_ms`): the bow, the crossbow (1.25 s to load, it
shoots only once loaded, a bolt of 11 points that flies 1.3 times as fast and pierces two creatures:
`BowShot::pierce`, `ArrowFlight::pierce_left`; a piercing bolt goes through what it already hurt)
and the throwing axe (thrown out of its stack of 16, it falls back as an item where it ends). All
three fly as `MobKind::Arrow`; `BowShot::ammo` says what is picked up again.

### Status effects

`shared/src/players/effects.rs` is the generic system the potions will use: an `EffectKind`
(poison, burn, slowness, weakness, regeneration), a level and the ticks left (`Effect`), one at most
of each kind in `Player::effects` (`StatusEffects::give`: the higher level wins, at the same level
the longer time). The server gives them (`Player::give_effect`, never in creative) and counts them
down once a tick (`status_effects_system`, `Player::tick_effects`): poison takes a point every 25
ticks (halved by level) but never the last, burn every 20 ticks and may kill (`DamageSource::Fire`),
regeneration heals every 50; slowness sets `Player::slowness`, part of the `MovementState`
(15 % per level), and weakness takes 2 points per level from a blow (`StatusEffects::weakened`).
They are saved with the player, sent in `PlayerUpdateEvent::effects` and shown at the top left of
the HUD (`ui/hud/effects.rs`). The Drowner's grip slows, the Bramble's thorns poison, the Gnawer's
bite weakens (`hostile::inflicted_effect`), an ember burns. A bucket of milk, from a cow with a
bucket, takes them all away (`blocks::drink_milk`).

### Hostile mobs

The rules are shared (`shared/src/world/hostiles.rs`): `MobKind::hunt` (sight, forget distance,
speed) and `strike` (damage, reach, cooldown, knock back), `is_prey` (a living survival player),
`line_of_sight`, `is_dark`, `spawn_kind` and `pack_size`. `hostile_spawning_system`
(`mob/spawning.rs`) removes every hostile and ember of a peaceful world (`/difficulty`, saved in
`world.ron`; `Difficulty::strike_damage` scales the blows and the embers in an easy or a hard
one); otherwise, every 40 ticks, it despawns the hostiles 96 blocks from every player
and gives each player one attempt: a spot 24 to 64 blocks away horizontally, at a random height
within 24 of its feet, scanned down for water or a floor with two free cells; if it is far
enough from every player, dark ((night and open sky) or covered, and no light emitter in the
17³ cube around it), its place picks the kind: Gnawers (packs of 2 to 4, 3 in 4) or a Roller in
the caves below y = 50, a Cinderling 1 in 3 below y = 20, Rollers above y = 100, a Gust Owl 1 in 2
at night under the sky above y = 110, Stumplings on night grass and Brambles or Stumplings in
the woods, a Drowner in water 3 deep or more (1 in 4 by day, 1 in 2 at night, no darkness
needed). Caps (more in the deep tiers, below): 10 hostiles per player within 64 blocks (a spawned hostile is always within
them), 40 in the world; the embers do not count. At dawn the Stumplings burrow into the ground
and go.

A hostile (`mob/hostile.rs`, `act`) keeps its prey until it is out of the forget distance,
dead or in creative, else finds the nearest prey within its sight and in sight, then acts by
kind: the Gnawer and the Stumpling walk to it along the way they find (`hostile::chase`,
`mob/path.rs`) and strike in reach (the Stumpling flees fire and lava within 6 blocks). Their
way is an A* over the cells their feet may stand in: round walls, up a step of one block, down
a drop of 3 blocks plus one per point of health left beyond the first (6 at most), never into
water, lava or fire. A search expands at most 400 nodes within 24 blocks, all of one tick 1600
(`path::Pathing`), so that those over budget keep their way, or walk straight, one tick more;
it runs again every 20 ticks, when the prey moved 2 blocks or the mob strayed 3 blocks from it.
Short of the prey it leads to the nearest cell met. The Stumpling opens the closed doors of
its way (`path::open_doors`, once all mobs stepped) and walks through open doors; the Bramble never moves and bites who comes within 2.5 blocks;
the Roller winds up 20 ticks, then rolls at 7 blocks/s until it hits the player (4 damage, a
knock back of 8) or a block (stunned 40 ticks); the Drowner swims in 3D, never leaves the water
to chase, and its grab drags the player down (`Player::drag`, part of the `MovementState`); the
Cinderling hovers 2 blocks above the ground at 6 blocks from its prey and throws embers; the
Gust Owl circles 8 blocks over its prey and dives at it, a deadly fall within 3 s being put on
it. A strike shows as `MobAction::Attack` for 10 ticks, which the client plays once
(`HOSTILE_CLIPS`); the fliers and the Drowner in water skip gravity (`behavior::floats`).

Every other mob in water floats (`mob/float.rs`): the water eases its vertical velocity toward the
one that brings it to its waterline, 60% of its height under water, and leaves it still there. It
goes at `WATER_SPEED_MULTIPLIER`, and a block on its way at most a block above the surface is
leapt on (`climb_out`). Afloat with nothing else to do, it walks to the nearest shore within 16
blocks (`swim_ashore`); an animal on land never wanders across a liquid.

### Villagers and trade

`MobKind::{Farmer, Smith, Cartographer}` (`is_villager`) are born with the chunks of a village,
one per house (`Piece::home`): the villages of a world already generated get none. They are
saved with the mobs, with what they remember (`ServerMob::villager`: their home and what they
sold), never despawn, are never hurt, and the hostiles ignore them. By day they walk about their
home, at night they go into it, along routes of cells (`server/src/mob/villager.rs`,
`find_route`, A* over a bounded number of cells); no door stops them (`collides_at`). One from a
spawn egg lives where it first stood. Each trade has a voice of its own, heard now and then and as a trade opens.

The offers are a table both sides know (`shared/src/world/trade.rs`), against `Coin`s, which
players earn by selling; the stock of each offer is renewed at sunrise (`market_day`). A press
of the right button on a villager opens a trade (`combat::interact` into `Trades`). `trade_system`
checks and makes the trades asked (`Trade`), sends `TradeUpdate` with what is left when it
changes, holds the villagers trading, and ends the trade of a player farther than
`TRADE_DISTANCE`, dead or gone. The client predicts no trade: the inventory of the next
`PlayerUpdate` shows it. It draws the offers in the inventory panel
(`client/src/ui/hud/inventory/trade.rs`), and sends `EndTrade` when the panel closes. The
directions a cartographer sells are searched on the generation pool
(`Locator::start_directions`) and come in the chat; a world without dungeons sells none.

### The ember

An ember is a mob kind (`MobKind::Ember`, `is_projectile`) that the server moves itself
(`mob/cinder.rs`, `cinder_system`): exact ballistics under −16 blocks/s², stepped by at most a
quarter of a block, for at most 100 ticks. The first player box it touches takes 1 damage and
burns for at least 60 ticks; the first cell with a hitbox or a liquid ends it, and lights the air
cell it came from when that block is flammable. Embers are neither pushed, struck, counted in
the caps nor saved. A Cinderling that touches water, or sees the sky in the rain, goes out
without loot.

### The bow

`players/bow.rs` (shared) draws the bow while the right button is down: `draw_bow`, part of
`simulate_player_movement` so that a client replays it, counts `MovementState::bow_draw_ms`,
which slows the player, stops its sprint and, being sent, shows the others the draw. `shoot_bow`,
at the release, spends an arrow of the 36 slots and wears the bow (not in creative), and returns
a `BowShot`: its power, its damage (bow power adds to it) and whether the
arrow may be picked up. Only the server turns it into a mob, `MobKind::Arrow`, a projectile as the
ember, with its `ArrowFlight` (neither sent nor saved). `arrow_system` (`mob/arrow.rs`) flies it
in a curve, tested every tenth of a block: it hits the players in survival
(`DamageSource::Arrow`, absorbed by armour; the death names the shooter, `DeathCause::Shot`) and
the mobs (`combat::wound`, shared with the blows), but its shooter for its first 5 ticks. In a
block it sticks, `MobAction::Idle`, falls again if the block goes, and goes as a dropped item to
a player with room who touches it, or after 3 minutes. The pictures of the drawn bow,
`Bow_Draw1` to `Bow_Draw3`, are told by `bow::draw_stage` (`ItemHolder::draw`).

### Experience

`players/experience.rs` (shared) holds `Player::experience`, an `Experience`: a level and the points
toward the next, which costs `level_cost(level)`, 8 points and 3 more per level. Only the server
changes it; every `PlayerUpdateEvent` sends it and the save of the player keeps it. The engraving
table asks `Player::has_levels` and takes `Player::spend_levels`. An orb is a `DroppedItem` whose
`experience` is above 0 (its stack means nothing): it moves, lasts, is saved and sent as an item,
merges with any orb within 1 block, flies to the nearest living player within 6
(`DroppedItems::attract_orbs`) and gives its points on contact. Orbs come from the hostiles the
players kill (`MobKind::experience`), from the ores that drop their resource, coal and diamond, when
harvested (`BlockId::experience`; an ore that drops itself would be placed and mined again for ever)
and, at the player, from the stacks taken out of a furnace (`InventoryActionOutcome::smelted`,
`smelting_experience` in hundredths of a point). Death takes all the experience and leaves an orb of
half of it, 400 points at most (`Experience::lose_at_death`). The bar (`ui/hud/experience.rs`) sits
right over the hotbar, under the hearts and the drumsticks.

### Enchantments

`world/enchantments.rs` (shared) is their one table: what takes each `Enchantment`, its three levels,
their cost (3, 5 then 8 levels of experience) and what they do: efficiency +50 % mining speed a level,
durability +50 % durability, sharpness and bow power +1 damage, protection +1 % absorbed by the piece.
They are part of the type of the stack, `ItemType::Enchanted`, as its wear: what moves, saves, drops or
sends a stack keeps them, and older saves and chest files, which hold the other variants, load as they
are. What carries them stacks to 1, so nothing merges. Read the wear and the durability of a stack with
`ItemType::wear` and `ItemStack::durability`, not by matching `Tool`.

The engraving table (`BlockId::EngravingTable`) opens the inventory panel with a slot,
`ENGRAVING_SLOT`, kept in the inventory as the crafting grid and given back at the close.
`InventoryAction::Enchant` raises an enchantment of the item there (`players/enchanting.rs`): the
server alone applies it, free in creative, and the next `PlayerUpdate` shows it. The client lists the
enchantments in `ui/hud/inventory/engraving.rs`; an enchanted item shows a tinted icon, a gem in its
slot (`SlotGem`) and a line each in its tooltip. In the world, in a hand or on the ground, it glints
(`dropped_items/glint.rs`): a copy of its mesh over it, added to its colours in a teal that breathes
and drifts to violet, whose material is shared by every glint of a texture and coloured anew each
frame. `Inventory::in_hand` gives the item held with whether it is enchanted (`Held`), so that the
others see the glint too.

### The anvil

`world/anvil.rs` (shared) is what it does: `forge` takes the item of its first slot, which must wear,
what its second slot holds and the name asked for, and gives a `Forging` (the item made, its cost in
levels, what it takes from the second slot) or a `ForgeRefusal`. A unit of the material of the item
(`repair_material`: planks, cobble, an ingot, a diamond, leather, rope) repairs a quarter of its
durability, for a level each; another item of its kind merges with it, their durabilities added with
12 % more and their enchantments combined, two of the same level making the next, for 2 levels and
those of the enchantments it brings; a name costs a level, an empty one gives back the name of the
kind. Each repair or merge counts on the item, and adds 1, 3, 7, 15... levels to the next work; beyond
39 levels it is too dear. The count and the name are in a variant of the type of the stack,
`ItemType::Forged`, built by `ItemType::worked`, which keeps the older variants for what was never
worked; `ItemName` holds the name inline, up to 24 characters, so that a stack stays `Copy`, and saves
and sends it as a string.

The anvil (`BlockId::Anvil`, from three blocks and four ingots of iron) opens the inventory panel
with `ANVIL_LEFT_SLOT` and `ANVIL_RIGHT_SLOT`, kept in the inventory as the crafting grid.
`InventoryAction::Forge` carries the name typed (`players/anvil.rs`): the server alone applies it,
free in creative. `ui/hud/inventory/anvil.rs` shows the name field, filled with the name of the item
put in, what the work does and costs, and the button.

### Nothing is lost

What a player gets and has no room for falls at its feet (`DroppedItems::drop_from_player`) instead of
vanishing: what a block it uses gives (`ActionsOutcome::dropped`), what a close puts back from the
cursor, the crafting grid, the engraving table and the anvil (`InventoryActionOutcome::dropped`), and
what `/give` gives, a backpack of it at most. Crafting into the inventory and trades are refused
without room, and an item on the ground stays there until there is.

### Mounts

`players/riding.rs` (shared) is the ride: `Player::riding` (`Riding`: the mob, its kind, the seat,
the heading, and for a horse its speed, jump, breath and the charge of its jump) is part of the
`MovementState`, so the client predicts a ride as it predicts a walk, the server repeats it, and the
others see the rider seated. While it is set, `simulate_player_movement` hands the input to
`simulate_riding`: the sneak key gets off (`dismount`, beside the mount where there is room); the
driver moves the mount's box, steered by where it looks, and sits on it (`seat_offset`). A horse walks
at its speed, gallops `GALLOP` times faster with the sprint key while its breath lasts (winded, it
waits for a third of it back), steps up a block, and jumps when the jump key, held on the ground, is
let go, as high as it was held long (`CHARGE_MS`); a fall is cushioned by 3 blocks. A boat floats,
glides to its speed, 8 blocks/s on the water and 1 on land. The passenger of a boat (seat 1) does not
move by itself.

`server/src/mob/riding.rs` puts a player on a mount when it right-clicks it (`mount`, from
`combat::interact`): a boat on a free seat, a horse grown up, tamed and saddled by that player
(`free_seat`). After the inputs of each tick, `riding_system` puts each driven mob where its driver's
simulation says, carries each passenger with its boat, and takes off the riders whose mount is gone
or who died; `ServerMob::riders`, neither saved nor sent, is rebuilt from them. A driven mob has no
behaviour of its own and is not pushed, and a rider pushes nothing. On the client, the mount of the
local driver is drawn under its prediction (`place_ridden_mount`), and `ui/hud/mount.rs` writes the
line over the hotbar.

A horse is tamed by its food like the wolf (`MobKind::is_tameable`), but does not follow its owner
(`follows_owner`); a hurt tamed horse heals by eating. Each horse has `HorseTraits` (`world/mobs.rs`):
a `Coat` whose base gives its speed and endurance, varied at random, and a jump; `add_mob` rolls
them, and a foal inherits the mean of its parents' with a little drift. The coat picks the model,
`Horse<Coat>.glb`, of the same parts as `Horse.glb` whose clips move them; its `saddle` part shows
when the horse is saddled (`ServerMob::saddled`, which a saddle item sets). The boat is the mob
`MobKind::Boat` (`is_vehicle`: no behaviour, no voice, it floats), put down from `ItemId::Boat` as a
spawn egg aimed like a bucket, on the water, and dropping its item when broken.

### Companions and steeds

`server/src/mob/companions.rs` keeps the tamed animals beside their owners. A right click of its
owner sits a wolf or a cat, or stands it up (`toggle_sit`, `ServerMob::sitting`, saved and sent: the
client tilts the model back, `model_transform`). A follower runs to its owner along the way the
hostiles' path finder gives (`hostile::chase_to`, the budget of the tick shared), and is brought next
to it (`behavior::beside`) beyond `FOLLOW_TELEPORT` when the owner stands in loaded terrain. A wolf
hunts (`ServerMob::prey`, neither saved nor sent) what its owner struck (`rally`, from
`combat::interact`) and the hostiles that go at its owner (`guard`), never a player nor a tamed
animal (`may_hunt`); `mob_behavior_system` surveys the mobs first (`Pack`: the tamed cats, the box
of each mob), the wolves run at their prey (`hunt`) and bite in reach, and the bites land once every
mob moved (`apply_bites`): `WOLF_BITE` points, an animal flees, a hostile that strikes in reach
strikes back. A tamed wolf heals by eating meat (`breeding::feed`). The Gnawer and the Gust Owl
(`fears_cats`) keep `CAT_FEAR_DISTANCE` from a tamed cat (`hostile::shun_cats`). The steeds
(`MobKind::is_steed`: the horse, the donkey and the mule) are ridden alike (`Riding::steed`); the
blows of the hostiles at a rider fall half on its steed (`Player::mount_blows`,
`bear_mount_blows`), lessened by its horse armour (`ItemId::horse_armour`, `armoured_damage`: 4 % a
point), as are the blows of the players and the wolves (`take_blow`). The models of the steeds have
`saddle`, `armour_<material>` and `chest` parts, shown from the mob (`shows_part`).

A horse and a donkey have a mule (`breeding::young_kind`), which does not breed. A chest put on a
tamed donkey or mule (`ServerMob::pack`, a `Container`, saved and sent) is opened sneaking with a
right click (`InventoryAction::OpenPack`, `Inventory::open_pack`): `server/src/mob/pack.rs` lends it
to `ChunkMap::packs` while the server simulates the inputs of its owner within `PACK_REACH`, which
the inventory uses as an open chest, and takes it back after; the update of the player carries it
in `open_chest`, and the client keeps it in its own `ChunkMap::packs`. What it holds drops where the
mob dies.

`server/src/mob/leash.rs` handles the leads (`ServerMob::leash`, `Leash::Player` by name or
`Leash::Post` by cell, saved and sent). `combat::interact` ties an animal to the hand with a lead
and takes the lead back; `tie_to_post`, after it, ties the animals a player leads to the fence post
it right-clicks. An animal farther than `LEASH_SLACK` from what holds its lead walks back to it
(`pull`, before anything else it does), and `leash_system` drops the lead of one pulled beyond
`LEASH_BREAK`, of a player gone, or whose post was broken. The client draws the leads
(`mob/leads.rs`).

### Fishing, cooking and the farm

`world/fishing.rs` (shared) holds the rules of fishing: the waits, the bite, the line, and what a
catch is (`catch`: 84 % fish, 11 % junk, 5 % treasure; the glowfin bites at night only). The float
is the mob `MobKind::Bobber`, a projectile, which `server/src/mob/fishing.rs` flies: `use_rod`, from
`handle_player_inputs_system` after `combat::interact`, casts one at a press of the right button
with a rod in hand (the client's `simulate_player_block_interactions` leaves the rod alone) and
draws it in at the next press, `ServerWorldMap::fishing` (`Lines`) keeping the float of each
angler. `fishing_system` flies it (`MobAction::Walk`), floats it on the top of the water it falls
into (`Idle`, `Bobber::in_water`), and after `BITE_WAIT_TICKS` makes it bite (`Attack`, lowered by
`BITE_DIP`) for `BITE_TICKS`. Drawn in during a bite, the catch drops at the angler's feet with an
orb, and the rod wears by `CATCH_WEAR`; a float caught on the ground wears it by `SNAG_WEAR`. A float
whose angler left, died, went beyond `LINE_LENGTH` or put the rod away is drawn in for nothing. Its
`owner` is the angler's name: the client draws the line to that player (`mob/fishing.rs`,
`draw_lines`), plays the cast, the bite and the reel from the float's appearance, its `Attack` and
its removal (`apply_mob_updates`), and bursts bubbles at a bite (`show_bites`). Its model,
`Bobber.glb`, plunges in its Attack clip.

The dishes are foods (`ItemId::food_value`) more filling than their ingredients. `food_effect` gives
an effect as they are eaten (regeneration for the fish chowder and the grilled glowfin, which a full
player may eat), `eaten_remains` the bowl left in the hand, and `crafting_remains` what a craft
leaves in a cell of the grid (the bucket of a bucket of milk, `take_one_from_each_cell`). The
shapeless recipes take families (`ItemTag::RawFish`, `ItemTag::RoastMeat`). The furnace grills the
fish, fries the eggs and bakes the batter and the raw pies (`smelting_result`); a campfire cooks at
once what the furnace cooks into a food.

The hens lay an egg every `EGG_ODDS` ticks on average (`breeding::lay_eggs`, from
`mob_behavior_system`); a sheep shorn grows its wool back as it grazes. An apple sapling grows an oak
of `AppleLeaves`, which `block_ticks_system` makes drop an apple below them one random tick in
`APPLE_ODDS`; broken, they drop an apple one time in four, and oak leaves an apple sapling one time
in sixty (`DroppedItems::drop_block_items`). Compost, used on a crop, a berry bush or a sapling
(`use_block`), grows the crop a stage or ripens the bush at once, both predicted; for a sapling the
server grows the tree one time in two (`BlockInteractions::fertilized`, `ticks::grow_tree`).

`server/src/mob/wildlife.rs` brings the wild animals back: every `RETURN_INTERVAL_TICKS`, each player
has a chance of a herd of the biome (`WorldGenerator::wild_herds_at`, none on a village) appearing on
grass or snow under the sky in `RETURN_RING` around it, at least `AWAY_FROM_PLAYERS` from every
player and `AWAY_FROM_BEDROLLS` from every bedroll, cut so that the animals within `ZONE_RADIUS`
stay under `ZONE_CAP` (`may_return_at`).

### The depth tiers

`world/depths.rs` (shared) is their one definition: `DepthTier` is the caves (y 50 to 20), the depths
(20 to 12) and the furnace (below 12: the shores of the lava lakes, `LAVA_LEVEL` 10, and all that is
lower). `DepthTier::of` gives that of a cell, none unless ground is above it: air, liquids, leaves
and decorations are not ground, so a valley or a sea floor is in no tier. `strength` is 100, 150
and 200 %. A hostile takes the tier of where it appears (`ServerMob::tier`, saved; guards and spawn
eggs too), which multiplies its health (`max_health`), its damage before the difficulty, its
experience and the rolls of its loot, a treasure added in the two lower tiers (`combat::loot`). The
tier of a player multiplies its spawn attempts and its cap (`player_cap`), that of a guard's place
the odds and the number of the guards; the world's cap stays at 40. `announce_depths_system` tells
a player (`DepthReached`) the first time it is in each tier, kept in `Player::depths_reached` in
its save; the client shows `ui/hud/depths.rs`.

### The lair and the Ember Mother

`world/lair.rs` (shared) draws the one lair of a world with dungeons from its seed and its terrain,
320 to 1000 blocks from the origin, under land; `generation/dungeon/lair.rs` carves it, after the
dungeons, cell by cell from the seed, whatever the order of the chunks. A world saved before the
dungeons has none. The client works out the same place for the depths compass
(`player/compass.rs`), whose needle spins over the lair and where there is none.

`mob/boss.rs` is the Ember Mother (`MobKind::is_boss`): the behaviour, the pushes, the spawns, the
despawns and the save of the mobs leave her alone. `boss_system` runs after the inputs, puts her
asleep on her island when her lair is loaded, invulnerable, and wakes her when a prey (`is_prey`)
is in the lair (`lair::holds`); 5 s without any, or a peaceful world, where she never wakes, put
her back asleep, healed. 200 points of health, 80 more per other player inside. She keeps to her
arena and melts the blocks in her way. Over half her health she keeps 9 blocks away, throws
volleys of embers, calls Cinderlings (2, one more per player, 5 at most) and rests 6 s in each 15;
under it she hunts, strikes and lights the ground (40 fires at most). Her bar is `BossBar`, sent
when it changes to the players near, and plays the boss music (`ui/hud/boss.rs`). A fatal blow
leaves her to `boss_system`, which drops her loot; gone any other way, she sleeps anew. Beaten: `boss_defeated` in `world.ron`, the lava is obsidian, and `Victory` shows
the credits (`ui/credits.rs`, read from `data/credits.txt`; `spawn_credits` serves any screen).

### Maps and waypoints

`world/cartography.rs` (shared) holds what the maps know: `Surface`, the top block of a column as
the minimap scans it (`scan_cell`), the paper maps (`PaperMap`, a square of 128 × 128 blocks on a
grid centred on the origin, `paper_map_centre`, with a `Surface` per column seen) and the
`Waypoints` of a player, kept in its save: its last death (set by `Player::hurt`), the places it
found (`find`, one of a kind within 48 blocks) and its markers (`mark`, at most 64, names of 24
characters). Paper (three reeds), the empty map (eight paper around coal or charcoal), the compass
(four iron ingots around a copper ingot) and the frame of a map (eight sticks around leather) are
crafted (`MAP_RECIPES`); a map drawn and an empty map make a copy, the map staying in its cell
(`copies_a_map`, `craft_result`, `take_one_from_each_cell`). A filled map is `ItemType::Map { id }`.

The simulation does not draw: a right click with an empty map, or on a frame of a map, is told
to the server (`BlockInteractions::drawn_maps`, `frames`), which `world/cartography.rs` (server)
acts upon after the signs and bedrolls: it gives the map an id, fills it around its user, and puts
a map in a frame or takes it back; a broken frame drops its map. Every 5 ticks the map in the hand
of each player fills within 40 blocks; every 20 ticks the village, dungeons and lair of the
regions of the players are looked at (`LoadedGenerator`), and those near enough are found. Every
10 ticks each player is sent the maps it carries or may see in a frame whose revision changed
(`PaperMap`), when they change its waypoints with its home (`Waypoints`), and the frames
(`FramedMaps`). The maps are kept in `maps/<id>.map` of the world (bincode and lz4), written with
the world; the frames in `places.ron` with the signs.

### The seas and the deep caves

`server/src/world/generation/ocean.rs` (`Seas`) adds the life of the seas to the generation, as
a pure function of the seed and the place. Each column of an ocean biome decides its plants and
its reef (`Seas::decorate`, after the terrain of the column): seagrass, a column of kelp whose top
stays under the surface, or, where a slow noise peaks, a reef of the three colours. A column of
chunks may hold a shoal of Glintfins (reefs, shallows) or Shoalbacks (`chunk_fish`, with the
animals). A region of 320 blocks may hold a wreck (`site`, `layout`): a hull of planks 11 long
on a sea floor 9 to 40 deep, turned, rotted, with a broken mast and a chest in its stern whose
loot holds a `TreasureMap` of `ItemType::Treasure { at }`, the chest buried under two blocks of a
column 24 to 72 blocks away, on a shore if one of a few tried is. The wrecks are cached by region
and placed, with their containers, like a structure (`StructureBlocks`), from the regions around
a chunk, since the treasure may lie in the next one. `world/treasure.rs` marks every 20 ticks the
treasure of each treasure map a player holds as a marker named "X" of its waypoints.

Kelp and seagrass grow in water (`BlockId::is_submerged`): their visibility is `Liquid`, so that
they are swum in, breathed in and lit as water, and `is_watery` stands for water in the checks of
the mobs. Placed only into water, broken they leave a water source. The client draws them as their
plant among the blocks and the faces of a cell of water among the liquids (`VoxelShape::in_water`,
`is_water_face`); a column of kelp sways as a whole.

`generation/caverns.rs` (`Caverns`) gives the deep caves of a deep world, from height -1 down to
-56, three biomes by two slow 3D noises (`biome_at`): damp caves (moss on the floors and the
roofs, clay and moss on the walls, pools in the pits, cave vines hanging from the roofs), crystal
caverns (a shell of geodestone with glowing crystal in it, clusters on the floors) and, below -18,
galleries of basalt (magma on the floors, lava in the pits). `decorate` changes the rock next to
the air of the caves (`Caves::carve`, the masks of the chunks around asked as needed) and the air
itself after the ores, so that a cell is the same whichever chunk is generated first. The
creatures: the Mossback, an animal, and the Shardling, a hostile, are born with their chunk
(`chunk_creatures`); the Slagcrawler appears in the dark on basalt and magma (`spawn_kind`), as
the Shardling on geodestone; its bite burns (`inflicted_effect`), as magma does who stands on it
without sneaking (`fire::stands_on_magma`). The fish (`MobKind::is_aquatic`) live in
`server/src/mob/swim.rs`: no gravity in water, still a while, then a swim to a spot nearby or back
to the middle of their kind within 10 blocks (`school`), a dart away from a player within 4
blocks; out of the water they flop toward it. `/locate` finds the nearest wreck by region and the
nearest cell of a cave biome on a grid of 16 blocks at three heights.

### The progress of a player

`players/progress.rs` (shared) is what a player achieved in a world, `Player::progress`, kept in its
save: 21 achievements, all earned through `Player::earn`, which ignores the creative mode: nothing
is earned there, though what was taken there is if held back in survival.
`world/progress.rs` (server) looks every 10 ticks at what a player holds and wears, where it is
(tiers, dungeons, lair) and the night it lives through; kills, trades, births, the bedroll and the
boss earn theirs where they happen, and so does an enchantment (`Player::enchant`). A player is told of its `Progress` alone, with what it has just earned, when it joins
and when it changes: the client shows toasts (`ui/hud/progress.rs`) and the Progress screen of the
pause menu (`ui/menus/progress.rs`). `Progress::stats` counts the play time, the lowest height, the
blocks mined, the creatures beaten and the deaths, all but the time in survival only; it is sent
once a second at most, and shown on the same screen and in the credits.

`Progress::steps` holds the `FirstStep`s whose hint is over: `ui/hud/first_steps.rs` shows that of
the first one left, top left, with the keys bound, in survival and unless the Interface settings
turn the hints off. The server sees the wood cut and the torch placed; the client tells
(`ClientToServerMessage::FirstStep`) the others, and those left after a while. The last points to
the achievements. A player saved before they were has them all done.

## One client frame

`GameSet` (`client/src/state.rs`) orders the in-game systems of `Update`; `game_plugin`
(`client/src/game.rs`) chains them:

1. **UiMode**: is a dialog open, or is the player typing?
2. **Input**: the camera, the selected hotbar slot and the view mode of the frame.
3. **Simulation**: block interactions, then movement, predicted on the local player; a
   crossed portal sets the camera's yaw and pitch.
4. **Reconciliation**: the states received from the server are applied.
5. **Camera**: the camera is placed on the player. Its eye, 0.8 above the centre that crosses
   the portals, goes into a ceiling portal before the centre does and comes out of a ceiling
   twin after it: while the segment from the centre to the eye goes into an open portal, the
   camera is drawn through that portal, from in front of the twin (`Portals::eye_through`,
   `draw_the_eye_through_portals`), never from inside the ceiling, and given back at the start
   of the next frame, for the inputs, the aim and the shots.
6. **Meshing** (in `PostUpdate`): the chunks are drawn by groups of 4 × 4 columns of chunks
   (`GROUP_WIDTH`), each one entity, and a second one for its liquids: a frame costs the CPU
   for each entity, far more than for each face. A group whose chunks changed gets a new
   mesh, built on the async compute pool, which has up to 8 threads
   (`app::task_pool_plugin`), with a task per chunk whose faces are put together. A chunk of
   air, or buried in stone, is not meshed. While chunks arrive, a group waits until its chunks
   and those of the columns along its sides are in (`WaitingChunks`; around the player and
   around each twin, `ChunkReach::areas`), which would each mesh it
   again, so that a group is meshed once; at most 128 groups are
   meshed at once, the nearest to the player first, and about 200 000 vertices are shown a
   frame. A vertex holds its normal and colour in bytes and its atlas tile in a `u16`
   (`meshing.rs`), 32 bytes in all. The GPU culls the groups (`NoCpuCulling`); only those within 12 chunks of the player
   and within its own reach cast shadows in every view, and those within 12 of the twin of a
   portal it approaches and within the reach the player will have there, but not of the player
   (`Approach`, `portal/approach.rs`, which follows the approach as the
   server reckons it and keeps a twin a chunk past the widest approach), in the views of the
   portal cameras only (`ShadowRole`): culled on the CPU, they are taken out of the shadow
   cascades of every other camera once Bevy has listed them
   (`keep_twin_shadows_to_portal_cameras`, after `CheckLightVisibility`; the meshing, which sets
   the roles, runs before `CheckVisibility`: `order_the_shadows`, `world/mod.rs`), so that around a close twin, among the player's
   own chunks at a render distance past 12, their long shadows at a low sun never reach the
   player's view. Bevy hands a mesh culled on the GPU only to the views it was visible to, and
   the cascades are views: a group that starts or stops casting is culled on the CPU for a
   frame (`CpuCulledForAFrame`). Bevy 0.19 filters the casters by the layers of the light, not
   of the view, but fits the cascades to each camera: the sky light shadows layer 3 too, the
   groups only portals show, so that through a far twin the portal camera draws the shadows the
   player sees after crossing (about 0.25 ms more of GPU at High and 0.5 ms at Ultra per portal
   view showing such a twin, linear up to 3 views: +0.76 and +1.43 ms; at 960×540 on an RTX 5060
   Ti, `bench_far_twin_shadows`; nothing without), and none of
   them ever casts in every view, as it would into the player's cascades from beyond its fog.
   And a fog of the colour of the sky hides the edge of the render distance:
   its colour (`sky_fog_color`, `camera/sky_fog.rs`), tonemapped by the materials, is the
   clear colour, which is not, so that a block wholly in the fog is the sky: it is found on
   the CPU, by Newton's method, from the LUT of TonyMcMapface that Bevy keeps in the main
   world (`ToneCurve`). The groups beyond the player's own reach
   (`ChunkReach::within`, the margin included), sent only for a portal, are on layer 3, which
   only the portal cameras draw (`portal_chunks_layers_system`); a group changes layer as the
   player moves, culled on the CPU for that frame like a new view's chunks.
   The faces of the blocks a portal lies on, sides, tops or bottoms, are not meshed.

Then the portals are drawn (`client/src/portal/`, in `PostUpdate` before `GameSet::Meshing`
and the propagation of the transforms). A portal is a box hollowed half a block into its wall (or
floor, or ceiling), drawn by its inner faces, and a glowing rim, all built in the portal's frame
(right, up, normal): deeper than the near plane of the player's camera
reaches behind the portal while its eye is in the opening, and shallow enough that the boxes of
a pair on the two faces of a wall one block thick never meet. An open portal has a camera (`camera.rs`) that stands where the
player's camera would be if the space in front of the portal went on behind its twin
(`through · camera`), with the twin's plane as its near plane (Bevy's oblique `near_clip_plane`,
`projection.rs`), so that what lies between the twin and that camera is not drawn. It draws,
before the main camera, into an image the size of the window, which the portal's surface reads
where it is on the screen (`portal.wgsl`); it is on only while its portal is in view, from the
front, and within the player's fog: a portal wholly beyond it is not drawn and its camera is off
(`beyond_fog`). A portal seen again through itself, as two facing portals are, shows its view
too, down to three levels (`PORTAL_RECURSION`): its camera of level l + 1 stands where its
camera of level l would be through the portal, the player's camera being level 0, and draws
when that one draws and sees the portal's front, in front of the twin's plane (Bevy's frustum
leaves the oblique near plane out) and within its fog. The deepest level draws first (`order`
-l). Each open portal has two images: level l draws into image l % 2 and reads, on its own
portal, the image of level l + 1, so no camera reads the image it draws (wgpu forbids it). A
portal seen through another portal shows its colour. The main camera sees layers 0 and 1 (a
portal's view of level 1); the portal cameras see 0, the local player's body in first person
(2, `BODY_LAYER`: the player sees itself through a portal, not from inside its body; in third
person the body is on 0, and the sky light shadows both), the chunks only portals show (3,
which the sky light shadows too), never the arm and the item the player sees in its own hand
(4, `VIEW_MODEL_LAYER`, the main camera's alone: `player/hand.rs`),
and two of the three layers of each portal drawn (`camera_layers`): its stand-in in its colour,
or, for the portal's own cameras, the surface showing the next level when that level draws (its
colour at the deepest level, nothing past the camera's fog). A
lone portal shows its colour. The colours and the rims are not drawn for a camera behind their
portal's plane, from where the front of a portal cannot be seen: a portal camera always stands
behind the twin, inside its box when it is within 0.12 block of its plane, where it keeps the
plain projection. Its fog ends behind the twin where the chunks the client shows around it end
(`held_reach`), but never farther than the player's own fog, which under water it keeps
(`portal_fog`). The chunks shown around a twin are counted in shells, in the order they are
sent, as they arrive and their group's mesh is shown (`portal/held.rs`, `HeldAreas`): the
whole shells in front of the twin's plane are a box of chunks, and when the twin is among
them, the player's own chunks are another; each box reaches as far as its nearest face, a
face counting as far as the view can turn to it and not at all behind the twin. The fog opens
at `FOG_GROWTH` (96 blocks a second) and closes at once (`eased_reach`), so that a view
streaming in shows fog moving back, not holes nor terrain popping in.
The top and bottom of a box count only nearer than they are to the player at most, as the
player's own fog does not count them: the twin's box as high as the player's render distance
does not count them at all, so through an approached twin wholly shown the fog is the
player's own; a box reaching the bedrock (y = 0) has no bottom.
So looking down out of a far ceiling twin, the fog ends at the bottom of the chunks sent.
Through a close twin, the player's own chunks reach
past its fog, and the portal camera gets that fog as it is: crossing does not change it. The sun, the moon and the stars, parented to the local player, are drawn a
million blocks away (`CELESTIAL_DISTANCE`), on the far plane, unfogged and never
frustum-culled, so that every camera, a portal camera far from the player too, sees them in the
same direction; under water they are hidden. With the shaders, a portal camera gets what makes
the picture (HDR, the physical sky a frame after the player's camera, the rays of light, the
depth prepass) and leaves to the player's camera what it does to the whole screen, the portals
included: tonemapping, colour grading, bloom, SMAA or TAA; it shares the environment map of
the sky made for the player's camera. That map rests on two private internals of Bevy 0.19.1
(`give_portal_cameras_the_sky` in `world/rendering/shaders.rs`): `prepare_probe_textures`
takes once, for the camera of the map, the tables of the atmosphere (LUTs) of the first camera
with an atmosphere it finds (`AtmosphereProbeTextures`), and `TextureCache::get` hands the
textures of a descriptor to the cameras in the order they ask. So a portal camera gets its
atmosphere a frame after the map is made, and tables one texel wider (`portal_atmosphere`):
otherwise the player's sky could be drawn with a portal camera's tables, the ground of the
planet over the horizon. Switched off, the sky goes from the cameras a frame after its
atmosphere (Bevy's `extract_atmosphere` otherwise keeps it extracted, its medium gone, and
panics), the map is put aside (`put_the_sky_map_aside_while_off`), and its tables are taken
from the camera's render entity (`forget_the_sky_tables_while_off`, by the private name), so
that on again Bevy takes the camera's new ones; once off, the sky is shown again only after
every camera lost its atmosphere (`settle_the_sky`), however short the switch. The name needs
Bevy's `debug` feature, which `client/Cargo.toml` asks of `bevy_utils`. A test fails once Bevy is another version
(`the_sky_rests_on_the_internals_of_bevy_0_19_1`). Its image is in 16-bit floats at every quality, which hold
the colours before tonemapping with the shaders, and after it without. It draws the god-ray
haze, a layer 640 blocks wide of its own, centred under it (`follow_cameras_with_haze`): Bevy
draws every fog volume in every view, and its shader, changed, leaves out a volume tagged as
one camera's (`one_camera_offset`) in every view whose camera does not stand within half a
block of where it says along x, y and z and look within a degree of its way, so the player's
view never changes with the portals, not even under a floor portal stacked under a ceiling
portal, whose cameras stand straight above it, and a portal camera far off sees the haze the
player will see after crossing. Cameras that stand and look alike, as those of pairs side by
side whose twins are as far apart do, share one volume (`drawn_in`, the filter on the CPU):
each would draw the other's. The CPU's test of the look is stricter than the shader's by 1e-4
of cosine (`SHARED_LOOK` against `ONE_CAMERA_LOOK`), a hundred times what the sines and cosines
of desktop GPUs are off by, so that a camera never shares a volume its shader would leave out,
at the clamp of the pitch too; with looks 0.81° to 1.15° apart, it draws two hazes instead. Each view marches its own layer, about 0.6 ms of GPU at High and
1.3 ms at Ultra at 960×540 on an RTX 5060 Ti; leaving out another camera's volume costs a view
about 15 µs (`bench_portal_haze_per_pair`). If Bevy's shader text no longer matches, the game
logs an error and a unit test fails, and the haze is as before the portal cameras drew it: the
player's camera alone, with one volume, untagged (`HazePerCamera`). Out of the layer, above or under it, a portal camera's volume is stretched to hold
it (`haze_transform`), which Bevy then draws from inside: from outside it rasterizes the faces
of the volume, which the oblique near plane on the twin clips when the twin is in the layer, and
the view had no haze. The volume's offset carries the heights of the layer, and the changed
shader marches only between them (`START_IN_THE_LAYER`); the player's volume, which is the
layer, carries none (`one_camera_offset_uncut`), and its rays are marched as before. It draws it as the player's camera
does (same steps, no jitter: the player's TAA reaches its image only through the motion of the
portal surface; at level 2 and deeper, no more steps than High), through its oblique
near plane: Bevy's `volumetric_fog.wgsl` read the start depth of a ray with the near distance
only and ended the rays of the sky on the far plane, which the oblique matrix brings near, and
`world/rendering/volumetric_fog.rs` changes those two lines in Bevy's embedded shader before it
is loaded (the same result under a plain projection). It draws no ambient occlusion: Bevy's is
noisy and left to TAA, and through a close portal it measured as a seam. The horizon skirt, a ring closed at the bottom
so that no way under the horizon misses it, is drawn around each camera (`horizon.wgsl`), and
so is the overcast dome (`overcast.wgsl`). Under the weather, a portal camera copies the fog
and the clear colour of the player's camera, overcast and flashed, and its own fog behind a far
twin closes in as the player's does (`Overcast::fog_range`); the light, the exposure, the
grading and the wet ground are the same for every view. It is a `PrecipitationView` too: the
rain, the snow and the bolts are drawn around it, so they show through the portal around the
twin, clipped by its near plane. Its drops stop under the roofs by the twin: each twin of a
portal with cameras has an occlusion map of its own, centred on its opening
(`portal/weather.rs`, `ViewOcclusion`), and whether a view draws drops and bolts is its own
(`UnderWaterView`: the player's camera by its eye, a portal camera by the medium its twin opens
into; under water, neither). A strike within the shown reach of an approached twin draws its bolt, and each view
draws only the bolts struck within the shown reach of its own eye (`drawn_from`, cut in
`lightning.wgsl`): one by a far twin shows through the portal, not over the player's horizon.
A view that stops being a `PrecipitationView` loses the bind groups of both passes
(`forget_views_no_longer_drawn`), which would otherwise go on drawing it, and so does a view
in a frame that misses one of their bindings, rather than draw with last frame's. The systems of
the weather that follow one camera (the dome, the flash, the ears of the weather sounds) take
the camera of the world that is not a portal camera.
A shot is seen flying (`portal/shot.rs`): the prediction of the local player (its bolt 0.6
block ahead of its eye) and the server's `PortalShot` for the others write a `ShotFired`, from
which a bolt and its trail are spawned, moved at `BOLT_SPEED` (100 blocks a second) to the hit
point, where the bolt bursts into eight sparks (the fox's dust particles) and both are
despawned once the trail has shrunk into it. They are unlit and alpha-blended, brighter than
the portal's colour, four times more with HDR to bloom over the physical sky. What is drawn of
the portals is `ShownPortals`: while a placed shot's bolt flies, the client map's portals with
that portal as it was drawn (or none), else `None`, the map's as they are. `sync_portals`, the
portal cameras and the meshing of the chunks read it, so a portal, its opening sound and the
hole in its wall come when its bolt lands; the simulation, on the map, has it at once.
The rim fades out within 0.3 block of its plane, where only a player passing through stands,
and from where it would be a line down the screen. The aimed block is not outlined on the face
of an open portal. The chunks are
culled on the GPU only, and Bevy 0.19 hands such a
mesh only to the views there are when it becomes visible: when a portal camera is turned on,
the chunks are culled on the CPU for a frame (`show_chunks_to_new_views`).

The sky follows the world clock (`ClientTime`): the sun and the moon turn around the player
(`celestial.rs`), and the clear colour and the fog go from `SKY_COLOR` at noon to a warm
colour at sunrise and sunset and a dark blue at night (`sky_color`, `camera/underwater.rs`;
the fog, `sky_fog_color`, shows as that colour once tonemapped);
under water the fog keeps the colour of the water. The clouds (`world/clouds/`) are cells of
12 × 12 blocks, 4 high, at y = 192, each with a density after a noise that repeats every 256
cells and has the same seed in every world, so that all the players of a server see the same clouds
with nothing sent. They drift east at half a block a second, from the clock, by the
`Transform` of their entity, so that culling and sorting see where they are; their mesh holds
the outer faces of a disk of cells around the camera, twice the render distance wide and at
most 1024 blocks, and is built again only when the camera enters another cell of the pattern;
it also holds a disk around the twin of each portal the player approaches (`Approach::twins`,
`quads_around`, each disk meshed on its own window, the faces of the cells two disks share
once), so that the clouds a portal camera shows there are those the player sees after
crossing.
Each face carries the densities of the two cells it lies between, and the shader shows it when
the threshold of the weather lies between them, so the clouds thicken without a new mesh;
under a clear sky the mesh holds only the cells full then, about a seventh of the faces, and is
built with the others as soon as the sky clouds over.
`clouds.wgsl` draws them unlit, tinted by the hour and by the side of each face, at 0.8
opacity and fading out at the edge; a child entity first writes their depth alone, so that
only the nearest face of a cloud is coloured and its opacity is the same from below, above
and inside. The player chooses Fancy, Fast (one flat layer) or Off in the Video tab
(`CloudStyle`, saved in `video.ron` with the render distance).

A day lasts 20 minutes; a new world starts 75 s after sunrise (`NEW_WORLD_TIME`). The sun and the moon go round on a path that leans 30° from the
vertical, so they are never right overhead; `celestial.rs` draws them, and the stars, with
`CelestialMaterial` (`celestial.wgsl`) on the far plane: a square sun reddening near the
horizon, a square moon of 16 × 16 pixels that waxes and wanes over 8 days, and stars that turn
with the sky and show at night. The shaders (`world/rendering/shaders.rs`) are chosen in the Video
tab, Off, Low, Medium, High (the default) or Ultra (`ShaderQuality`, saved in `video.ron`), and
applied in game at once by inserting and removing Bevy's components on the camera and the
light of the sky: a physical sky (Bevy's atmosphere, in an HDR target, with an exposure that
follows the sun, bloom and colour grading), SMAA or TAA, water reflecting the screen
(`EFFECT_SSR` in `chunk.wgsl`), rays of light through a haze over the ground, ambient occlusion
in screen space and longer, sharper shadows. `bench_shaders` measures what each costs on the
GPU. The end-to-end tests draw without them unless a test asks (`Options::shaders`).

The weather the server tells (`ClientWeather`, `world/weather.rs`) is two levels, rain and
thunder, from 0 to 1, that ease towards it: each follows the smoothstep of a ramp that moves
linearly, 20 s from one end to the other, so that a storm rolls in and out gently and a
change turned back midway does not jump. A player who joins gets them at once, and they go
back to clear when the world is left. The debug overlay (F3) shows them. The wetness of the world is the client's
own too: it rises while drops fall, 40 s to soaked, and dries in 2 minutes.

Every effect reads these levels through its own curve, the sky leading the drops. The overcast
(`world/rendering/overcast.rs`) is `smoothstep(0, 0.5, rain)`, and a storm level darkens it
further. It greys and thickens the clouds, hides the sun, the moon and the stars, dims the sun,
turns the environment light down for a grey ambient light, lowers the saturation and closes the
fog in, never nearer than 48 blocks. What the shaders know of the weather is one storage buffer,
`WeatherShading` (`weather.wgsl`): the greys of the overcast at the horizon and the zenith, the
wind and how far it has carried what falls (summed on the CPU, `WindDrift`), the levels, the
wetness and the lightning, bound at 107 to every material that shows the sky (the chunks, for
their fog and the water's reflection, the horizon skirt and the overcast dome) and written in
place when it changes, so that no chunk's bind group is made again. The
dome is drawn on the far plane over the physical sky; without the shaders, the clear colour
and the fog take the same grey.

The rain and the snow (`world/precipitation/`) have no mesh and no entity: a render pass of
their own, in `Core3d` after the TAA and before the bloom and the tonemapping, builds every
drop, flake, splash and sheet of far rain from the vertex index and the time, and tests the
depth of the world by hand. Drawn before the TAA, thin blended streaks without motion vectors
would be smeared away. The climate map (`world/rendering/climate.rs`) tells each column's rain,
snow or nothing, from its biome, and the snow line turns rain to snow. The occlusion map
(`occlusion.rs`) is a 144 × 144 texture of the 9 × 9 chunk columns around the eye holding the
height at which what falls stops (any block but air and the decorations, water at its
surface), scanned again when chunks come and go and the same frame a block changes; a chunk
column not scanned yet, after a jump or in a new world, stops everything. The rain
rings the water in `chunk.wgsl`'s `water_normal`, in every preset that draws its waves.

In High and Ultra (`ShaderFeatures::wet`) the world gets wet; elsewhere the wetness reaches the
shaders as 0 and every wet term is skipped. The chunks bind the occlusion map at 108, so that
`chunk.wgsl` wets only the faces the rain reaches, and each block texture has a surface
(`WetSurface`, in the texture table) saying how much it darkens, how glossy it turns and
whether puddles gather on it. The sheen and the puddles reflect `sky_radiance`, not the world:
the ground is drawn before the frame the water's reflection reads.

A `LightningStrike` becomes a `Strike` (`world/lightning/`). Its bolt is grown from the seed by
a small PRNG of our own (`bolt.rs`), so every client draws the same, and flashes in two or
three strokes over half a second. A second pass after the TAA (`render.rs`, `lightning.wgsl`)
draws its channels, held in a storage buffer, as ribbons facing the camera, added to the frame
far brighter than white for the bloom; a strike beyond the render distance draws no bolt, only
its flash in the clouds, which fades out by 1,500 blocks, where the thunder is no longer heard.
The flash reaches the shaders through `WeatherShading` (the struck block and the direction of
the flash in the clouds) and lights the sky, the clouds and the faces open to it, and the
ambient light and the exposure jump with it. The strikes are aged in `PreUpdate`, before every
system that reads the flash.

The sounds of the weather (`audio/weather.rs`) play on their own Weather volume. The thunder
of a strike is delayed by its distance at 340 blocks a second, cracks within 48 blocks and
rumbles farther, and is not attenuated by Bevy's spatial sound, which would silence a strike
100 blocks away: its emitter is kept 2 blocks from the ears toward the strike, its volume set by
hand. Three loops, the rain in the open, the rain drumming on a roof and the wind, play all
along, their volumes following the weather, the share of rain and snow around, and how open the
player stands, found by rays read against the occlusion map; under water they are muffled.

The network is polled in `PreUpdate`, every frame, and the inputs are uploaded in
`FixedUpdate`. A message of chunks of 4 KiB or more is decoded on the compute pool, and the
messages are applied in the order they came (`DecodingChunkMessages`), about 1024 chunks a
frame at most.

The client has one plugin per domain, registered by `game_plugin`: `camera`, `input`, `mob`,
`network`, `player`, `portal`, `ui::hud` and `world`. The player's camera is the one with a
`CameraController`. The menus have their own plugin
(`ui::menus::menu_plugin`), and so do the sounds (`audio::audio_plugin`), heard in the menus
too. The gameplay sends messages (`BlockChanged`, `LocalPlayerMoved`, `PortalOpened`) that
`audio` turns into sounds, a portal that opens and a passing through one included, from files
of other sounds; the sound files in `data/sounds/` are made by `scripts/generate_sounds.py`. Each
creature has its voice (`audio/creatures.rs`): a `CreatureVoice` on each mob drawn times its
cries, and follows the actions the server sends: a hostile that starts to `Chase` is heard
approaching, again every few seconds while it hunts, and one that starts to `Attack` strikes;
`MobHurt` carries the kind, for its own hurt. The sounds of the later features (`audio/cues.rs`, files by
`scripts/generate_feature_sounds.py`) are read from what the server already sends, nothing
added to the protocol: the armour worn and the bow drawn of each player (put on, taken off, or
gone in a hurt: broken), the arrows that vanish (in flight near a player or a creature: a hit;
stuck near a player: picked up), `ItemPickedUp::orb`, the level, `TradeUpdated` (an offer left
that goes down was taken), a click on a disabled offer or enchantment (refused), `Mob::in_love`,
`Mob::tamed`, `Mob::born` (first seen in its first two seconds), a mob's feet going into water
after a second out of it, and the resources of the HUD (`DepthReached`, `JustEarned`, `NearBoss`,
a `Credits` spawned: the victory in a game). Each is rate-limited; the stings play on the Music
volume, the screens' on the Interface one, the first-step hints none. The music (`audio/music.rs`) is made by
`scripts/generate_music.py` into `data/music/`: a `MusicScheduler` plays one piece at a time,
with minutes of silence between them, chosen for the mood of the moment it starts in (the title
in the menus, the day, the night, the underground below y 50). It fades a piece out early only
when the kind of moment changes: leaving the menus, or a boss fight, which later code asks for
with `MusicMoodOverride(Some(MusicMood::Boss))`. It plays on the Music volume. The sheep
model, `data/models/animated/Sheep.glb`, is built in Blender by `scripts/generate_sheep.py`, and
those of the other animals by `scripts/generate_animals.py`. `GameState`
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
by the server. While a chest is open, its slots take the place of the crafting row. The
gestures that last longer than a press are in `ui/hud/inventory/gestures.rs`: the drag of the
held stack over cells, sent on its release with a preview of its counts from the shared
`drag_plan`, the double clicks, and the inventory tweaks of Mouse Tweaks, Shift+drag and the
wheel, all in the `CellGestures` resource. The item
browser (`ui/hud/item_browser/`) shows beside the open inventory, in `HudBottom` with it, every
item of the game: `catalog.rs` orders and searches them, `layout.rs` gives the columns that fit
the window (8 to 4, the panel sliding left to leave them room, else a drawer of up to 6 over the
right edge). In Creative, items come out of it: `creative.rs` decides what a gesture on the
catalog does, and `catalog_actions` sends it as an `InventoryAction`, as the inventory's clicks
are sent; the observers of the panel (Enter in the search, `+ Fill grid`) leave their request in
`BrowserRequests` for it, since an observer runs outside the frame's input. It is also the recipe
book: `view.rs` shows the
recipes or the uses of an item instead of the catalog, as cards built when the view changes, from
the read-only recipe API of `shared::world::crafting` (`recipes`, `recipes_for`, `uses_of`,
`missing`, which counts the inventory and the crafting grid), with a history in
`ItemBrowserState`; `recipes.rs` holds its texts. `bookmarks.rs` keeps the items the player bookmarked with A, for the world being played: read from `item_browser.ron` in the game folder when a world is entered (keyed by `SessionOrigin`: a world by its name, a server by its address as typed), written at each change, and shown as a row of slots above the catalog that are catalog slots too (`CatalogSlot` with `BookmarkSlot`), so that every gesture of the catalog works on them. R, U and A act on the same slot, `KeyedSlot`: the focused one while its ring shows, else the hovered one. Tab walks the panel and the browser together. `dialog.rs` is a modal over the screen: its focused action (Cancel, in a
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
`list_worlds`. The Multiplayer screen
(`ui/menus/multi/`) rebuilds its server list from `servers.ron` after every change, and asks the
statuses and resolves the host names on threads (`InFlight`), so a frame never waits for the
network. A dialog and a key capture take Escape through `FocusedInput<KeyboardInput>` and finish
in `PostUpdate`; the handlers that read `ButtonInput<KeyCode>` in `Update` (the Escape of the
main menu, of the world and server lists, of the settings and of the pause menu, and the New
world opener, Ctrl+N) skip while a modal is open (`modal_open`: a dialog, or the quitting card),
a text field has the focus, a key is being captured or, for the pause menu, the settings are
open. The Skin screen (`ui/menus/skins.rs`) lists the skins of the game
(`shared::players::skin::BUILT_IN_SKINS`, drawn by `scripts/generate_skins.py`) and the PNGs of
the `skins/` folder of the game folder, where a file dropped on the window is copied
(`FileDragAndDrop`); a 64x32 skin is widened to 64x64 by mirroring its right arm and leg
(`player/skin.rs`). Its preview is the player's model (`spawn_player_model`) on a layer of its own
(`PREVIEW_LAYER`), drawn by a `Camera3d` into an image the screen shows; the main menu shows it too, beside its
column of buttons, between the player's name and the Skin button (`spawn_home_preview`). The choice is saved in
`skin.ron`; `ChosenSkin` holds it with the `PlayerSkin` sent in `AuthRegisterRequest`: a name, or
the 16 KiB of RGBA of an imported skin. The server keeps the validated skin on the `Player` (not
saved) and every `PlayerSpawnEvent` carries it, so a client builds the model of each player from
the asset of a skin of the game or from an image of the pixels. What a player holds is the item
of its selected hotbar slot (`Inventory::in_hand`), which the server puts in each
`PlayerUpdateEvent` so that the others see it: `player/hand.rs` draws it in the right hand of
every model, under the arm, with the meshes of the dropped items (a block as a small cube, any
other item as its thick picture), and, for the local player in first person, in a view model
hung from its camera: its right arm, cut from its skin, and the item in the lower right, which
bobs with the walk, swings with the strike of the arm, pushes forward when a block is placed or
an item used up and goes down and up again when the item changes. Its materials are unlit copies
dimmed with the ambient light, and its frame stretches with the field of view, so that a zoom
does not enlarge it. The connecting card reads Escape the same way: no dialog or field exists there. The
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

The Account dialog (`ui/menus/account.rs`) is opened by the name button of the main menu, or the
name dialog when `--player-name` was given. Logged out, one form dialog has the Log in and Create
account tabs (`spawn_tabs`) over three fields, the confirmation being hidden on the first tab
(its `TabIndex` goes to -1); "Forgot password?" and "Play offline as..." close it with a `Follow`
screen, which opens on the next frame in `NextDialog`, once the focus is back. A call to the hub
(`account/hub.rs`) runs on a thread (`InFlight`) and `poll_call` takes its answer: the account
becomes `Account::LoggedIn`, is saved to `account.ron`, and the `Renewal` is asked for a
certificate at once (`ask_now`); the refusals of the hub go under their field (`FieldError`) or in
a toast. Making or recovering an account shows the recovery code in a dialog that Escape shows
again: only its button ends it. `EditableText` has no password masking in Bevy 0.19, so a password
field keeps its text transparent and a sibling `Text` draws one bullet per character over it.
Opening the dialog of a logged-in account asks the hub for `GET /v1/me`; a 401 logs the account
out. The end-to-end tests (`e2e/account.rs`) run it against the stand-in hub of
`account/test_hub.rs`, whose URL they give `Options::hub`.

The Multiplayer screen has two tabs, Servers and Hosted worlds, each a `MenuState` of its own
(`Multi`, `Hosted`) whose tab bar opens the other. The Hosted worlds tab
(`ui/menus/multi/hosted.rs`) asks the hub `GET /v1/me` then `GET /v1/realms` on a thread
(`HubCalls`, on the screen's root, which drops its calls and their answers with it) when it
opens, on Refresh (F5), when the account changes and after each change it made, and builds its
cards and its detail pane again from each answer; one listing at a time, the newest: a new one
forgets the one still waiting, and Refresh waits for it. Logged out, it offers the Account
dialog (`NextDialog`); a session the hub refuses (401) logs the account out through
`end_revoked_session`, as at start, and the dialogs of the tab (`HostedDialog`) close when the
account changes, without their action (`dismiss_dialog`, as Escape). The buttons follow what the player is to the world (`standing`): Join for
everyone but while the world stops, Members for the owner and the operators (read-only for
them), Stop and Delete for the owner, after a confirmation; Create a world shows when the
account has the `host` role and room in its `realm_quota`. The form and the Members dialog wait
for their call (`busy`), and say under their field why the hub refused. Its dialogs are in `hosted_dialogs.rs`: the form of a new world sends the
names of `GameMode`, `Difficulty` and `WorldPreset` (kebab-case, no single biome) as the
server's flags take them, which a test checks against the hub's allowlist
(`hub/agent-protocol/src/allowlist.rs`). Joining a hosted world (`network/hosted.rs`) opens a
session whose `SessionOrigin::Server` has its `realm`, and an empty `ip` and an unspecified
`address` until they are known: `HostedStart` asks the hub to start the world (`POST
/v1/realms/{id}/start`, which only says where a running world is; up to 200 s, above the 3
minutes the hub gives its agent), then asks its status port (`StatusProbe`) every 500 ms for 60 s
at most from the hub's answer, on a thread that stops asking once a cancel dropped
`HostedStart`, while the connecting card shows Starting the world...
(`ConnectStep::StartingHostedWorld`, the first row as for the closing of the last world). The
launch of a local server and the connection stand down meanwhile; `wait_for_hosted_world` then
hands the address over to `init_server_connection`, or ends the session with
`ConnectError::HostedWorldFailed` and the hub's message on the error card, whose Try again starts
the world again and whose way back is the Hosted worlds tab (`list_of`); a refused session ends
it with `ConnectError::HubSessionEnded`, which logs the account out and has no Try again. `TestHub::routes`
answers by method and path, for the end-to-end tests of the tab (`e2e/hosted.rs`).

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

`ViewSettings` (`camera/settings.rs`, saved in `view.ron`) holds the mouse sensitivity, set on the
Controls tab, the field of view and the fullscreen, set on the Video tab. `fov_system`
(`player/stance.rs`) eases the camera to that field of view, which the sprint and the zoom
scale.

`M` opens the world map (`ui/hud/minimap/world_map.rs`), a dialog over the whole window: the
same surface, all of it, drawn by `compose_world` tile by tile into one image at a block a pixel,
again at most every 250 ms while the surface changes. The UI scales that image (1 to 8 px a
block) and moves it, so that dragging, the wheel (around the cursor) and `=` / `-` never draw it
again; the arrow and the dots are its children, placed in percent of it. It follows the player
until dragged, and Space centres it again. It joins `Overlays`, so the HUD hides under it, and
closes on `M` or Escape.

The waypoints and the paper maps (`ui/hud/minimap/cartography.rs`) come from the server
(`Cartography`, filled by `network/world.rs`). The waypoints are marks over the minimap (home and
the last death held at its edge) and on the world map with their names: home (the bedroll, or the
spawn of the world), the last death, the villages, dungeons and lair found, and the markers named
by the player. A click on the world map that ends no drag (`WorldMap::dragged`) opens the editor of
a marker, a name and one of `MARKER_COLORS`, sent as `PlaceMarker`; a right click on a marker sends
`RemoveMarker`. Each paper map received is drawn into an image of 128 × 128 (`paper_pixels`: the
minimap's colours on paper), shown at the bottom right while it is in the selected slot, with the
arrow of the player, and on the front of the frames of maps as an unlit quad. The compass
(`player/compass.rs`, `HomeNeedle`) points home as the depths compass points to the lair.

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
| `world.ron` | Name, seed, world type (`generation`), difficulty, the game mode new players start in, whether the boss was beaten, time, mobs, the items lying on the ground, the weather and the ticks it has left, and the format version | the writer thread |
| `chunks/<x>_<y>_<z>.region` | The chunks of a region of 8×8×8 chunks, each encoded like a network message, after an index of where each starts | the writer thread |
| `chunks/<x>_<y>_<z>.bin` | One chunk, encoded the same way, as worlds saved before version 5 hold them: read when its region does not hold it | worlds saved before |
| `containers/<x>_<y>_<z>.bin` | The contents of the chests of one chunk, encoded the same way; only a chunk holding a chest has one | the writer thread |
| `furnaces/<x>_<y>_<z>.bin` | The furnaces of one chunk, what they hold and how far they burnt and cooked, likewise | the writer thread |
| `players/<id>.ron` | Position, orientation and inventory of one player; `<account uuid>` instead of the id on a server in hub mode | the writer thread |

Every file is written to a temporary file and renamed, so a crash never leaves a half-written
file (`server/src/world/format.rs`). The writer thread (`world/writer.rs`) keeps the disk out
of the tick. It writes the files of a save aside on 8 threads, syncs them all at once (one
`syncfs` on Linux, where syncing each file costs milliseconds; a save of 16 files or fewer syncs
each of them instead), then puts them in place, the metadata first. Only the changed chunks are written, into their regions, which keep the other chunks they held, with the files of their chests and their furnaces, each
removed once they hold none (`world/container_files.rs`, generic over the `Stored` kind of
block). Worlds saved by older versions are converted
on their first load and the old files are kept: `world.ron.legacy` for the single-file formats,
`chunks.v1/` for the chunks of format version 1, whose blocks had no level (`world/legacy.rs`,
`world/load_from_file.rs`). A world without `furnaces/`, saved before the furnaces smelted, has
empty furnaces. Format version 2 differs from 3 only by having no `containers/`,
3 from 4 only by writing a chunk cell by cell, and 4 from 5 only by a file per chunk: all load
as they are.

The Singleplayer screen manages the folders (`client/src/ui/menus/worlds.rs`): it writes
`world.ron` when a world is created, with the world type, the game mode and the difficulty
chosen (`NewWorldOptions`), so that the world is listed at once; a rename renames the folder and the name in `world.ron`, keeping its format version and its date; a delete removes
the folder, after a confirmation.

## Hub

The hub is a service apart (`hub/`, its own Cargo workspace; `hub/README.md` has its routes, options
and the deployment). It builds from the same toolchain and shares `crates/protocol` with the game, and
no game crate depends on it: a game talks to it over HTTPS and JSON.

| Part | Files | Role |
|------|-------|------|
| The hub | `hub/src` (`accounts`, `sessions`, `certificates`, `admin`, `realms/`, `maintenance`) | axum over Postgres (`hub/migrations`, applied at start). Accounts with Argon2id passwords and recovery codes, bearer sessions, certificates of devices signed with the hub's Ed25519 key, the list of revoked ones, and the hosted worlds. |
| The realm agent | `hub/realm-agent`, types in `hub/agent-protocol` | The only holder of `docker.sock`. It takes newline-delimited JSON requests (`start`, `stop`, `status`, `archive`, `purge`) on a Unix socket, checks them against the game's names (`allowlist.rs`) and runs one container per world. |
| In the game | `client/src/account` (`hub.rs` calls, `file.rs` is `account.ron`), `client/src/network/hosted.rs`, `ui/menus/account.rs`, `ui/menus/multi/hosted*.rs`; `server/src/hub.rs` | The Account dialog and the Hosted worlds tab, and the server's side of `--auth hub` and `--realm`. |

Three flows cross the hub:

- **Joining a server with an account.** The client logs in (`POST /v1/sessions`), then a
  `Renewal` thread asks `POST /v1/certificates` for a certificate of its device key, valid 7 days,
  and renews it in the background. The server (`--auth hub`) reads the hub's keys (`/.well-known/rustcraft-hub`) at its
  start and the revoked certificates every hour. A join carries the certificate and a signature
  of the server's challenge and of the address the player dialed; the server checks them alone.
  Changing the password, recovering the account or suspending it revokes its certificates.
- **Starting a hosted world.** `POST /v1/realms/{id}/start`: the hub takes the lowest free slot
  (at most `--max-running` at a time), renews the world's realm token (only its hash is stored),
  and asks the agent to start `rustcraft-realm-<uuid>`, which publishes the UDP ports
  `base_port + 2n` (game) and `+ 1` (status). The client then asks the status port of the
  world until it answers (`network/hosted.rs`) and connects as to any server. The hub's
  `realms::monitor` probes it too (`realms::probe`) and moves the world through `starting`,
  `running`, `stopping`, `stopped` or `failed`, and stops it after 10 minutes without players.
- **The members of a hosted world.** The server of a world (`--realm <uuid>`, the token in
  `RUSTCRAFT_REALM_TOKEN`) reads `GET /v1/realms/{id}/config` at its start and every minute: the
  owner and the members decide its whitelist and its operators, so a world has no `ops.ron`.
  A refused token (the world was stopped or deleted) ends the server after it saves.

`maintenance` runs the hourly and daily chores (old sign-ups and sessions, names of deleted
accounts, archiving and purging worlds). The deployment (`hub/compose.yaml`, `hub/deploy`): Compose
with Postgres, the hub on `127.0.0.1:8080` behind Caddy, the agent with no network, and the worlds on
a Docker network of their own whose published UDP ports a `DOCKER-USER` rule is the only way in to.

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
  id. This is fine among friends, not for a server open to anyone: such a server runs with
  `--auth hub`, where the id must be the one of the account the player proves. Even then only
  the handshake is checked: the packets after it are not authenticated, so someone who can
  forge the player's UDP address can speak for it, and since a client picks its own id,
  someone who connects in a loop with a player's id keeps the player out (each attempt is
  dismissed after 5 s, `AuthTimeout`). Netcode's secure connect tokens, issued by the hub, are
  the fix; they are left for later.
- Block interactions are processed once per input, so once per rendered frame of the client:
  a client with a higher frame rate breaks blocks faster.
- A rider is not saved on its mount: a player who joins again stands where it sat. The passenger of a
  boat is carried by the server, a tick behind its rower. A horse's coat and gifts are rolled by the
  server, so a horse of an older save gets them the first tick it moves.
- A player is not held back by the mobs it pushes; no animal is born after a chunk is
  generated: a herd led away is gone for good.
- What `/give` finds no room for falls at the feet of the player up to a backpack of it (36 stacks);
  the rest is lost, and the answer says so. The item browser of creative puts into the inventory
  what fits of a stack taken from it, and forgets the rest.
- A name given at an anvil is checked by nobody: a server shows any word a player writes on its items.
- A player who dies loses its inventory for good unless it comes back
  for it within the 5 minutes its items lie there, which count on even where no player is.
- The Gnawer and the Stumpling find their way to their prey (`mob::path`, an A* of at most 400
  nodes within 24 blocks, looked for again every second or when the prey moves 2 blocks, 1600
  nodes a tick for all), but a prey farther round than that is walked at straight, toward the
  nearest cell met. The Roller still rolls straight, the Drowner keeps to the water, and the
  flyers fly straight. A Stumpling opens doors and leaves them open; no hostile breaks a block.
  The spawning ignores the game mode, so hostiles still spawn around a creative player, though
  they leave it alone.
- Nothing proves who a player is: a server knows its operators, bans and whitelist by name
  only, and a player may join under the name of an operator.
- A world still loading 10 s after the quit is stopped with the process: its server cannot be
  told to stop before it accepts the player. Its files are written atomically, so the save stays
  readable.
- Chinese, Japanese and Korean characters and emoji are in neither font, the pixel font nor Fira
  Mono: they show as missing glyphs in the chat.
- An accented letter pasted into the chat field may show for one frame in the pixel font, as a
  missing glyph, before the field switches to Fira Mono.
