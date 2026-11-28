# Game modes

A world is created with a game mode, a difficulty and a world type, all chosen in the **New
world** dialog of the Singleplayer screen (Ctrl + N). The game mode and the difficulty can be
changed later with a command; the world type cannot.

## Survival and Creative

There are two game modes. Survival is the default.

|  | Survival | Creative |
|---|---|---|
| Damage | Falls, creatures, fire, lava, drowning and hunger hurt you | Nothing hurts you |
| Hunger and air | Food goes down, air runs out under water | Never |
| Health | Comes back only while food is 18 or more | Comes back on its own, 1 point every 4 s |
| Flying | No | Yes, F toggles it; you fly through blocks |
| Items | Mined, crafted, found | Taken from the item browser beside the inventory |
| Enchanting and the anvil | Cost experience levels | Free |
| Bows and crossbows | Use an arrow or bolt per shot | Need no ammunition |
| Effects from creatures | Poison, slowness and others apply | Never; potions still work |
| Death | You drop everything you carry | You drop nothing |
| Achievements | Earned | Not earned; only play time is counted |

In Creative, the catalog of the item browser gives items: a click puts a full stack on the
cursor, a right click one item, and Shift + click a full stack into your inventory. A stack
dropped back on the catalog is destroyed. Switching from Creative to Survival while flying puts
you back on your feet.

### Switching mode

Type **/gamemode creative** or **/gamemode survival** in the chat (T). It changes your own mode
only. In a single-player world you can always use it; on a server it needs operator rights. See
[[guide:commands|Commands]].

Each player's mode is saved with them. The mode chosen for the world is the one new players
start in.

## Difficulty

The difficulty belongs to the world and applies to everyone in it. It changes how hard hostile
creatures strike, and whether they exist at all. Normal is the default.

| Difficulty | What it does |
|---|---|
| Peaceful | No hostile creatures. Those already in the world are removed. The Ember Mother never wakes. |
| Easy | A blow of *d* deals half of it plus 1, rounded down, and never more than *d*. |
| Normal | Blows deal their full damage. |
| Hard | Blows deal half as much again, rounded down. |

What some hostiles' blows become, in points of health (you have 20):

{{table game-modes.blows | Creature | Easy | Normal | Hard}}

Hostiles also hit harder the deeper you go: see [[guide:depths|The depths]]. Armour reduces these
blows (see [[page:armour|Armour]]). Difficulty does not change falls, fire, drowning or hunger.

Change it with **/difficulty peaceful**, **easy**, **normal** or **hard**. On Peaceful, the
hostiles go on the next tick.

## World types

| Type | What the world looks like |
|---|---|
| Default | Continents, seas and every biome at their usual size. |
| Large biomes | Biomes twice as large, and the continents with them. |
| Archipelago | Islands scattered over a wide sea. |
| Single biome | One biome everywhere. |

See [[page:biomes|the biomes]] for each of them.

The New world dialog offers Default, Large biomes and Archipelago. A single-biome world is made
by a dedicated server, when it creates its world:

**rustcraft-server --world-type single-biome --biome desert**

The biome is named by its key, such as **plains**, **jungle** or **ice_spikes**. The server's
**--world-type**, **--game-mode** and **--difficulty** options only apply to a world it creates:
an existing world keeps its own. See [[guide:multiplayer|Multiplayer]].

The seed field of the New world dialog takes a number from 0 to 4294967295; left empty, the seed
is random.

## See also

- [[guide:first-day|Your first day]] in Survival
- [[guide:health|Health and hunger]]
- [[guide:controls|Controls]]
