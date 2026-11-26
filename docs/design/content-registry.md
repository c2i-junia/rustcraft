# Data-driven content and hot reload for Rustcraft — design

Read-only study of `main` at `2d1ea403` (2026-11-26). Nothing was built or run; every cost and
effort figure below is an estimate from reading the code, not a measurement.

## 0. Decision in one page

1. **Content becomes data files** under `data/content/rustcraft/`, RON, **one file per feature**
   (`jungle.ron`, `palette.ron`, `forge.ron`), each holding its blocks, items, recipes, smelting
   and mob stats. Adding a feature adds a file; it does not edit shared lists.
2. **Properties are read from one dense table** indexed by the block number, held in a
   process-global snapshot (`&'static Content` behind an `AtomicPtr`, old snapshots leaked).
   The `impl BlockId` getters keep their signatures; their bodies become an array read.
3. **Strangler first.** The enums stay for the first three lots. The table is first *generated
   from the existing match arms*, then *overridden by files*, then the files become the source
   and the match arms are deleted. Hot reload of textures, hardness, light, drops arrives
   before any id is touched.
4. **Then `BlockId(u16)` / `ItemId(u16)`** with associated consts named exactly as today's
   variants (`BlockId::Stone` keeps compiling — the trick of the PoC branch), so the ~7000
   references are not rewritten. Engine-named blocks keep compile-time numbers; data-only
   blocks get numbers at load.
5. **Ids are strings on disk** (`rustcraft:jungle_log`). Each world stores its own
   name→number list; chunk files keep storing numbers and are remapped per chunk palette on
   load. Old worlds load with **no chunk rewrite**: a world without the list is read with the
   frozen list of today's 280 names.
6. **Server is authoritative**: it watches the files (`notify`), validates, swaps, and sends
   the whole content snapshot at join and on each reload — the same pattern `biomes:
   Vec<BiomeDef>` already uses in `AuthRegisterResponse` (`shared/src/messages/auth.rs:57-58`).
   The client rebuilds the texture array and `BlockLooks`, then calls the existing
   `respawn_every_group` (`client/src/world/rendering/render.rs:915`).
7. **Behaviour stays Rust**, attached by a `behaviour` enum and tags in the data. `MobKind`
   stays an enum; only mob stats, models, clips and drops move to data.

Estimated total: **about 20 agent-days in 10 lots**. First visible value after lot 0
(textures, about half a day) and lot 2 (hardness / light / drops edited live, about 4 days in).
A brand-new block with no Rust change needs lots 0-5 (about 13 days).

---

## 1. How properties are read today, and what a table costs

### 1.1 Current state

- `BlockId` is a fieldless enum of 280 variants (`shared/src/world/blocks.rs:25`), `ALL` at
  `:547`. Every property is a `match` in `impl BlockId` (`:545-1570`): `get_hitbox` `:910`,
  `get_ray_hitbox` `:976`, `get_break_time` `:1051`, `is_mined_faster_by` `:1170`,
  `get_required_tool` `:1270`, `get_drop_table` `:1336`, `is_usable` `:1454`,
  `get_visibility` `:1479`, `light_opacity` `:1549`, `light_emission` `:1559`.
- Most of them first call `self.palette()` (`shared/src/world/palette.rs:443`, a ~190-line
  match returning `Palette { form, base, item }`), then fall back to the plain match. So a
  property read today is one or two jump tables plus, for palette blocks, a recursive call on
  `base`.
- The hot loops already amortise by *kind*, not by cell, because chunks are paletted
  (`shared/src/world/chunk.rs:45`, at most 256 kinds):
  - Lighting: `chunk.map_cells(opacity)` / `map_cells(emission)`
    (`client/src/world/rendering/lighting.rs:333-336`, `:540`) — the closure runs once per
    kind (`chunk.rs:189-197`).
  - Meshing: `chunk.map_cells(|block| block.map(kind))`
    (`client/src/world/rendering/meshing.rs:291`), `kind` = `(id, get_visibility())` (`:270-273`).
    Per visible block it reads `looks.get(block.id)`, already a dense `Vec` indexed by
    `id as usize` (`meshing.rs:381-401`).
  - Server simulation scan: `chunk.may_hold(...)` (`server/src/world/simulation.rs:102`).
- Per-cell reads exist only in low-volume paths: collisions
  (`shared/src/world/chunk_map.rs:314`, `:334`, `:353`, `:371` — a dozen cells per entity per
  tick), raycast (`shared/src/world/raycast.rs`, 3 call sites), mob path/behaviour
  (`server/src/mob/*.rs`, ~15 call sites), worldgen (`server/src/world/generation.rs`, 2
  property reads; it mostly *writes* named blocks).

Outside `blocks.rs`/`palette.rs` there are ~70 property call sites in ~35 files. None takes a
registry parameter, and several run on worker threads (mesh tasks, generation pool
`server/src/world/pool.rs`). Threading a `&Registry` through all of them would be the single
largest cost of the project, for no gain.

### 1.2 The table

```rust
// shared/src/content/mod.rs
pub struct Content {
    pub generation: u32,
    pub blocks: Vec<BlockProps>,          // index = BlockId number
    pub block_names: Vec<&'static str>,   // "rustcraft:stone"
    pub block_by_name: HashMap<&'static str, BlockId>,
    pub items: Vec<ItemProps>, /* same three */
    pub recipes: Vec<Recipe>, pub smelting: ..., pub fuels: ...,
    pub mobs: [MobProps; MobKind::COUNT],
}

pub struct BlockProps {
    pub flags: BlockFlags,        // bitflags: USABLE, REPLACEABLE, TALL, GLASS, CONTAINER, ...
    pub visibility: BlockTransparency,
    pub light_opacity: u8,        // 255 = opaque
    pub light_emission: u8,
    pub break_time: u8,           // 0 = unbreakable
    pub tools: ToolMask, pub required: Option<RequiredTool>,
    pub hitbox: BlockHitbox, pub ray_hitbox: BlockHitbox,
    pub shape: Shape,             // Cube, Cross, Slab, Stairs, Door{..}, ... (today's Form + voxel.rs shapes)
    pub base: Option<BlockId>,
    pub behaviour: Behaviour,     // see 1.4
    pub drops: &'static [(u32, ItemId, u32)],
    pub item: Option<ItemId>,
    pub links: Links,             // toggled, top_half, lit, grows_into, hardened, sapling
    pub look: LookDef,            // face textures, tint, sway — read by the client only
}

static CONTENT: AtomicPtr<Content> = ...;
#[inline] pub fn content() -> &'static Content { unsafe { &*CONTENT.load(Ordering::Acquire) } }
pub fn install(new: Content) { CONTENT.store(Box::leak(Box::new(new)), Ordering::Release) }

impl BlockId {
    #[inline] pub fn props(self) -> &'static BlockProps { &content().blocks[self.index()] }
    pub fn light_emission(&self) -> u8 { self.props().light_emission }
}
```

- **Leaked snapshots instead of `ArcSwap`/`Res`**: a read is one atomic load plus one indexed
  load; it returns `&'static`, so names are `&'static str` with no extra trick and worker
  threads need nothing passed. A reload leaks one snapshot (order of 100 KB); reloads are a
  development action. The PoC leaks too (`registry.rs:361`, `:448`) but behind a `OnceLock`
  (`registry.rs:504`), which is what blocks reload.
- **Expected cost**: against a jump-table match, the table read is the same order (one
  bounds-checked load); for palette blocks it removes the `palette()` double dispatch. I expect
  no measurable change in meshing or lighting because those call per kind. This must be
  confirmed with the existing benches (`client/src/world/rendering/bench.rs`,
  `server/src/world/bench.rs`) in lot 1 — it is an expectation, not a result.
- A function that reads several properties in a loop should take `let c = content();` once so
  that one reload cannot be observed half-way through a mesh.
- `BlockLooks::get` (`meshing.rs:400`) must become bounds-safe (unknown number → the Debug
  look), because a mesh task may run with looks one generation older than the chunk data.

### 1.3 Naming specific blocks from engine code

Keep the spelling `BlockId::Stone`. After lot 4:

```rust
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct BlockId(u16);

engine_blocks! {            // generates assoc consts + ENGINE_NAMES
    0 Dirt "dirt", 1 Debug "debug", 2 Grass "grass", 3 Stone "stone", ...
}
```

This is the PoC's macro (`origin/poc/mod-blocks-and-recipes:shared/src/world/blocks.rs`,
`base_blocks!`), with `#[allow(non_upper_case_globals)]`. Consequences:

- Associated consts of a type deriving `PartialEq + Eq` are valid `match` patterns, so
  existing `match id { BlockId::Water | BlockId::Lava => .. }` compiles. What is lost is
  exhaustiveness checking; nearly every property match already ends in `_ =>`. The compiler
  will flag the few exhaustive ones (missing wildcard), which is the worklist.
- `id as usize` stops compiling: `client/src/i18n.rs:191`, `meshing.rs:401`, `:545` →
  `id.index()`.
- Load **fails** if a name of `ENGINE_NAMES` has no definition in the files: the engine can
  never hold a dangling const. This is the safety that replaces the enum.
- Only 113 of the 280 block variants and 134 of the 361 item variants are named anywhere
  outside the definitional files (`blocks.rs`, `palette.rs`, `items.rs`, `crafting.rs`,
  `furnace.rs`). The rest (about 60 %) are data-only today in all but form, and should
  *leave* the macro list in lot 7: a block is in the list only if Rust code names it.
- Rejected: resolving statics by name at load (`blocks::STONE: LazyLock<BlockId>`) — not
  usable in patterns, and 7000 call sites would change. Compile-time numbers for the engine
  list cost nothing because the save and wire formats stop depending on them (section 3).

### 1.4 Data versus behaviour

| Pure data (moves to files) | Where it is today |
|---|---|
| break time, faster tools, required tool, drops | `blocks.rs:1051`, `:1170`, `:1270`, `:1336` |
| visibility, light opacity, emission | `blocks.rs:1479`, `:1549`, `:1559` |
| collision and ray boxes | `blocks.rs:910`, `:976` (+ consts `:380-444`) |
| usable, replaceable, tall, glass, container, soil list, sapling | `blocks.rs:841-888`, `:1454`, `:1475` |
| shape + base + item of palette blocks | `palette.rs:443` (`Palette`, `:53`) |
| pairs: open/shut, door top, hardened powder | `palette.rs:630-720` |
| face textures, tint, sway | `client/src/world/rendering/voxel.rs:195-355` |
| break/step sounds, minimap colour | `client/src/audio/sounds.rs:38`, `client/src/ui/hud/minimap/colors.rs` |
| item stack size, damage, food, tool, durability, category, block placed | `items.rs:798-1370` |
| recipes, smelting, fuel | `crafting.rs:84-420`, `furnace.rs:40-130` |
| mob health, size, hunt, strike, food, model, clips | `mobs.rs:133-300`, `hostiles.rs:96-205`, `client/src/mob/models.rs:53-130` |

| Behaviour (stays Rust, selected by data) | Where |
|---|---|
| liquids | `shared/src/world/liquid.rs`, `server/src/world/flow.rs` |
| fire spread, burning | `shared/src/world/fire.rs` |
| growth stepping (the *stages* are data links) | `shared/src/world/growth.rs` |
| furnace tick, containers, signs, bedroll, anvil, engraving | `furnace.rs`, `container.rs`, `anvil.rs`, … |
| door halves, toggling, fence/wall/pane connection | `palette.rs:630-700`, `blocks.rs:890` |
| portals, mob AI, worldgen algorithms | `portal.rs`, `server/src/mob/`, `server/src/world/generation/` |

The attachment is a closed Rust enum named in the data, plus open tags:

```rust
pub enum Behaviour { None, Liquid, Fire, Furnace{lit: bool}, Container, CraftingTable, Sign,
    Bedroll, Crop, Sapling, Leaves, Door, Trapdoor, FenceGate, Falling, Torch, Campfire{lit: bool}, ... }
```

Engine code moves from identity tests to capability tests where the test is really a class:
`is_furnace()` (`blocks.rs:1470`) becomes `matches!(self.props().behaviour, Behaviour::Furnace{..})`;
`is_container()` (`:858`) reads a flag. Identity tests that are really about one block
(`BlockId::Water` in the mesher, `meshing.rs:710`) stay as they are. A data block that names
`behaviour: Furnace` gets a working furnace with no Rust; a *new* behaviour is a Rust change,
by design.

---

## 2. Data format and layout

### 2.1 Format: RON

RON, not TOML. The repo already speaks RON everywhere (`data/lang/*.ron`, `<Name>.anim.ron`,
`world.ron`, player files; `ron = "0.6"` in the three crates), and the definitions are serde
enums with payloads (`Shape::Door{top, open}`, hitbox boxes), which RON writes directly. The
PoC's `blocks.toml` (209 lines, 29 blocks) converts mechanically; its field set
(`break_steps`, `tools`, `needs_tool`, `drops`, `look`, `collision`, `outline`, `light`,
`usable`, `replaceable`) is a good starting vocabulary and should be kept.

### 2.2 Layout: one file per feature

```
data/content/rustcraft/
  terrain.ron  wood.ron  jungle.ron  palette.ron  forge.ron  farming.ron  mobs.ron ...
```

The loader reads every `*.ron` of the folder; file names carry no meaning; a name defined
twice is an error naming both files. A feature file:

```ron
(
  blocks: {
    "jungle_log": (break: 10, tools: [Axe], faces: (top: "JungleLogTop", bottom: "JungleLogTop"),
                   tags: ["log", "flammable"], name: (en: "Jungle Log", fr: "Bûche de jungle")),
    "jungle_leaves": (break: 2, look: Transparent, tint: Foliage, behaviour: Leaves,
                      sapling: "jungle_sapling", drops: []),
    "lit_furnace": (like: "furnace", light: 13, behaviour: Furnace(lit: true), item: None),
  },
  items: { "cocoa_beans": (stack: 64, places: "cocoa") },
  recipes: [ (out: "jungle_planks", count: 4, shapeless: ["jungle_log"]) ,
             (out: "lamp", pattern: ["CTC","TKT","CTC"], key: {'C': "cobblestone", 'T': "torch", 'K': "coal"}) ],
  smelting: [ ("jungle_log", "charcoal") ],
  fuel: { "#planks": 300 },
)
```

Rules that keep files short:

- **Defaults**: a block with no fields is a full solid cube, textured `<PascalName>.png` on
  six faces, dropping itself, with an item of the same name that places it. The item needs no
  entry. (This is today's implicit rule: texture named after the variant —
  `voxel.rs:455` — and "an item with no texture shows the block's" — `materials.rs:303-310`.)
- **`like: "x"`** copies another definition before applying the fields — this is `Palette.base`.
- **Names**: local ids are snake case (`jungle_log`), namespaced by the folder
  (`rustcraft:jungle_log`). The PascalCase form (`JungleLog`) is derived mechanically — the
  conversion exists (`items.rs:771-796`) — and stays the texture file name, the lang key and
  the name in RON saves, so no PNG, lang line or save is renamed.
- **Display names inline** (`name: (en:, fr:)`), falling back to `item.<Pascal>` in
  `data/lang/*.ron`. The 361 existing `item.*` lines stay where they are; new content does
  not touch the two lang files.
- **Tags** (`"#planks"`) replace `ItemTag` (`crafting.rs:27-60`) and the hand lists such as
  `fire_wood` (`furnace.rs:114`).

### 2.3 Families (the palette)

`palette.rs` is 1326 lines of per-wood / per-rock / per-colour matches plus `palette_recipes()`
(`crafting.rs:355-411`), which already loops `Wood::ALL`, `Rock::ALL`, `DyeColor::ALL`. That
loop is the template:

```ron
families: [
  ( each: {"w": ["oak", "spruce", "jungle", "willow", "acacia"]},
    blocks: {
      "{w}_slab":       (like: "{w}_planks", shape: Slab),
      "{w}_stairs":     (like: "{w}_planks", shape: Stairs),
      "{w}_door":       (like: "{w}_planks", shape: Door(top: false, open: false),
                         top: "{w}_door_top", toggled: "open_{w}_door", faces: (all: "{W}Door")),
    },
    recipes: [ (out: "{w}_slab", count: 6, pattern: ["PPP"], key: {'P': "{w}_planks"}) ] ),
]
```

Expansion is textual substitution on the parsed `ron::Value` before typed deserialisation
(`{w}` snake, `{W}` Pascal). A new wood is one word in a list plus textures. Irregular names
(`Door` for the oak door, `palette.rs:108`) are handled by `aliases` (section 3.4).

### 2.4 Mobs

`MobKind` stays an enum (24 variants, `mobs.rs:14`; ~740 references; behaviour-heavy).
`mobs.ron` holds, per kind name: `health`, `size`, `hunt (sight, forget, speed)`,
`strike`, `eats`, `drops`, `tameable`, `model`, `clips` (action → clip), `lift`, `sounds`
folder, `ai` (archetype: `Grazer | Hunter | Villager | Hostile(..)`). Herds are already data
(`Herd` in `BiomeDef`, `biome.rs:235`). Hostile spawn rules (`hostiles.rs:315-346`) are a
decision tree with shared rng draws; they stay code until someone needs otherwise. GLB models
and animations already hot-reload in principle through Bevy's asset server once the
`file_watcher` feature is on; the Python generators are unchanged.

A new mob kind remains a Rust change (enum variant + AI choice). It gets cheaper because the
stats, model table and clip table no longer need match arms in three crates.

---

## 3. Id stability

### 3.1 Three numberings, two of which disappear from contracts

| | Who fixes it | Lifetime |
|---|---|---|
| **Name** `rustcraft:jungle_log` | the content file | forever; the only contract |
| **Runtime number** | engine list at compile time (0..E), then data blocks in sorted-name order at first load; appended on reload, never renumbered in a process | one process |
| **Saved number** | the world's own list, append-only per world | one world |

### 3.2 Saves

- `world.ron` (or a sibling `ids.ron`) gains `block_names: Vec<String>` and `item_names:
  Vec<String>`: position = saved number.
- Chunk files keep their encoding (bincode + lz4, `shared/src/codec.rs`, regions in
  `server/src/world/chunk_files.rs`). After decoding, `chunk.remap(&saved_to_runtime)`
  rewrites only the chunk's palette entries (`chunk.rs:45`, ≤256) — O(kinds), skipped when the
  table is the identity. Before encoding, the inverse. Unknown runtime ids met while saving
  are appended to the world's list.
- Container and furnace files (`server/src/world/container_files.rs:107-114`, bincode) get the
  same remap for `ItemId`. Player files and `world.ron` are RON: `BlockId`/`ItemId` serialise
  as their name when `is_human_readable()` (as the PoC does), so they need no table.
- **Migration**: a world with no list is read with `LEGACY_BLOCK_NAMES` / `LEGACY_ITEM_NAMES`,
  the frozen 280 / 361 names in today's enum order (generated once from `BlockId::ALL`
  `blocks.rs:547` and `ItemId::ALL`, and pinned by the existing `block_encoding_is_pinned`
  test, `blocks.rs:1765`, which becomes "the legacy list never changes"). No chunk is
  rewritten. `FORMAT_VERSION` goes 5 → 6 (`server/src/world/format.rs:33`) because an older
  build would misread a world with data blocks; the reader accepts `2..=6`
  (`load_from_file.rs:156`).

### 3.3 Network

- `AuthRegisterResponse` (`shared/src/messages/auth.rs:42`) gains `content: ContentSnapshot`:
  the names in runtime order plus every definition. Size estimate: a few hundred definitions
  of ~100-200 bytes, well under the 64 MiB decode bound (`codec.rs:31`) and small after lz4;
  to be checked against the auth channel's limits (`shared/src/channels.rs`).
- The client **adopts the server's numbering** for everything above its engine list, and
  **verifies** that the first E names equal its own `ENGINE_NAMES`. A mismatch is refused with
  the first differing name in the message. No remap on the wire.
- `PROTOCOL_ID` (`crates/protocol/src/lib.rs:11`) then bumps only when a message struct or a
  non-content enum changes. Content stops driving it (27 bumps in 5 weeks today). Adding an
  engine-named block is caught by the name check, with a better error than "wrong protocol".

### 3.4 Removed and renamed content

- **Removed** (file deleted, or a world opened without the pack): the name stays in the
  world's list; its runtime definition is a placeholder (Debug texture, breakable, drops
  nothing, name shown as the raw id). It is saved back under its original name, so restoring
  the file restores the blocks. This is the PoC's "become debug blocks" behaviour
  (`BlockId::of_mod`) without the data loss on save.
- **Renamed**: `aliases: ["old_name"]` on the new definition; the world's list resolves the
  old name to the new block and is rewritten at the next save.
- During a hot reload a removed block is tombstoned the same way; its number is never reused
  in that process.

---

## 4. Hot reload, end to end

### 4.1 Server

1. A `notify` watcher thread (debounced ~200 ms) on `data/content/` sends "changed" over a
   channel. The server is `MinimalPlugins` with no asset server (`server/src/init.rs:190`),
   so `notify` directly is simpler than adding `AssetPlugin`. A `/reload` chat command does
   the same by hand.
2. A system reads all files, expands families, resolves names against the **current** runtime
   numbering (existing names keep their number; new ones append; missing ones tombstone), and
   validates: every engine name defined; every reference (`like`, `toggled`, drops, recipe
   items, smelting) resolves; boxes inside the cell; light ≤ 15; no duplicate names.
3. On error: nothing changes; the error (file, key, reason) goes to the log and as a chat
   message to the players. On success: `install(new)`, generation + 1, then
   `ServerToClientMessage::Content(ContentSnapshot)` on a reliable channel.
4. Server state to refresh: scheduled ticks and growth entries whose block lost its behaviour
   (`shared/src/world/ticks.rs`, `growth.rs`); `Furnace` instances if smelting changed (they
   re-evaluate on their next tick, `furnace.rs:160`); the Luau layer's name lookup
   (`server/src/mods.rs:185-190` resolves through `ItemId::from_name`, so it follows
   automatically). Recipes stop being a `OnceLock` (`crafting.rs:415-418`) and live in
   `Content`.

### 4.2 Client

On `Content` (and at join):

1. Remote server: `install(snapshot)`. Solo: nothing to install (see 4.3).
2. Re-run the texture build: `setup_materials` (`materials.rs:289`) and `create_all_atlases`
   (`:408`), whose "already built" gate is `material_resource.blocks.is_none()` (`:429`) —
   reset it. This rebuilds the block texture array, its table, `BlockLooks`
   (`materials.rs:607`, `meshing.rs:389`) and the item atlas (`materials.rs:615-700`).
   `ChunkMaterial` assets must be updated in place (their `layers` and `table` handles) rather
   than recreated, as the chunk entities hold the handles.
3. `respawn_every_group` (`render.rs:915`, today bound to the `ReloadChunks` debug key,
   `:908`) remeshes everything; lighting is recomputed by the mesh jobs, so a changed
   `light`/`look` shows.
4. Caches to make reloadable: `item_names()` `OnceLock` (`client/src/i18n.rs:91-104`); the
   item browser catalog (`client/src/ui/hud/item_browser/catalog.rs`); block item meshes
   (`meshing.rs:230`); held-item and dropped-item visuals; minimap colours
   (`client/src/ui/hud/minimap/colors.rs`); per-block sounds (`client/src/audio/sounds.rs:38`).
   A `ContentGeneration(u32)` resource, changed on install, is what these systems watch
   (`resource_changed`).
5. **Textures** are client files, not registry data: enable Bevy's `file_watcher` feature (it
   is not enabled today — no `file_watcher`/`notify` anywhere in the workspace) and, on
   `AssetEvent::<Image>::Modified` for a handle of `AtlasHandles` (`materials.rs:270`), do
   steps 2-3. This needs no server and no protocol change: it is lot 0.

A property-only reload (hardness, drops, recipe) needs no remesh: skip steps 2-3 unless a
`look`, shape, box, light or texture name changed (compare old and new `BlockProps` per id).

### 4.3 Solo mode

The solo server is a thread of the client process (`client/src/network/setup.rs:221`,
`LocalServerHandle`; joined in `cleanup.rs:258`). With a process-global snapshot the two
sides share it. Consequences, all acceptable but they must be written down:

- In solo the **server thread is the only writer**. The client sees new properties
  immediately and receives the message a moment later, at which point it rebuilds looks and
  remeshes. In between, mesh jobs may pair new properties with old looks: harmless because
  numbers never move within a process and `BlockLooks::get` is bounds-safe.
- Opening another world in the same process may bring a different data set: the runtime
  numbering is rebuilt at world start. Nothing of the previous world may survive that —
  `ClientWorldMap` is already dropped on leave; any cache keyed by `BlockId` that outlives a
  session must key on the generation.
- A client that leaves a solo world and joins a remote server installs the remote snapshot
  over the local one: correct, since the local server has stopped (`join_local_server`).
- Tests run many "servers" and "clients" in one process (`client/src/e2e/`). They must all
  use the default content, and tests of reloading must work on `Content` values
  (`Content::load(dir) -> Result<Content>`, pure) instead of installing globally. One
  dedicated e2e test, serialised, covers the real install path.

### 4.4 Where the files come from

Runtime path through `GameFolderPaths` (as textures today). Not embedded with `include_str!`:
embedding would make every content edit recompile `shared` and its dependants on the next
`cargo` invocation, which is the cost being removed. Tests find the folder through
`env!("CARGO_MANIFEST_DIR")`; `content()` loads it lazily on first use so that no test needs
setup.

---

## 5. Migration plan

Each lot is mergeable alone and leaves the suite green. Estimates are agent-days of an agent
session on this repo, including tests; they are judgment calls, not measurements.

| # | Lot | Est. | Protocol / format | Delivers |
|---|---|---|---|---|
| 0 | **Texture hot reload** (client only): `file_watcher`, rebuild array + `BlockLooks` + item atlas on image change, `respawn_every_group`. Extract "rebuild textures" as a function for lot 2. | 0.5-1 | none | Edit a PNG, see it in game |
| 1 | **Property table, generated from the match arms.** `Content`, `BlockProps`, `content()`, `install`. `Content::builtin()` fills the table by calling today's functions (renamed `builtin_*`); public getters read the table. Same for items. Run the mesh / worldgen benches before and after. | 2 | none | Nothing visible; all reads go through one seam |
| 2 | **Override files + live reload.** `data/content/rustcraft/*.ron` may override pure-data fields of existing blocks/items by name. Server watcher, validation, `Content` message at join and on change, client install + selective remesh, `/reload`. | 2-2.5 | `PROTOCOL_ID` +1 (last content-driven bump) | **Edit hardness / light / drops / tools in a file, see it without restart** |
| 3 | **Files become the source for block and item data.** A one-shot dump writes the full files from `builtin()`; a test asserts files == builtin; then delete the match arms (`blocks.rs` ~1000 lines, `items.rs` ~600). `Behaviour` enum and flags introduced here. Face textures / tint / sway from `voxel.rs:195-355` move to `look`. | 3 | none | Agents edit data, not matches, for existing-shaped content |
| 4 | **`BlockId(u16)`** with `engine_blocks!` consts; `index()`; name serde in RON; world name list + chunk palette remap; legacy list; join name check. Data-only blocks become possible (placed by commands, worldgen, Luau). | 3 | `FORMAT_VERSION` 6; `PROTOCOL_ID` +1 | New block with no Rust (no item yet) |
| 5 | **`ItemId(u16)`**, same treatment; implicit item per block; container / furnace file remap; item browser and i18n by table; remove the `"… / 361"` literals (`client/src/ui/hud/item_browser/display.rs:875-995`, `client/src/e2e/item_browser.rs:168-422`). | 3 | `PROTOCOL_ID` +1 | **New placeable, craftable-later block from a file, live** |
| 6 | **Recipes, smelting, fuel in data**, reloadable; tags. Replaces `RECIPES`, the `OnceLock` and the furnace matches. | 2 | none | Recipes edited live |
| 7 | **Families**: template expansion; convert `palette.rs` (1326 lines) and `palette_recipes`; shrink the engine lists to the names Rust really uses (113 blocks / 134 items today). | 2 | none | A new wood / rock / colour = one word + textures |
| 8 | **Mob data**: stats, hunt, strike, drops, model, clips, sounds in `mobs.ron`; `MobKind` stays. | 2 | none | Mob tuning live; fewer files per new mob |
| 9 | **Agent ergonomics**: `docs/CONTENT.md` (schema + examples), `--check-content` CLI that validates without starting a world, CLAUDE.md note, error messages with file/key. | 1 | none | Agents validate content in about a second, no build |

Total about 20-21 days. Lots 0-2 (about 5 days) can stop there and still pay for themselves
for tuning work. Lots 6 and 8 depend only on lot 2's machinery plus (for 6) lot 5.

### 5.1 Is the strangler worth it? Yes.

Compared with going straight to `BlockId(u16)` as the PoC did:

- The PoC branch forked at `2b31d811` (2026-10-20), when there were 29 blocks; `main` is 601
  commits and 251 blocks ahead. Its `blocks.rs` diff (581 lines changed) cannot be rebased;
  the ideas carry over (const macro, name check at load, TOML field set, tombstone as Debug,
  the Mods screen), the code mostly does not.
- Lots 1-3 touch `blocks.rs`, `items.rs`, `voxel.rs` *bodies* only. No signature, no id, no
  save, one protocol bump. They can merge while other feature branches are in flight; a
  conflicting branch that adds a match arm gets a failing "files == builtin" test telling it
  exactly which entry to add to a file.
- Lot 4 then changes the *type* behind an unchanged spelling, on a `blocks.rs` that has
  already lost its matches — a much smaller diff than the PoC's.
- The cost of the strangler is lot 1's throwaway `builtin()` glue (about a day) and the fact
  that new blocks still need an enum variant until lot 5. That is the right trade: first
  value in days instead of two weeks, and each step is reversible.

---

## 6. Risks

| Risk | Why it is real | Mitigation |
|---|---|---|
| **Hot-path regression** | Every property read gains an atomic load | Hot loops are per kind (1.1); hoist `content()`; run both bench suites in lot 1 before merging. Unverified until then. |
| **Global state in tests** | One process hosts many test worlds; a test installing content would poison others | Tests never install; reload logic is pure on `Content` values; one serialised e2e for the install path |
| **Loss of exhaustive matches** | After lot 4 a new block is silently handled by `_ =>` | By then defaults live in data, where "unspecified" is explicit and validated; behaviour tests go through `Behaviour`, which *is* an exhaustive enum |
| **Worldgen determinism** | Generation reads properties (`generation.rs`, 2 sites) and writes ids; data-block numbers depend on load order | Sorted-name numbering; never hash or order by number in generation; `worldgen_dump` (`server/examples/worldgen_dump.rs`) compares names, not numbers. Editing a property legitimately changes chunks generated afterwards. |
| **Goldens / pinned tests** | 38 UI goldens (`client/tests/golden/`), item-count literals, 58 uses of `::ALL`, encoding pin tests | Lots 1-3 change no value, so goldens must stay byte-identical — that is the acceptance test. `ALL` becomes `BlockId::all()` (iterator over the table) in lots 4/5. |
| **The ~7000 references** | 2973 `BlockId::X`, 3885 `ItemId::X` | Spelling preserved by assoc consts; only `as usize` (3 sites) and exhaustive matches need edits. About 3500 of the references sit in the five definitional files (`blocks.rs`, `palette.rs`, `items.rs`, `crafting.rs`, `furnace.rs`) that lots 3, 6 and 7 largely delete. |
| **Half-applied reload** | A file saved mid-edit, or a reference to a block deleted in the same edit | Debounce; validate the whole set; all-or-nothing install; keep last good snapshot |
| **Remote clients lack textures** of server-only data blocks | Textures are client files | Out of scope: such a block draws as Debug. Solo and same-checkout play (the development loop) are unaffected. Asset download is a later feature. |
| **Save corruption on palette bug** | The remap sits on the save path | Round-trip property test (random chunk × random tables); legacy worlds take the identity path; keep a v5 fixture world in tests |
| **`ron 0.6`** is old | Error messages with positions are weak in 0.6 | Acceptable for lot 2; upgrade `ron` in lot 9 if messages prove poor (not checked) |
| **Scope creep into a mod system** | The PoC bundles mod.toml, dependencies, a Mods screen | Keep that as a separate track on top of this registry; the namespace folder is the only hook needed now |

### 6.1 What changes for an agent

| Feature | Today | After |
|---|---|---|
| **Jungle wood** (`b47456a9`: logs, planks, leaves, recipes) | 29 files; Rust in 12: `blocks.rs`, `items.rs`, `crafting.rs`, `voxel.rs`, `sounds.rs`, item browser (3), `commands.rs`, `constants.rs`, `worldgen_dump.rs`, e2e; + `PROTOCOL_ID`, counts in tests, 2 lang files | `data/content/rustcraft/jungle.ron` + PNGs. No Rust, no protocol bump, no count to fix, no rebuild. Rebase conflicts: none (new file). |
| **Palette** (`0f283217`) | 208 files, +3714 lines, 2804 in `shared/src`; 30 non-image files | `palette.ron` with three families (order of 150 lines) + the 177 PNGs + `generate_palette.py`. The signs-on-screen part of that commit stays Rust. |
| **Horses + boats** (`1932a822`, 50 files) | enum variant + matches in `mobs.rs`, `hostiles.rs`, `models.rs`, `sounds.rs`, i18n | Riding/rowing logic stays Rust; stats, models, clips, sounds, drops go in `mobs.ron`. Rough guess: a third fewer files. |
| **Tuning** (hardness, light, drop rates, recipe) | edit Rust, rebuild 3 crates, restart, walk back | save the file; visible in the running game |
| **Rebase of a content branch** (38 min measured for jungle) | renumber append-only ids, `PROTOCOL_ID`, counts | nothing to renumber; names do not collide |

The verification loop also changes: `--check-content` (lot 9) validates a content edit
without compiling, and an agent can leave one game running across many edits.

---

## 7. Open points for the owner

1. **RON vs the PoC's TOML** — I recommend RON for consistency; if the teammate's mod track
   has external authors in mind, TOML is defensible and the loader cost is the same.
2. **Inline display names** vs keeping every string in `data/lang/*.ron` — inline is fewer
   touch points and no merge conflicts; it splits translations across files.
3. **Whether lot 0-2 is enough for now.** If the pain is mostly tuning, stop after lot 2 and
   revisit. If it is mostly "agents edit too many files per feature", lots 3-7 are the payoff.
4. **Coordination with MizKyosia**: lots 4-5 supersede the PoC's `registry.rs`; its mod
   manifest, dependency order and Mods screen can sit on top unchanged in concept.
