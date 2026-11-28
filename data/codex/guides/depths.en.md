# The depths

The underground is cut into three tiers, each harder than the one above. The deeper you go, the stronger and more numerous the hostiles, and the better what they drop. The first time you reach a tier, its name appears on screen and you earn an [[page:achievements|achievement]].

## Two kinds of world

A **new world** is deep: its stone gives way to gloomrock under y 0 and goes down to bedrock at y -64. A world saved before the deep underground came in stays **shallow**, as it was generated: bedrock at y 0. The tiers, the lava and the ores sit at different heights in each.

| | New world | Old world |
|---|---|---|
| Bedrock | y -64, rough up to -60 | y 0 |
| Lowest caves | y -59 | y 6 |
| Lava lakes | every cave at y -54 and below | every cave at y 10 and below |
| [[Gloomrock]] | below y 0, mixed with stone around -2 to 0 | none |
| Deep cave biomes | y -1 to -56 | none |

Caves are carved on floors about 48 blocks apart. Below y 48 they open into chambers, and in a new world they widen into big caverns below y 0. Bedrock cannot be broken. Fall below y -114 and you leave the world and die.

## The three tiers

A place counts as underground only with real ground over it: water, leaves and plants don't hide the sky, so the bottom of a lake is not in a tier.

| Tier | New world | Old world | Hostile strength |
|---|---|---|---:|
| The Caves | below y 50 | below y 50 | 100 % |
| The Depths | below y 0 | below y 20 | 150 % |
| The Furnace | below y -40 | below y 12 | 200 % |

### What strength changes

A hostile keeps the strength of the tier it appeared in. Strength multiplies, rounded up:

- its health, the damage of its blows, and the experience it gives;
- how many hostiles may be around you at once: 10, then 15, then 20 within 64 blocks;
- how often they appear: one spawn try every 2 seconds in the Caves, one plus a 50 % chance of a second in the Depths, two in the Furnace;
- how many times its loot is rolled when you kill it: once in the Caves, once plus a 50 % chance of twice in the Depths, twice in the Furnace;
- how many guards a [[guide:structures|dungeon]] may hold.

On top of that, a kill deep down may drop a bonus:

| Tier | Bonus drop |
|---|---|
| The Caves | none |
| The Depths | [[IronIngot]], 1 in 8 |
| The Furnace | [[GoldIngot]], 1 in 6, and [[Diamond]], 1 in 20 |

### The hostiles of the caves, tier by tier

Health, damage of a blow at Normal difficulty, and experience:

{{table depths.hostiles | Hostile | The Caves | The Depths | The Furnace | {health} HP • {damage} dmg • {xp} XP}}

Under a roof and below y 50, the usual hostiles are [[mob:Gnawer|Gnawers]] (in packs of 2 to 4) and [[mob:Roller|Rollers]]. From the Depths down, 1 spawn in 3 is a [[mob:Cinderling]] instead, which throws embers that set you burning.

## Deep cave biomes

In a new world, the caves between y -1 and -56 belong to one of three biomes, or to none. Find the nearest with /locate lush_caves, /locate crystal_caves or /locate basalt_galleries (see [[guide:commands|commands]]).

### Lush caves

Damp caves carpeted with [[Moss]] on floors and ceilings, with patches of [[Clay]] on the walls, small pools of water in the floor and [[CaveVine|cave vines]] up to 4 blocks long hanging from the ceilings. Peaceful [[mob:Mossback|Mossbacks]] graze here; they appear with the cave and drop moss and sometimes leather. Moss makes potions last longer at the [[page:alchemy|alembic]], and cave vine brews splash poison.

### Crystal caves

Caverns walled with [[Geodestone]], studded with glowing [[Glowcrystal]], with [[CrystalCluster|crystal clusters]] growing on the floor. Both break with anything and give [[CrystalShard|crystal shards]]. [[mob:Shardling|Shardlings]] guard these caves: some are there when the cave is found, and more appear in the dark on geodestone. The glow of the crystals keeps hostiles from appearing close to them.

### Basalt galleries

Only below y -18, near the lava: galleries of [[Basalt]], with [[Magma]] on some floors and lava in some floor pits. [[mob:Slagcrawler|Slagcrawlers]] appear in the dark on basalt and magma; their bite burns. Standing on magma burns you unless you sneak. Magma is the ingredient of the potion of fire resistance.

## Hazards

- **Lava** deals 4 damage at once and 4 more every half second you stay in it, slows you to a crawl, and sets you burning for 15 s after you get out. Water puts the burning out, but not while you are still in the lava.
- **Fire** sets you burning for 4 s, 1 damage a second. Cinderling embers and Slagcrawler bites burn too.
- **Water**: you have 15 s of air, then lose 2 health a second.
- **Falls** deal 1 damage per block beyond 3, and armour does not soften them.

## How to prepare

- **Light.** Any light source within 8 blocks, along each axis, stops hostiles from appearing: place a [[Torch]] or [[Lantern]] as you go, and you leave safe ground behind you.
- **Armour.** Each point of armour takes 4 % off mob blows, arrows, fire and lava. A full iron suit takes 60 % off. See [[page:armour|armour]] and [[guide:combat|combat]].
- **Fire resistance.** The [[FireResistancePotion]] (neutral base and magma) stops all fire, lava and magma damage for 3 minutes: drink it before the Furnace.
- **Water breathing.** The [[WaterBreathingPotion]] (neutral base and [[Kelp]]) keeps your air full for 3 minutes.
- **Night vision.** The [[NightVisionPotion]] (neutral base and [[RawGlowfin]]) brightens the world for 3 minutes.
- **Water.** A [[WaterBucket]] puts out burning. A lava source that water touches turns into obsidian, flowing lava into cobblestone. Water also kills Cinderlings.
- **A pickaxe that can mine the way back.** Gold and diamonds need an iron pickaxe; see [[guide:ores|ores]].
- **A respawn point.** Use a bedroll before you go down: you come back at it if you die. See [[guide:health|health]].

Down at the level of the lava waits the Ember Mother, in her lair. A [[DepthsCompass]] points to it. See [[guide:ember-mother|the Ember Mother]].
