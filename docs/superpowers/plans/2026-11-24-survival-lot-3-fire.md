# Survival Lot 3 (Random block ticks and fire) Implementation Plan

**Goal:** The server gives random ticks to the blocks near the players, through a pure shared
rule. Fire is the first block to use it: lit with a flint and steel, it ages, spreads to what
burns, burns it away, and goes out in the rain or without support. A player in a fire burns for
4 s, 1 damage a second, until water or rain puts it out; the client shows flames at the bottom
of the screen. To see: craft a flint and steel (iron ingot + flint, from gravel), right click
the floor of a wooden house.

**Spec:** `docs/superpowers/specs/2026-11-24-survival-death-design.md`, "Lot 3".

## Global Constraints

- English, Conventional Commits, no trailer. Doc comments on items, tests named as sentences.
- TDD: each test seen failing (not compiling counts) before the code.
- `PROTOCOL_ID` stays 29 (`BlockId::Fire`, `ItemId::Flint`, `ItemId::FlintAndSteel` and
  `PlayerUpdateEvent.burning_ticks` are appended). `FORMAT_VERSION` unchanged: the age of a fire
  is its `BlockData.level`, already saved, and the burning is not saved.
- `timeout 600 cargo test -p <crate>` while working, `timeout 1200 just check` at the end.

## Decisions (where the spec is silent)

1. **The random tick API** (`shared/src/world/ticks.rs`), for lots 4 (lava) and 7 (crops,
   farmland):
   - `RANDOM_TICKS_PER_SECTION` (3): the cells of a 16³ chunk drawn per tick.
   - `TickContext { raining: bool }`: what a rule reads beyond the blocks. A rule that needs
     the sky asks `sees_sky(world, pos)`.
   - `BlockChange { pos, block: Option<BlockData> }`, `None` for air.
   - `random_tick(world: &ChunkMap, pos, context: &TickContext, rng: &mut impl Rng) ->
     Vec<BlockChange>`: what the block at `pos` does on a random tick, computed from the map as
     it is; it changes nothing itself. Lots 4 and 7 add an arm to its `match`.
   - `sees_sky(world, pos)`: no block but decorations above `pos` in the chunks loaded.
2. **When fire ticks.** One random tick per 1365 ticks per cell would leave a fire still for a
   minute: a fire's random tick comes on its own schedule, every 30 to 40 ticks, as Minecraft's
   fire. The server keeps the fires it knows (`BlockTicks.fires`): a cell turning into fire (a
   player, a spread) schedules it, and a random tick that lands on a fire not scheduled (a
   saved one) schedules it. At most `MAX_FIRE_UPDATES` (64) fires tick in a tick; the others
   wait for the next, still due.
3. **The simulation distance.** Random ticks reach the chunks within `SIMULATION_DISTANCE`
   (4 chunks, a cube) of each player, or its render distance if lower; a chunk of air only is
   skipped.
4. **The fire rules** (`shared/src/world/fire.rs`), on each tick of a fire, in this order:
   rain and a sky seen put it out; without support (a solid block below, or a flammable block
   among its six neighbours) it goes out; it ages by one and goes out past `MAX_FIRE_AGE` (15).
   Then each flammable neighbour burns with chance `burn / 300`: it turns into fire (chance
   `5 / (age + 10)`) or into air. Each air neighbour touching a flammable block catches with
   chance `catch / 100` of its most flammable neighbour. A new fire is as old as the one that
   lit it, plus 1 one time in 5. Flammability (`catch`, `burn`), Minecraft's: planks and the
   crafting table 5/20, logs 5/5, leaves 30/60, tall grass, fern and flowers 60/100, vine
   15/100, cocoa 30/60.
5. **The fire block.** Cross shape (`flora`, no sway), light 15, `Decoration`, replaceable, no
   hitbox, a ray box over the bottom 0.4 of its cell, broken by the first hit (break time 1:
   0 would divide the breaking stage by zero), no drop, no block sound. Texture
   `fire_0.png` (16x128) as `Fire.png` with `Fire.anim.ron` `(frame_time: 2, interpolate:
   false)`.
6. **Flint and steel.** `ItemId::durability()` gives the durability of a tool or of the flint
   and steel (64), which is not a mining tool (`tool()` stays `None`). A right click lights a
   fire in the air cell against the aimed face, if a fire would stay there; it wears by 1, and
   waits for the release of the button like the bucket. The recipe is shapeless: `Recipe`
   gains `shapeless`, matched as the multiset of the filled cells; it shows as a row of two.
7. **Gravel** drops flint one time in 10: its table is `[(9, Gravel), (1, Flint)]`.
8. **Burning** (`shared/src/players/fire.rs`). `Player.burning_ticks` and
   `Player.fire_damage_wait`, `#[serde(skip)]`. `Player::burn(in_fire, doused)`: doused (the
   body touching water, or rain with the sky seen from the head) clears it; in a fire it is set
   to at least `BURNING_TICKS` (80); while it burns the player takes `FIRE_DAMAGE` (1) at once
   and every `FIRE_DAMAGE_INTERVAL` (20) ticks. Creative and dead players do not burn. Respawn
   clears it. `burn_system` runs after `breathe_system`.
9. **Client.** `PlayerUpdateEvent.burning_ticks` sets `Player.burning_ticks`.
   `client/src/ui/hud/burning.rs`: two flames over the bottom third of the screen, cut from
   `Fire.png`, frame by frame (0.1 s each), shown while the local player burns. No sound: the
   game has no crackle, none is generated in this lot.

## Tasks

### Task 1: Blocks and items in shared
- [x] Tests: `Fire` is a decoration, replaceable, light 15, no hitbox, broken by a hit, drops
  nothing; gravel drops flint about 1 in 10; the shapeless recipe in both orders and columns;
  `durability` of the flint and steel; pinned encodings.
- [x] Variants, tables, recipe, durability in `wear_tool` and the client's wear bar.

### Task 2: Fire rules and random ticks in shared
- [x] Tests (seeded rng): a fire with no support goes out; one on stone ages and goes out past
  15; rain puts out a fire under the sky and not one under a roof; a fire next to planks
  spreads to the air around them and burns them; a fire never spreads into stone.
- [x] `fire.rs`, `ticks.rs`, `sees_sky`.

### Task 3: Flint and steel
- [x] Tests (`players/blocks.rs`): a right click lights the air against the face and wears the
  flint and steel; refused into water and where a fire would not stay.
- [x] `light_fire`.

### Task 4: Burning
- [x] Tests: a player in a fire burns 80 ticks for 4 damage; water and rain clear it; creative
  does not burn; respawn clears.
- [x] `burn`, `body_in_fire`, `doused`.

### Task 5: Server
- [x] Tests: random ticks only reach the chunks near a player; at most 64 fires tick per tick,
  the rest later; a scheduled fire ticks within 40 ticks; `burn_system` burns a player in fire.
- [x] `world/ticks.rs` (`BlockTicks`, `block_ticks_system`), `burn_system`, the schedule,
  `burning_ticks` in the update.

### Task 6: Client
- [x] Shape, textures (`Fire.png`, `Flint.png`, `FlintAndSteel.png`), tables, the flames.
- [x] e2e (`survival.rs`): a player lights a fire at its feet, burns, loses health, sees the
  flames.
- [x] Goldens of the item browser: only the diffs of the new items regenerated.

## Notes

- The flint and steel's item browser line is "Tools" without "· Tool": that suffix reads
  `tool()`, which stays for the mining tools.
- No crackle: `data/sounds/` has none and this lot generates no sound.
- The goldens of the item browser and of the inventory screens (which dock it) changed by the
  two new items only: the count, flint among the materials, the flint and steel after the
  buckets, the scrollbar.
