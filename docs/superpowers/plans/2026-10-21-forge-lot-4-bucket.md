# Forge Lot 4 (The bucket) and Closing Implementation Plan

**Goal:** Three iron ingots in a V make a bucket. A right click with a bucket on a water source
within reach takes the source away and fills the bucket; a right click with a water bucket
pours a source against the aimed face, as a block is placed, and the flow spreads it; the water
bucket becomes a bucket again. The same in Creative and Survival, predicted by the client like
a placed block, with a splash. Then the branch is closed: the README, `docs/ARCHITECTURE.md`
and the changelog tell of the forge, the hearts no longer cover the open inventory, and the
whole branch is read over once. To see: craft a bucket, pour water from it next to you, take it
back.

**Spec:** `docs/superpowers/specs/2026-10-21-forge-design.md`, "Lot 4 — the bucket" and
"Closing", and its "Decisions" (1 to 32 from lots 1 to 3).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments.
- TDD: each test is written and seen failing (not compiling counts) before the code that
  makes it pass.
- `ItemId` and the recipe table are append only: `Bucket` then `WaterBucket` at the end of
  `ItemId` (70 and 71), the bucket's recipe at the end of the table; the pinned encoding tests
  gain their lines. `PROTOCOL_ID` stays 24 (bumped once for the branch in lot 1).
  `FORMAT_VERSION` is unchanged.
- The rules of the bucket are shared (`shared/src/players/blocks.rs`), run by the client as a
  prediction and by the server as the authority, like the placing of a block. Breaking and
  placing blocks keep ignoring water.
- `just test-fast` while working; `timeout 1800 just check` at the end.

## Decisions (where the spec is silent)

1. **Items.** `Bucket` stacks to 16, `WaterBucket` to 1, as Minecraft; both `ItemType::Generic`,
   filed under Tools (Minecraft's "Tools & Utilities"): the browser's Tools are no longer only
   the items with a `tool()`. Neither burns nor smelts.
2. **Recipe**, appended: three iron ingots in a V, `I.I / .I.`, three wide, so on a crafting
   table only. It makes one bucket.
3. **Aim.** An empty bucket aims at the first water source or block on the ray, a source hit as a
   whole block; flowing water stays transparent to it, as Minecraft's `SOURCE_ONLY`. Every other
   item aims through water, as today. The aim depends on the held item only, and is the same for
   the outline, the right click and the left click: a bucket aimed at a source does not break
   the block under it (water is unbreakable). `raycast_to_water_source` in `raycast.rs`, and
   `aim(world_map, camera, position, view_mode, held)` in `players/blocks.rs`, which the client
   calls too.
4. **Filling.** On a source within `INTERACTION_DISTANCE`: the source is removed, and the flow
   drains or refills the cell from its neighbours as for any removed block. A stack of one bucket
   becomes the water bucket in place; from a larger stack one bucket goes, and the water bucket
   goes into the inventory (`add_item_to_inventory`), else the server drops it, thrown ahead of
   the player like a dropped item (`BlockInteractions::dropped`). Aimed at a block, the bucket
   does nothing (`Refusal::NotASource`).
5. **Pouring** follows `place_block`: against the aimed face, into air or into a replaceable
   cell, that is water. Flowing water there becomes a source. A cell already holding a source is
   `Occupied`, and the water bucket is kept: pouring would change nothing. A decoration (tall
   grass, a torch...) is not replaceable, so the cell is `Occupied`, as for a block. Water has no
   collision box: unlike a block, it may be poured where the player stands. The water bucket
   becomes a bucket in its slot.
6. **Usable blocks.** As for a block: a bucket or a water bucket aimed at a crafting table, a
   chest or a furnace opens it, unless the player sneaks.
7. **Once per press.** A block is placed again every `PLACE_REPEAT_MS` while the right button is
   held; a bucket acts once per press, or holding the button would pour back what it just took.
   After a bucket acts, `place_wait_ms` waits for the release.
8. **Sound.** A water source that appears or goes where there was air or water is a bucket: the
   clients play the splash of `data/sounds/liquid/splash.ogg`, made by `generate_sounds.py` and
   never played until now (`Sound::Splash`). The flow never makes nor removes a source, so it
   stays silent.
9. **Textures** from Pixel Perfection: `item/bucket.png` and `item/water_bucket.png`, 16x16, as
   `items/Bucket.png` and `items/WaterBucket.png`. No model in the hand: the game draws none.

Closing:

10. **README**: the features (the furnace, smelting and fuels, the tool tiers and mining levels,
    diamonds, the storage blocks, the bucket), the Use / Pour line of the controls (right click,
    unchanged keys), and the `furnaces/` folder in Saves.
11. **`docs/ARCHITECTURE.md`**: the furnaces in the tick order (after the inputs), the generic
    contents store and the `furnaces/` files, the open furnace in `PlayerUpdateEvent`, the
    bucket in the prediction section; the known limit of a full inventory names the bucket's
    drop.
12. **`CHANGELOG.txt`** is kept by hand on main (each feature commit adds its line, lots 2 and 3
    did): a line for the bucket.
13. **Hearts.** Minecraft hides them under an open inventory: `update_health_bar` shows them in
    survival while `InventoryRoot` is hidden only, whatever dialog the panel shows (plain,
    table, chest, furnace). Test first, in the survival e2e test.
14. **Review of the branch** (`git diff fa684e8f..HEAD`): stale "lot N" notes, dead code, TODOs.

## Tasks

### Task 1: The bucket items and recipe (`shared`)
- [ ] Tests (items.rs): pinned encodings 70 and 71; stacks of 16 and 1; Tools; names
  `bucket` and `water_bucket`; the Tools category is the tools and the buckets. (crafting.rs):
  the V makes a bucket, not in the 2x2 grid; the count of recipes (57) and the uses of an iron
  ingot. See them fail.
- [ ] `ItemId::Bucket`, `WaterBucket`, `ALL`, `get_max_stack`, `category`, `get_default_type`,
  the recipe. Commit with the textures (Task 2), so that every item keeps an icon.

### Task 2: Textures (`data`)
- [ ] `items/Bucket.png`, `items/WaterBucket.png` from the pack, 16x16 checked, credited in
  `docs/ressources.txt`; look at them; the client test that every item has an icon passes; the
  goldens the two new items change are looked at and updated. Commit `feat: the bucket and the
  water bucket` (items, recipe, textures).

### Task 3: The bucket rules (`shared`)
- [ ] Tests (raycast.rs): the water-source ray stops at a source as a whole block and goes
  through flowing water; the ordinary one still goes through both. (players/blocks.rs): a
  bucket on a source fills, the source goes; from a stack of 3, the water bucket goes into the
  inventory, or is returned as dropped when it is full; a bucket aimed at flowing water, at a
  block, out of reach: nothing; a water bucket pours a source against the aimed face and becomes
  a bucket; into flowing water: a source; into a source, a decoration, an unloaded chunk:
  refused, kept, re-stated; where the player stands: poured; aimed at a chest without sneaking:
  nothing; once per press; left click with a bucket on a source breaks nothing. See them fail.
- [ ] `raycast_to_water_source`, `aim`, `fill_bucket`, `pour_water`, `BlockInteractions::dropped`,
  `ActionsOutcome::dropped`. See them pass. Commit `feat(shared): a bucket takes a water source
  and pours it elsewhere`.

### Task 4: The server drops what does not fit (`server`)
- [ ] Test (session.rs): with a full inventory, a water bucket poured ahead becomes a bucket and
  a source every client sees, spreading; two buckets take it back, and the water bucket that
  does not fit lands on the ground. See it fail.
- [ ] `DroppedItems::drop_from_player`, called for `outcome.dropped`. Commit `feat(server): a
  filled bucket with no room in the inventory is dropped`.

### Task 5: The client aims and splashes (`client`)
- [ ] Tests (audio/blocks.rs): a source that appears or goes splashes, flowing water and a
  block placed into a source do not. See them fail.
- [ ] `handle_block_interactions` aims with `aim`, so that the outline, the opening of a chest
  and the right click follow the bucket; `Sound::Splash`. Commit `feat(client): the bucket
  aims at water and splashes`.

### Task 6: A player fills and empties a bucket (e2e)
- [ ] `client/src/e2e/bucket.rs`: pour the water bucket ahead, see the water spread, take it
  back with the bucket, see it drain; snapshot of the poured water. Commit `test(client): a
  player pours water from a bucket and takes it back`.

### Task 7: The hearts under the open inventory (`client`)
- [ ] The survival e2e test opens the inventory and asserts the hearts are hidden, shown again
  once it closes. See it fail. Hide them. Look at the review snapshots (inventory, chest,
  furnace) before and after. Commit `fix(client): the hearts hide under the open inventory`.

### Task 8: Closing
- [ ] README, `docs/ARCHITECTURE.md`, `CHANGELOG.txt`; the review of the branch; spec decisions
  appended; `timeout 1800 just check`; commits `docs: ...`.
