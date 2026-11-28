# Structures

The world holds villages on the surface, three kinds of dungeon from the surface down to the lava, shipwrecks on the sea floor with treasure buried near them, and the lair of the Ember Mother. Their chests are filled when the world is generated.

## How loot works

Each chest and barrel has 27 slots. Every row of its table is rolled on its own: first the chance, then a count between the two numbers. A chest can hold many rows or none. What a container holds is fixed by the world's seed and its position.

## Villages

There is at most one village in each 512 x 512 block area, with a chance of 4 in 5. A village needs dry, fairly flat land below height 76, well inside a biome that has a village style, away from giant jungle trees. It reaches at most 64 blocks from its centre.

### Styles

{{table structures.styles | Style | Biomes | Built of | Plains | Oak plank walls, oak log posts, thatch roofs; gravel streets | Savanna | Rammed earth walls, acacia posts and floors, thatch roofs; gravel streets | Desert | Rammed earth walls, hewn stone floors, brick posts and roofs; sand streets | Snowy | Spruce planks and logs, spruce roofs; gravel streets}}

Jungles, mountains, ice spikes, seas and rivers have no villages.

### Buildings

A well stands in a square at the centre. Streets run out from 2 to 4 sides of it, branching up to 5 times, with a lamp every 9 blocks. Up to 14 buildings line them:

| Building | How common | Most per village |
|---|---:|---:|
| Small house | 40 | any |
| Field (tilled soil with rye) | 15 | 4 |
| Big house | 14 | any |
| Granary | 10 | 2 |
| Pen | 9 | 2 |
| Forge | 7 | 1 |

"How common" is the weight each building is drawn with, out of 95.

### Pens

Each pen holds 2 to 4 animals, picked from those of the village's style:

- Plains: [[mob:Sheep]], [[mob:Pig]], [[mob:Chicken]], [[mob:Cow]]
- Savanna: [[mob:Cow]], [[mob:Goat]], [[mob:Chicken]]
- Desert: [[mob:Goat]], [[mob:Chicken]]
- Snowy: [[mob:Sheep]], [[mob:Rabbit]]

### Villagers

Every house, small or big, has one villager. Their trades follow the order the houses were built: [[mob:Farmer]], [[mob:Smith]], [[mob:Cartographer]], then again. They wander round their home by day and go in at night. Nothing can hurt them and hostiles ignore them. The cartographer sells directions to the nearest dungeon. See [[guide:trading|trading]].

### Village loot: forge

{{table structures.forge | Item | Chance | Count}}

### Village loot: granary

{{table structures.granary | Item | Chance | Count}}

### Village loot: houses and other buildings

{{table structures.house | Item | Chance | Count}}

## Dungeons

Each 256 x 256 block area may hold one dungeon of each kind. Their loot gets better the deeper they lie. Worlds saved before dungeons existed have none.

| Dungeon | Chance per area | Where | Containers | Guards |
|---|---:|---|---|---|
| Ruin | 50 % | On flat, dry land, below the mountains, away from villages | 3 chests, 3 barrels | 4 [[mob:Bramble]], 3 [[mob:Gnawer]] |
| Crypt | 75 % | Off a cave wall, y 4 to 50 (y 20 to 50 in an old world) | 2 chests, 1 barrel | 6 [[mob:Gnawer]] |
| Sanctuary | 60 % | Off a cave by the lava, y -55 to -47 (y 9 to 17 in an old world) | 3 chests | 4 [[mob:Roller]], 2 [[mob:Cinderling]] |

- **Ruin**: a roofless hall with broken walls. A [[Ladder]] goes down to a cellar.
- **Crypt**: a nave between two rows of tombs, with a chamber behind it.
- **Sanctuary**: a hall of [[Obsidian]] pillars between two channels of lava, an altar at the end. It also holds a [[GoldBlock]] and two [[BronzeBlock]]. It lies in the [[guide:depths|Furnace tier]], so its guards are twice as strong.

The area that holds the Ember Mother's lair has no crypt and no sanctuary.

### Guards

Each guard has its own spot in the dungeon's plan. A dungeon wakes while a player is within 36 blocks of it. Every 2 seconds it then has a 1 in 4 chance to send a guard to one of its spots, more in the deeper tiers (times the tier's strength).

- Guards need no darkness, unlike other hostiles.
- A guard never appears within 8 blocks of a player, nor where a player can see the spot.
- A dungeon holds at most 5 hostiles at once (fewer if it has fewer spots), times the tier's strength.
- Nothing spawns on Peaceful.

**To stop the guards**, put a [[Torch]], a [[Lantern]] or a lit [[Campfire]] within 3 blocks of each spot, or fill the spot with any block.

### Ruin loot

{{table structures.ruin | Item | Chance | Count}}

### Crypt loot

{{table structures.crypt | Item | Chance | Count}}

### Sanctuary loot

{{table structures.sanctuary | Item | Chance | Count}}

## Shipwrecks and buried treasure

Each 320 x 320 block area has a 70 % chance of a shipwreck, on a sea floor 9 to 40 blocks deep. Its chest, under the deck of the stern, always holds a [[TreasureMap]] besides its loot.

The map leads to a chest buried 2 blocks under the ground, 24 to 72 blocks from the wreck, on a shore or in shallow water when one is near. While you carry the map, the spot is marked with a red cross named "X" on your maps. Dig there to find the chest.

### Shipwreck chest

{{table structures.shipwreck | Item | Chance | Count}}

### Buried treasure

{{table structures.treasure | Item | Chance | Count}}

## The Ember Mother's lair

Each world with dungeons has one lair, a great cavern at the level of the lava around a lake of lava, where the boss of the game sleeps. A [[DepthsCompass]] points to it. See [[guide:ember-mother|the Ember Mother]].

## Finding structures

Type /locate followed by a name to get the nearest one (see [[guide:commands|commands]]):

| Structure | Command |
|---|---|
| Village | /locate village |
| Ruin | /locate ruin |
| Crypt | /locate crypt |
| Sanctuary | /locate sanctuary |
| Shipwreck | /locate shipwreck |

Without commands, explore: a village is added to your waypoints when you come within 48 blocks of it, a dungeon within 8 blocks, the lair within 40. The cartographer also sells directions to the nearest dungeon.
