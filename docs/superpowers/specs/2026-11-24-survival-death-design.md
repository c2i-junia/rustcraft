# Survival, death, hunger and hostile mobs — design

Date: 2026-11-24. Branch `feat/survival-death`, worktree `feat-survival-death/`, from main
`80544cca` (protocol 29; first cut from `c5eec11b`, rebased on 2026-11-25).

The user asked for death in survival (falls, drowning, fire), hostile mobs modelled in Blender,
and a visual proposal first: https://claude.ai/artifact/4KGRvVzjr4xWM7AV6Vyh4X . They then said
"go" on every recommendation of the proposal and widened the scope: simpler hostile mobs as
well, lava, hunger, meat, and some farming. Every other decision is delegated; this file records
them. Later rulings are appended under **Decisions**.

## Rulings from the proposal

- **Death drops the inventory** where the player died, as dropped items (the item-dropping code
  exists). Creative never drops.
- **All mobs**: the four of the proposal plus three simple ones.
- **Lava is in** (the user overrode the proposal's "no").
- **`/difficulty peaceful|normal`**, per world, saved in `world.ron`; peaceful removes and stops
  hostile mobs only.
- **Hunger is in** (the user overrode the proposal's "out"), with meat and crops to eat.

## Global rules

- Server authoritative, 20 ticks/s. Pure rules (damage, air, hunger, flow, growth, fire spread)
  live in `shared` as functions unit-tested there; server systems apply them.
- Enums that travel or are saved (`BlockId`, `ItemId`, `MobKind`, `DeathCause`, messages,
  `MobAction`, `NetworkAction`) are **append only**, and their pinned-index tests get the new
  variants. `PROTOCOL_ID` goes 29 → 30 once, in lot 1, and stays 30 for the whole branch.
  `FORMAT_VERSION` only moves if a saved encoding changes; new `Player`/`WorldMeta` fields use
  `#[serde(default)]` so old saves load.
- Textures come from Pixel Perfection (see `CLAUDE.md`), already downloaded to
  `/tmp/claude-1000/-home-user-Code-rustcraft/9b840b79-4241-4412-a63d-04e24ad06d76/scratchpad/pp/`
  (`block/`, `item/`, `gui/sprites/hud/`); fetch any other file from the raw GitHub URL. Credit
  each file in `docs/ressources.txt` and in the commit message.
- Sounds: `data/sounds/player/hurt.ogg` already exists and nothing plays it. New sounds follow
  `scripts/generate_sounds.py`.
- Mob models: `scripts/generate_hostiles.py` (Blender, headless) builds
  `data/models/animated/{Stumpling,Drowner,Cinderling,GustOwl,Gnawer,Bramble,Roller}.glb`,
  already generated, each with clips Attack / Idle / Walk = glTF animations 0 / 1 / 2, 24 fps,
  metres, feet on the origin, facing +Z. Run it with
  `blender --background --factory-startup --python scripts/generate_hostiles.py -- [Name ...]`.
- Each lot: its plan in `docs/superpowers/plans/2026-11-24-survival-lot-N-<topic>.md`, test
  first, `just check` green, one or more Conventional Commits in English, **no attribution
  trailer**. Game text stays in English.

## Lots

### Lot 1 — one door for damage, and feedback

- `DamageSource` in shared: `Fall`, `Void`, `Drowning`, `Fire`, `Lava`, `Starvation`,
  `Mob(MobKind)`. `Player::hurt(amount, source)` replaces `hurt(damage, cause)`: game mode,
  **invulnerability** of 10 ticks after a hit (a second hit inside it is ignored; Void and
  Starvation pierce it), sets `death_cause` from the source.
- `DeathCause` gains, appended: `Drowning` "You ran out of air", `Fire` "You went up in
  flames", `Lava` "You tried to swim in lava", `Starvation` "You starved to death",
  `Mob(MobKind)` "<A Stumpling> got you", `ThrownOff(MobKind)` "<A Gust Owl> threw you off a
  ledge". A fall within 3 s (60 ticks) of a hit by a mob is `ThrownOff` that mob. Mob display
  names: Stumpling, Drowner, Cinderling, Gust Owl, Gnawer, Bramble, Roller, and the animals'.
- `Player::knock_back(direction, strength)`: horizontal push and a small hop, applied through
  the movement state the client reconciles with.
- Client feedback when the local player's health drops: red edge vignette fading over 0.5 s,
  a short camera tilt, `player/hurt.ogg`. Hearts blink briefly.
- PROTOCOL_ID 30.

### Lot 2 — drowning

- `Player.air: u16`, max 300 ticks (10 bubbles of 30). It drops by 1 a tick while the **eyes**
  are in a liquid (the block at eye height), refills 6 a tick otherwise. At 0: 2 damage every
  20 ticks, `DamageSource::Drowning`. Creative never loses air. Respawn refills.
- HUD: 10 bubbles (`air.png`, `air_bursting.png` for the one bursting) on the right above the
  hotbar, mirroring the hearts, shown only while air < max. Lot 6 puts the food row there;
  bubbles then sit above food.

### Lot 3 — random block ticks and fire

- A server `random_block_ticks` system: each tick, for each loaded chunk near a player (within
  the simulation distance), 3 random blocks per 16³ section get `random_tick`. Shared pure rules
  decide what each block does; this lot uses it for fire, lot 7 for crops.
- `BlockId::Fire`: no hitbox, a small ray hitbox so a left click puts it out at once, cross
  shape like tall grass, light 15, `Decoration` (water washes it away), animated `fire_0.png`
  (8 frames, `frame_time: 2`). Age kept in `BlockData.level`.
- Flammability table (shared): logs, planks, leaves, tall grass, fern, vine, flowers, cocoa,
  crafting table catch and burn; stone, dirt, sand, chest etc. do not.
- Fire rules on its random tick: it ages; past age 15 or with no support (solid block below or
  a flammable neighbour) it goes out. Each tick it may set a flammable neighbour on fire (chance
  by flammability) and may burn the block it sits on / next to it into air. Rain puts out fire
  that sees the sky. At most 64 fire updates per tick in the world.
- `ItemId::Flint` (gravel drops it 1 time in 10, else gravel), `ItemId::FlintAndSteel`
  (shapeless iron ingot + flint, durability 64): right click on a face sets fire on the air
  block in front.
- Player in a fire block: `burning_ticks = 80` (4 s). While burning, 1 damage every 20 ticks
  (`DamageSource::Fire`). Water (feet in water) or rain clears burning. Sent to the client:
  a flame overlay at the bottom of the screen while burning.

### Lot 4 — lava

- `BlockId::Lava`: liquid like water, light 15, animated `lava_still.png` (16 frames,
  `frame_time: 4`), opaque-ish orange fog when the eyes are in it, movement slower than water.
- It flows with the water rules made generic over the liquid: lava spreads 3 blocks on flat
  ground and steps every 30 ticks instead of 5.
- Water meets lava: a lava source becomes `BlockId::Obsidian` (new, diamond pickaxe, long break
  time, `obsidian.png`), flowing lava becomes cobblestone.
- In lava: 4 damage every 10 ticks (`DamageSource::Lava`) and burning set to 300 ticks. Lava sets
  flammable neighbours on fire on its random ticks.
- `ItemId::LavaBucket` (`lava_bucket.png`): the bucket takes lava sources and pours them like
  water. It is also furnace fuel (20 000 ticks, gives the bucket back).
- World generation: cave air at y ≤ 10 is filled with lava sources (lava lakes at the bottom of
  caves). Spawn search must not put players in lava.

### Lot 5 — combat, on top of main's blows

Main already has blows (`server/src/mob/combat.rs`, `ItemId::attack_damage`, `MobKind::health`,
sheep drop mutton and wool). This lot completes it rather than redoing it:

- Mobs get 10 ticks of invulnerability after a blow, a horizontal knock back away from the
  player (6 blocks/s, main's hop kept), and animals flee for 5 s once hit. Axes deal ×1.5 to
  Stumpling (lot 9, a `MobKind` hook now).
- A blow costs the weapon durability: 1 for swords and the spear, 2 for other tools.
- The client tints a hit mob red for 0.3 s and plays a hurt sound (`MobUpdate` carries what it
  needs; append fields with care for the pinned encodings).
- Loot for every animal, as dropped items, following main's naming (`RawMutton` /
  `RoastMutton`): pig 1–3 `RawPork`, cow 1–3 `RawBeef` + 0–2 `Leather`, chicken 1 `RawChicken` +
  0–2 `Feather`, rabbit 1 `RawRabbit`; others nothing. Their roast versions (`RoastPork`,
  `RoastBeef`, `RoastChicken`, `RoastRabbit`) are smelted in the furnace and eaten through
  main's food door (lot 6 turns it into hunger). Textures from Pixel Perfection (raw and
  cooked meats, leather, feather), in main's food category if it has one.
- The hostile lots plug into the same blow path (health, loot, hurt flash).

### Lot 6 — hunger

Main's food restores health directly (`ItemId::food_health`, a right click, refused at full
health). Hunger changes that door:

- `Player.food: u8` (0–20), `saturation: f32` (≤ food), `exhaustion: f32`, saved with serde
  defaults. Exhaustion grows: sprinting 0.1 per block, jumping 0.05 (0.2 while sprinting),
  swimming 0.01 per block, breaking a block 0.005, a blow 0.1, taking damage 0.1, regenerating a
  point of health 6. At 4 it wraps: saturation −1, or food −1 when saturation is 0.
- Health regeneration (1 point per 80 ticks) only when food ≥ 18. At food 0, 1 damage every 80
  ticks (`DamageSource::Starvation`, which can kill). Sprinting is refused at food ≤ 6.
  Creative: no hunger. Respawn: food 20, saturation 5.
- Eating keeps main's gesture but gives food and saturation instead of health, and is refused
  at full food instead of full health. `food_health` becomes `food_value() -> Option<(u8, f32)>`
  for every food item (main's and lot 5's): raw meats ≈ 2–3 food, roasts 6–8, bread 5, berries
  2, roast mushroom 4, carrot 3, potato 1, baked potato 5 — balance against main's values and
  write the table in the plan.
- HUD: 10 drumsticks (`food_full/half/empty.png` from Pixel Perfection, 9x9) right above the
  hotbar, filled right to left; the air bubbles move above them.

### Lot 7 — more farming, one random tick

Main has farming (`TilledSoil`, rye, berry bushes, saplings, `server/src/world/growth.rs`).
This lot extends it:

- **One random tick system**: fold lot 3's `world/ticks.rs` and main's `growth.rs` into one,
  keeping both behaviours (plants, cisterns, campfires in rain, fire, lava igniting) and the
  fire update bound.
- Carrots and potatoes as two more crops on tilled soil, grown like rye: items `Carrot`,
  `Potato` (each 2 % from tall grass, planted by a right click on tilled soil), `BakedPotato`
  (furnace). Crop blocks with Pixel Perfection stage textures (`carrots_stage0..3`,
  `potatoes_stage0..3`). Ripe crops drop 2–4, unripe 1.
- Tilled soil near water (4 blocks) grows crops twice as fast, if main's rules do not already
  do something like it.

### Lot 8 — hostile spawning, difficulty, and the three simple mobs

- Shared `is_night(tick)` from `DAY_DURATION_IN_TICKS` (the cycle starts at sunset; night is
  the first half). `WorldMeta.difficulty: Difficulty { Peaceful, Normal }` (default Normal),
  `/difficulty <peaceful|normal>` with completion; peaceful removes hostile mobs at once.
- Spawning: every 40 ticks per player, one attempt at a random loaded column 24–64 blocks away
  horizontally. Dark = (night and sky visible) or (a solid block above) and no light emitter
  (torch, lit furnace, fire, lava) within 8 blocks. The place picks the kind (table below).
  Caps: 10 hostiles per player within 64 blocks, 40 per world. Hostiles 96 blocks away from
  every player despawn. Spawning ignores the game mode; attacks ignore creative and dead
  players.
- Generic hostile behaviour: `MobAction` gains, appended, `Chase` and `Attack`. A hostile sees a
  survival player within 16 blocks with a clear line of sight, walks at it, attacks when in
  reach, with a per-kind cooldown. It forgets the player 24 blocks away. Hostiles have health,
  take hits like animals (lot 5), and drop loot.
- Client: a `HOSTILE_CLIPS` table (Attack 0, Idle 1, Walk 2); `Chase` plays Walk, `Attack` plays
  Attack once.
- **Gnawer** (cave rat): caves (solid block above, y < 50), packs of 2–4. 6 health, 2 damage,
  2.8 blocks/s, cooldown 15 ticks, small hitbox (0.6 × 0.5). Drops nothing.
- **Bramble** (bush ambusher): forests and jungles at night, on grass. Never moves; when a
  player comes within 2.5 blocks its lid opens (Attack) for 3 damage every 20 ticks. 12 health.
  Stays at day. Drops 1–3 sticks.
- **Roller** (boulder): caves and mountains (y > 100). When it sees a player within 12 blocks it
  winds up for 20 ticks (Attack clip), then rolls straight at 7 blocks/s until it hits a block or
  a player: 4 damage and a strong knock back (8). After hitting a block it is stunned 40 ticks.
  16 health. Drops 1–2 cobblestone and flint 20 %.

### Lot 9 — Stumpling

Surface (grass, plains and forests) at night in the dark. 24 health, 3 damage, 1.2 blocks/s,
cooldown 20 ticks. Axes deal ×1.5. Flees fire within 6 blocks. At dawn it sinks into the ground
(despawns, 2 s sinking) leaving nothing. Drops 2–4 sticks, charcoal 30 %.

### Lot 10 — Drowner

Water at least 3 deep, any time (more at night: double the chance). Swims in 3D in water at
3 blocks/s, 0.8 on land, goes back to water. Attack: grabs (2 damage) and drags the player
down 1 block/s for 40 ticks, during which the player cannot swim up. 18 health. Drops nothing.

### Lot 11 — Cinderling

Caves below y = 20 in the dark. Hovers 2 blocks above the ground, keeps 6 blocks from its
target, throws an ember every 60 ticks: a projectile in an arc (add a tiny glowing `Ember` model
to the generator and a projectile entity; the simplest sound way in this codebase is a mob kind
with no health that the server moves and removes) that deals 1 damage, sets burning 60 ticks,
and sets the flammable block it lands on on fire. Dies in water or rain. 10 health. Drops 1–2
coal, flint 10 %.

### Lot 12 — Gust Owl

Above y = 110 at night. Flies, circles 6–10 blocks above its target, then dives: 2 damage and a
knock back of 8 blocks/s; deaths by falling within 3 s are `ThrownOff(GustOwl)`. Sneaking
halves knock back from any source. 14 health. Drops 1–3 feathers.

### Lot 13 — death drops, balancing, docs

- On death in survival, the whole inventory (and the cursor stack) drops where the player died,
  scattered; it despawns after the usual dropped item lifetime (raise it to 5 min if lower).
- An e2e test dies from each cause it can stage cheaply.
- README features, `docs/ARCHITECTURE.md` (tick systems, random ticks, combat, spawning),
  `CHANGELOG.txt`.

## Decisions

- **Rebase on main `80544cca`** (2026-11-25). Appended after main's variants: `BlockId::Fire`
  96, `Lava` 97, `Obsidian` 98; `ItemId::FlintAndSteel` 146, `Obsidian` 147, `LavaBucket` 148;
  the flint and steel recipe is the last one (85). `ItemId::Flint` (110) and its drop from
  gravel were main's already: lot 3 reuses them, and main's texture. A flint and steel lights
  an unlit campfire as a flint does.
- Main moves a player by its horizontal velocity (the portals' throws) in
  `apply_wished_direction`, keeps it through the air and drops it on landing, in a liquid or in
  flight. The knock back uses that: lot 1's own braking (`apply_horizontal_velocity`) is gone,
  so a knock back in water does not push.
- What main already has that later lots must build on, not redo: mob blows
  (`server/src/mob/combat.rs`: `aimed_mob` casts against the mobs' boxes with `aabb_ray_hit`
  and stops at the block `aim` finds, reach 3 or 5 with the `Spear`; `ItemId::attack_damage`;
  `ServerMob::damage` against `MobKind::health`; sheep drop mutton and wool); food
  (`ItemId::food_health`, eaten by a right click to give health back, refused at full health:
  hunger should change that door); farming (`TilledSoil`, rye, berry bushes, saplings) grown by
  `server/src/world/growth.rs`, a second random-tick system beside lot 3's `world/ticks.rs`.
  The player's aim is `players::blocks::aim` (`raycast` within `AIM_DISTANCE` 20, the bucket's
  `raycast_to_source`, the portal gun's `raycast_far`); `shared/src/world/reach.rs` is about
  which chunks a client is sent, not about the player's reach.
- **Lot 5** (plan `2026-11-24-survival-lot-5-combat.md`). `ItemId` 149–158: `RawPork`,
  `RoastPork`, `RawBeef`, `RoastBeef`, `RawChicken`, `RoastChicken`, `RawRabbit`,
  `RoastRabbit`, `Leather`, `Feather`, filed under Materials as main's mutton. Every meat is
  edible through `food_health` with Minecraft's food points (raw 2–3, roasts 5–8), main's raw
  mutton included; the campfire roasts every raw meat as the furnace does. The spear has no
  durability in main, so a blow wears only swords (1) and other tools (2). Hooks for the
  hostile lots: `ServerMob.hurt_ticks` (invulnerability, not saved nor sent), the blow path
  in `server/src/mob/combat.rs::interact` (damage by `MobKind::blow_damage(held)`, with
  `MobKind::weak_to_axes` for the Stumpling; knock back; `Flee` for `FLEE_TICKS`, which a
  hostile kind will replace by its chase), loot by `combat::loot(&ServerMob, rng)` (add an arm
  per kind), and the client's red flash and `MobHurt` sound, started by any rise of
  `ServerMob::damage` in a `MobUpdate`. A killed mob is removed at once: no flash, no sound.
- **Lot 6** (plan `2026-11-24-survival-lot-6-hunger.md`, which has the food table).
  `ItemId::food_health` is now `food_value() -> Option<(u8, f32)>` with Minecraft's food and
  saturation; main's own foods move to the spec's values (berries 2, roast mushroom 4, bread 5).
  `Player.food`, `saturation`, `exhaustion` are saved (serde defaults 20, 5, 0, copied by
  `load_player_data`); only `food` travels, in `PlayerUpdateEvent`, which the client takes
  before replaying its inputs so that its prediction refuses the sprint at ≤ 6 as the server
  does. Exhaustion is the server's: `MovementOutcome.exhaustion` (sprint per horizontal block on
  foot, any move in a liquid, jumps; flight free) plus 0.005 per broken block in
  `ActionsOutcome.exhaustion`, applied by the server's input loop; blows in `combat::interact`,
  hits in `Player::hurt`, regeneration in `Player::metabolise`. `regenerate_health_system` became
  `metabolism_system` (same 80-tick clock for regeneration at ≥ 18 food and starvation at 0);
  creative keeps main's free regeneration and never tires. `Refusal::NotHurt` is
  `Refusal::NotHungry`. Hooks: `player.exhaust(amount)` server-side for any new effort (constants
  in `players::hunger`), an arm in `ItemId::food_value` for a new food (lot 7: carrot 3/3.6,
  potato 1/0.6, baked potato 5/6.0). HUD: `ui/hud/food.rs` (`FoodBar`, `FOOD_BAND`); `AirBar`
  sits `FOOD_BAND` higher.
- **Lot 7** (plan `2026-11-24-survival-lot-7-farming.md`). Main's `growth_system` is gone:
  `server/src/world/ticks.rs` (`block_ticks_system`, `BlockTicks::tick`) is the one system of
  the blocks that change by themselves, and main's rules live in shared
  `shared/src/world/growth.rs` (`growth_tick`, `next_stage`, `is_watered`, `CROP_ODDS` 4,
  `BERRY_ODDS` 5, `SAPLING_ODDS` 6), reached from `ticks::random_tick`; only the sapling's tree
  stays the server's (`sapling_tree`). `RANDOM_TICKS_PER_SECTION` is 4, main's
  `CELLS_PER_CHUNK`, so that main's rates are kept exactly, and lava's `IGNITE_ODDS` went 3 → 4
  to keep its own. "Near the players" is lot 3's (`ticks::chunks_near_players`: loaded,
  non-air chunks within `min(render distance, SIMULATION_DISTANCE 4)`), wider than main's
  7×5×7; "rained on" is `ticks::sees_sky` (decorations let the rain through), not main's 64
  blocks of air. Appended `BlockId` 99–106 (`CarrotSprout`, `YoungCarrots`, `TallCarrots`,
  `RipeCarrots`, `PotatoSprout`, `YoungPotatoes`, `TallPotatoes`, `RipePotatoes`: one block per
  stage like main's rye) and `ItemId` 159–161 (`Carrot`, `Potato`, `BakedPotato`, Materials).
  A carrot or potato is planted by a right click on tilled soil with air above (`use_block`),
  eaten anywhere else. Water within 4 blocks of the soil (its height or one above) waters a
  crop like the rain does, rye included; both do not add up. Tall grass drops 24/16/8/1/1 in
  50 (grass, fiber, rye, carrot, potato). Ripe carrots and potatoes drop 2–4, other stages 1.
  The furnace and the campfire bake potatoes. Hooks for lot 8: `chunks_near_players(chunks,
  players, radius)` for per-player iteration, `TickContext` (only `raining` so far) for the
  night, and `growth::rained_on`.
- **Lot 8** (plan `2026-11-24-survival-lot-8-hostiles.md`). Appended `MobKind` 10–12 `Gnawer`,
  `Bramble`, `Roller` (only the kinds of this lot: the four big ones are appended by their
  lots), `MobAction` 4–5 `Chase`, `Attack`, `ItemId` 162–164 their spawn eggs (Mobs, drawn by
  `scripts/generate_spawn_eggs.py`). The rules are shared (`shared/src/world/hostiles.rs`:
  `is_night`, `Difficulty`, `MobKind::hunt`/`strike`, `is_prey`, `line_of_sight`,
  `surroundings`, `is_dark`, `spawn_kind`, `pack_size`, `may_spawn_at`); the server has
  `mob/hostile.rs` (the hunt) and `mob/spawning.rs` (`hostile_spawning_system`, chained after
  `mob_behavior_system`). `ServerWorldMap.difficulty` is saved in `world.ron`
  (`WorldMeta`/`WorldData.difficulty`, serde default Normal, copied by both `From`s, the save and
  `load_world`); `/difficulty` is the sixth command; peaceful removes the hostiles on the next
  tick, a hatched one included. Spawning: per player every 40 ticks, a spot 24–64 blocks away
  (from every player), a random height within 24 of its feet scanned down ≤ 24 cells for a
  floor with two free cells; dark = (night and open sky) or covered, and no light emitter in
  the 17³ cube around it; caps 10 per player within 64, 40 per world; despawn beyond 96 checked
  every 40 ticks. "Forests and jungles" for the Bramble are told by leaves within 8 blocks of a
  grass spot at night (covered or not), not by the biome. Rollers above y = 100 and 1 in 4 of
  the cave spawns (covered, y < 50), Gnawers the other 3. Reach is from the mob's centre to the
  player's box; sight and the forget distance likewise. Melee strikes knock back nothing; the
  Roller's 8. A strike shows as `Attack` for 10 ticks (the clip plays once), then `Chase` or
  `Idle`. A hostile hit by a player takes it as prey (`hostile::provoke`) instead of fleeing.
  The Gnawer's box is 0.6 × 0.6 (its model is 0.625 tall), the Bramble's 1.0 × 0.75, the
  Roller's 0.75; the Gnawer wanders at 1.4 b/s without a prey. The Roller's roll moves it
  without a velocity, so `is_flung` stays for throws; it rolls 60 ticks at most, rests 20 after
  a hit, is stunned 40 after a block. Extension point for lots 9–12: an arm per kind in
  `hostile::act` (after the generic prey kept / forgotten / found), a row in `spawn_kind`, and
  the kind's `hunt`, `strike`, `health`, `size`, `display_name`, `is_hostile`, `loot`, model
  with `HOSTILE_CLIPS`; a flier or swimmer also gates `behavior::apply_gravity`.
- **Lot 9** (plan `2026-11-24-survival-lot-9-10-stumpling-drowner.md`). Appended `MobKind` 13
  `Stumpling`, `MobAction` 6 `Burrow`, `ItemId` 165 `StumplingSpawnEgg`. Box 0.9 × 1.95 (the
  model is 1.125 wide with its arms and 2.06 tall; the box fits the two free cells of a spawn).
  Reach 1.2, sight 16, wanders at 0.8. Night grass picks the kind: open grass a Stumpling, the
  woods (lot 8's leaves) a Bramble or a Stumpling, half each. It flees the nearest `Fire` or
  `Lava` within 6 blocks (a sphere of offsets sorted by distance, ≤ 925 lookups a tick) at its
  own speed as `Flee`, keeping its prey and striking nothing. Dawn is the spawning system's
  (it has the time): each tick of the day, a Stumpling not burrowing gets `Burrow` for 40
  ticks; a burrowing one skips gravity and its hunt and goes down by its height through the
  ground over them, which the client draws by itself (the ground hides it; clip Idle); with 0
  ticks left the spawning system removes it, without loot. A Stumpling hatched by day
  burrows at once. Loot: 2–4 sticks, charcoal 3 in 10. "sp pl" in the item browser now also
  finds the Stumpling's egg.
- **Lot 10** (same plan). Appended `MobKind` 14 `Drowner`, `ItemId` 166 `DrownerSpawnEgg`. Box
  0.8 × 1.5 (model 1.0 × 1.56). Reach 1.5, sight 16, cooldown 20, 3 b/s in water, 0.8 on land.
  Spawning: an attempt whose scan down meets water before any block measures the column
  (`spawning::find_water`, climbing to its top when it starts inside, ≤ 16 cells each way); at
  ≥ 3 deep `SpawnPlace.water_depth` makes `spawn_kind` give a Drowner 1 in 4 by day, 1 in 2 at
  night, with no darkness test, in the middle of the column. Lot 8's ring and caps hold. A
  Drowner whose centre is in water floats (`behavior::floats`, the gate around
  `apply_gravity`) and its velocity is dropped each step, so blows do not fling it there, as
  for a player. It swims straight at its prey's centre (else along the ground, else straight
  up or down), a step taken only if its box is free and its centre stays in water: it never
  leaves the water to chase. It grabs in reach anywhere; on land it walks to the nearest water
  cell within 8 blocks (feet height and one below) or wanders. The drag is shared:
  `Player.dragged_ms` (not saved) is part of `MovementState`, so the acknowledgement carries it
  and the client's replay agrees exactly; `Player::drag` sets 2000 ms (40 ticks of input). In a
  liquid the vertical velocity is then −1 b/s after the swim (jump, swim up and the shore exit
  are void); on land it only runs down. A respawn clears it. No loot.
- **For lots 11–12.** A flier gates gravity in `behavior::floats` like the Drowner and keeps
  its own velocity out of `is_flung` (`hostile::own_speed`); moves by position steps checked by
  `may_move_to`, as `hostile::swim` does in 3D. Strikes go through `hostile::strike_in_reach`
  (or `strike_player`, which already carries per-kind side effects such as the drag); a
  time-of-day rule goes in `spawning::burrow_at_dawn`'s place, the system that knows the time.
  A per-cell search uses a sorted `LazyLock` offset list (`FEAR_OFFSETS`, `WATER_OFFSETS`). A
  projectile kind should skip `push_mobs` and the hostile caps.
- **Lot 11** (plan `2026-11-24-survival-lot-11-12-fliers.md`). Appended `MobKind` 15
  `Cinderling`, 16 `Ember`, `ItemId` 167 `CinderlingSpawnEgg` (no egg for the ember). The
  projectile is a mob kind, as suggested: main's portal-gun bolt is drawn by the client alone
  (its hit is instant) and a dropped item is an item. `MobKind::is_projectile` (the Ember only)
  is neither hostile nor an animal: `behavior::step_mobs` and `push_mobs` skip it, blows do not
  aim at it, the caps do not count it, peaceful removes it, and the save leaves it out
  (`save::saved_mobs`), so a load never meets one. `server/src/mob/cinder.rs`
  (`cinder_system`, chained between `mob_behavior_system` and the spawning) flies the embers:
  exact ballistics under −16 b/s², steps ≤ 0.25 block, life 100 ticks (`action_ticks`); the
  first prey box touched takes 1 (`Mob(Cinderling)`) and burns ≥ 60 ticks; the first cell with
  a hitbox or liquid ends it, lighting the air cell it came from when the block is flammable
  (the fire ticks schedule it as a lit fire). `hostile::act` now returns what a hostile threw,
  which `step_mobs` adds after its loop. The Cinderling (box 0.7 × 1.6; model 1.125 wide with
  its orbiting embers, 1.75 tall) floats always; its box hovers 2 above the first block or
  liquid within 8 cells below (its height kept over a deeper void), and holds 6 ± 0.5 blocks
  from its prey horizontally, by `may_move_to` steps at 2 b/s (straight, else vertical, else
  horizontal). Its `Strike` is the throw: 1 damage, reach 16, cooldown 60, in sight; the ember
  aims at the prey's centre, flying the horizontal distance / 8 b/s (0.25–2 s). Covered spots
  below y = 20 give a Cinderling 1 in 3, else lot 8's cave rule. Touching water, or seeing the
  sky while it rains or storms, removes it without loot (it was not killed); killed, 1–2 coal
  and flint 1 in 10. The Ember model (2×2×2 glowing voxels, three identical spinning clips) is
  in `generate_hostiles.py`.
