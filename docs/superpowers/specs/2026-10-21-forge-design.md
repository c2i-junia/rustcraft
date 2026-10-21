# Forge and progression — design

Date: 2026-10-21. Branch `feat/forge`, worktree `feat-forge/`, from main `fa684e8f`.

The user picked direction A, "Forge et progression", of the roadmap artifact
(https://claude.ai/artifact/JLHeUcdEQAJuCKxVzncwau) and delegated every decision. This file
records them; later rulings are appended under **Decisions**.

## Goal

Give survival a loop: mine, smelt, equip, dig deeper. Today the furnace can be placed but does
nothing, iron and gold ores have no use, and every tool is wooden.

## Scope

In:

1. A working furnace: input, fuel and output slots, smelting over time on the server, a lit
   furnace that glows, kept in the save.
2. Its dialog in the client.
3. Progression: ingots, charcoal, diamonds and diamond ore, stone / iron / gold / diamond tools,
   mining tiers, storage blocks, every recipe; smelting in the item browser.
4. The bucket: take a water source, pour it elsewhere.

Out, on purpose:

- **Hunger.** The game has no food source yet: meat needs combat (direction B), bread needs
  crops (direction C). A hunger bar with nothing to eat only punishes. It moves to B/C.
- **Armour.** The only damage is falling. Armour belongs with combat (direction B), where it has
  something to stop; it moves there.
- Lava, hoppers, experience, furnace minecarts, blast furnace, smoker.

## Lots

Each lot gets its own plan in `docs/superpowers/plans/` and is built test first. `just check`
green at the end of each lot. Commits follow the repo's Conventional Commits, in English, no
attribution trailer.

### Lot 1 — the furnace, shared and server

**Model.** `shared/src/world/furnace.rs`:

```rust
pub struct Furnace {
    pub input: Option<ItemStack>,
    pub fuel: Option<ItemStack>,
    pub output: Option<ItemStack>,
    /// Ticks the fuel being burnt still lasts; 0 when the furnace is out.
    pub burn_left: u16,
    /// Ticks the fuel being burnt lasted in full, for the flame of the dialog.
    pub burn_total: u16,
    /// Ticks the item in `input` has cooked, up to `COOK_TICKS`.
    pub cook: u16,
}
```

- `COOK_TICKS = 200` (10 s at 20 ticks/s), as Minecraft.
- `smelting_result(ItemId) -> Option<ItemId>` and `fuel_ticks(ItemId) -> Option<u16>`, tables in
  shared, so the client (dialog, item browser) and the server agree.
- `Furnace::tick(&mut self) -> FurnaceChange` (pure, unit-tested), Minecraft's rules:
  - it *can smelt* when `input` has a result and `output` is empty or holds that result with
    room for one more;
  - when `burn_left == 0` and it can smelt and holds fuel, one fuel item is burnt:
    `burn_left = burn_total = fuel_ticks`;
  - while `burn_left > 0` it decreases by one each tick (fuel burns even if the input is taken
    out, as Minecraft);
  - while burning and able to smelt, `cook += 1`; at `COOK_TICKS` one input item becomes one
    result in `output`, `cook = 0`;
  - while not burning, `cook` goes back 2 per tick down to 0; when the input cannot smelt,
    `cook = 0`.
  - The returned change says whether anything changed and whether it went lit ↔ out.
- Fuel table (ticks): Coal and Charcoal 1600 (8 items); logs, planks, crafting table, chest 300;
  wooden tools 200; stick 100. Lava bucket: none (no lava).
- Smelting table: IronOre → IronIngot, GoldOre → GoldIngot, DiamondOre → Diamond, Sand → Glass,
  Cobblestone → Stone, every log → Charcoal. Lot 1 adds only what already exists
  (Sand → Glass, Cobblestone → Stone) plus the entries lot 3 completes; the table is one `match`
  so lot 3 extends it.

**Block.** A new `BlockId::LitFurnace`, appended: same hardness, tool, drop (the Furnace item)
and usability as `Furnace`, light emission 13, front texture `furnace_front_on`. The server
swaps `Furnace` ↔ `LitFurnace` (keeping `direction`) when the furnace lights or goes out, so the
clients learn it through the block updates they already receive. Both count as "a furnace" for
reach checks. The furnace faces the player who placed it (check how the placement sets
`direction`; do it if it does not).

**Storage.** `ChunkMap` gets `furnaces: HashMap<IVec3, Furnace>` and a changed set, like the
chests' `containers`. Placing a furnace creates an empty one; breaking it drops its three
stacks like a chest drops its contents. Saved in `furnaces/<chunk>.bin` in the world folder,
with the machinery of `container_files.rs` made generic over the stored type rather than
copied. `FORMAT_VERSION` is unchanged: an older world has no `furnaces/` folder, which means no
furnace state, and its furnaces start empty.

**Ticking.** A server system in the simulation set ticks every furnace of the loaded chunks
once per tick, marks the changed ones for the save and for the players who have them open, and
swaps the block on lit ↔ out. Furnaces in unloaded chunks do not tick (as Minecraft).

**Inventory actions.** Appended `InventoryAction::OpenFurnace(IVec3)` (index 11).
`Inventory::open_furnace: Option<IVec3>`; opening it closes a table or chest and vice versa.
Three furnace slots numbered after the chest slots (`furnace_slots()`, input, fuel, output),
lent to the inventory while an action applies, like the chest's. Rules:

- input takes anything; fuel takes only fuels; output takes nothing (take only);
- shift-click from the inventory: a smeltable item goes to input, else a fuel goes to fuel,
  else nothing; shift-click from a furnace slot goes back to the inventory;
- `Close` closes the furnace too; a furnace walked away from or broken closes.

**Network.** `PlayerUpdateEvent` gains `open_furnace: Option<Furnace>`, sent to its player only
while open, like `open_chest`; the client reads progress from it. `PROTOCOL_ID` 23 → 24, once
for the whole branch: later lots that change the wire do not bump again.

Tests: tick rules (lights, burns out, cooks, decays, output full, fuel lasts across items),
slot rules, shift-click routing, server ticking + block swap, save/load round trip, a broken
furnace drops its stacks, the pinned encodings (new action, new block, new field).

### Lot 2 — the furnace dialog, client

- Right click on a furnace (lit or not) opens the inventory with a furnace section where the
  chest section goes: input slot, flame, fuel slot under it, arrow, output slot. Same
  pixel-craft style as the rest of the redesigned UI; the item browser stays beside it.
- The flame shows `burn_left / burn_total`, the arrow `cook / COOK_TICKS`, both drawn from the
  synced `Furnace`; they may be smoothed between updates but never run ahead of the server by
  more than one update.
- Clicks, right clicks, shift-clicks, number keys and the keyboard navigation of the inventory
  work on the three slots. Client prediction applies the same shared rules as the server.
- Lit furnace: `furnace_front_on` texture, light 13 (from `light_emission`).
- Tests: e2e (place a furnace, put iron ore and coal, see the ingot come out), golden of the
  dialog (empty and mid-smelt), the screen review snapshots still pass.

### Lot 3 — progression

**New items, appended to `ItemId` in this order:** IronIngot, GoldIngot, Diamond, Charcoal,
DiamondOre, IronBlock, GoldBlock, DiamondBlock, then for each of Stone, Iron, Golden, Diamond:
Pickaxe, Axe, Shovel, Hoe, Sword. **New blocks, appended to `BlockId`:** DiamondOre, IronBlock,
GoldBlock, DiamondBlock.

**Tiers.** `ToolTier { Wood, Stone, Iron, Diamond, Gold }` drives `Tool`:

| Tier | speed | durability | mining level |
|---|---|---|---|
| Wood | 2 | 59 | 0 |
| Stone | 4 | 131 | 1 |
| Iron | 6 | 250 | 2 |
| Diamond | 8 | 1561 | 3 |
| Gold | 12 | 32 | 0 |

Each block that needs a pickaxe gets a required mining level: stone, cobblestone, coal ore,
furnace 0; iron ore, iron block 1; gold ore, diamond ore, gold block, diamond block 2. A block
broken with a lower level breaks but drops nothing (the existing `harvested` path).

**Drops.** Diamond ore drops a diamond; iron and gold ore drop themselves (to be smelted).

**Generation.** Diamond ore in `ores.rs`, after gold: heights 5–16, 1 attempt per chunk,
clusters of 2–6, no veins. Check the share with the existing generation bench or dump.

**Recipes.** Every tool of every material with the wooden tools' shapes (cobblestone for stone),
9 ingots / diamonds ↔ 1 storage block, torches from charcoal too.

**Smelting** completes the lot 1 table (ores, logs → charcoal).

**Item browser.** A smelting card in the recipe view (input → output, with the flame), the uses
of an item list its smelting, and a fuel lists "burns N items" in its uses. Filed in the right
categories (ingots and diamond: Materials; tools: Tools; storage blocks and ore: Blocks).

**Textures** from Pixel Perfection, per `CLAUDE.md`, credited in the commit and
`docs/ressources.txt`.

### Lot 4 — the bucket

- Items appended: Bucket (stacks to 16), WaterBucket (stacks to 1). Recipe: three iron ingots
  in a V.
- Right click with a bucket on a water source (level 0) within reach: the source is removed,
  one bucket becomes a water bucket (into the hand when the stack was one, else into the
  inventory or dropped). The raycast must be able to aim at water for this.
- Right click with a water bucket: a water source is placed against the aimed face as a block
  would be, and the flow simulation takes over; the water bucket becomes a bucket.
- Same in Creative and Survival. Predicted by the client like block placement.

### Closing

README (features, controls), `docs/ARCHITECTURE.md` (furnace ticking, `furnaces/` files),
decisions below.

## Decisions

1. Hunger and armour are out of this branch (see Scope).
2. `LitFurnace` is a block of its own rather than a block state: the meshing, the light and the
   block updates already work per `BlockId`.
3. Furnaces are stored apart from chests (`furnaces/`), so old worlds load without a format bump.
4. One `PROTOCOL_ID` bump (24) for the branch.
