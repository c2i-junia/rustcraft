# Ores

Ores are scattered through the stone (and the [[Gloomrock]] under it) in small clusters and straight veins. Each ore has its own band of heights: coal, copper and tin are common and high, iron sits in the middle, gold and diamonds are deep and rare.

The heights depend on the world. A **new world** is deep: its bedrock is at y -64. A world saved before the deep underground came in is **shallow**: its bedrock is at y 0, and its ores are squeezed into the stone above it. See [[guide:depths|the depths]] for the layers of each kind.

## Every ore

"Tries" is how many clusters or veins of the ore start in each 16 x 16 x 16 chunk of stone in its band. A vein can stray up to 7 blocks from where it starts, so ore turns up a little beyond the band.

{{table ores.spread | Ore | New world: y | Tries | Old world: y | Tries | Cluster | Vein | Veins | {from} to {to} | -}}

In a new world, gold and diamonds are not spread evenly over their band. Gold keeps 4 tries in 5 at y -32 and below, but only 2 in 5 above. Diamonds keep 4 tries in 5 at y -42 and below, and only 6 in 100 above: almost all of them lie between y -58 and -42.

Copper and tin never take the place of another ore. The other ores can: a rarer ore wins a block that two of them reach.

## Mining and smelting

Every ore needs a pickaxe of at least the tier below. With a weaker one, or with no pickaxe, the block breaks and drops nothing.

{{table ores.mining | Ore | Pickaxe needed | Drops | Smelts into | Any | Wood or gold | Stone or bronze | Iron | Diamond | -}}

- Coal ore and diamond ore give their resource straight away, and experience with it: 1 point for coal, 5 for a diamond.
- Copper, tin, iron and gold ore drop themselves. Smelt them in a [[Furnace]] (10 s each); their experience comes out of the furnace instead.
- Tin needs a stone pickaxe, so a [[StonePickaxe]] comes before bronze: three copper ingots and one tin ingot make 4 [[BronzeIngot]].
- Gold and diamonds both need an [[IronPickaxe]].

## Slate and limestone

Two rocks grow in large pockets of 20 to 27 blocks rather than in clusters, in both kinds of world. They take any pickaxe and drop themselves, for building.

| Rock | y | Pockets per chunk |
|---|---|---:|
| [[Slate]] | 5 to 30 | 12 |
| [[Limestone]] | 40 to 100 | 8 |

Below y 0 in a new world the rock is [[Gloomrock]]. Mined, it drops [[CobbledGloomrock]], which smelts back into gloomrock.

## Finding diamonds

- In a new world, dig to **y -42 and below**, down to -58. That is inside the [[guide:depths|Furnace tier]] (below y -40), where hostiles are twice as strong, and just above the lava that fills every cave at y -54 and below.
- In an old world, diamonds lie between y 5 and 16.
- A diamond cluster is 2 to 6 blocks and there is at most one try per chunk: mine long tunnels at one height rather than digging down.
- Diamonds also turn up in chests. A sanctuary chest holds 1 to 3 diamonds 55 % of the time, a buried treasure 60 % of the time (see [[guide:structures|structures]]). A hostile killed in the Furnace tier drops one 1 time in 20.

## Finding gold

- In a new world, gold lies from y -60 to -8, and is twice as common at y -32 and below. In an old world, look between y 5 and 32.
- Hostiles killed in the Furnace tier drop a [[GoldIngot]] 1 time in 6, and the chests of sanctuaries and buried treasures are rich in gold ingots.

## Gravel and flint

[[Gravel]] breaks with anything, faster with a shovel, and drops a [[Flint]] instead of itself 1 time in 10. Gravel covers the beds of deeper seas and lakes. Flint makes [[Arrow|arrows]], the [[Spear]] and [[FlintAndSteel]].

Flint also drops from [[mob:Roller|Rollers]] (1 in 5) and [[mob:Cinderling|Cinderlings]] (1 in 10).
