# Farming

Three crops grow on tilled soil: [[Rye]], [[Carrot]] and [[Potato]]. They feed you, feed your animals, and sell to the farmer villager. Berry bushes and saplings grow on plain ground.

## Getting seeds

Break tall grass. It is the only wild source of rye, carrots and potatoes; village fields and granaries hold rye too (see [[guide:structures|structures]]).

{{table farming.grass | Tall grass drops | Chance}}

## Tilling and planting

1. Use a hoe on [[Dirt]] or [[Grass]] with nothing on top of it. It turns into tilled soil, and the hoe wears by 1. Any hoe does it: wood, stone, bronze, iron, gold or diamond.
2. Use rye, a carrot or a potato on the tilled soil to plant it. Carrots and potatoes are planted rather than eaten when you aim at tilled soil.

{{recipe WoodenHoe}}

Breaking tilled soil gives back dirt.

## Growth

Crops grow only near a player: within 4 chunks, or your render distance if that is smaller. Each stage has a chance to pass on every random tick of its block, which comes about once every 51 seconds. The times below are averages worked out from those odds; a single crop can be much faster or slower.

{{table farming.crops | Crop | Stages | Average time | Watered | Ripe harvest | {minutes} min}}

{{table farming.stage | One stage takes about {minutes} minutes, half that when watered. Breaking a crop before it is ripe gives back 1 seed, so nothing is lost.}}

### Watering

A watered crop grows **twice as fast**. A crop is watered when either:

- rain falls on it (it sees the sky while it rains or snows), or
- water lies within **4 blocks** of its soil along both directions on the ground, at the soil's height or one block above.

Both together do not make it faster still. A ditch of water through the middle of a field 9 blocks wide waters all of it. See [[guide:weather|weather]] for how often it rains.

### Compost

{{recipe Compost}}

Use [[Compost]] on a plant. One compost:

- moves a crop up one stage,
- ripens a berry bush,
- or grows a sapling into its tree at once, half of the time.

[[Fiber]] comes from tall grass and from reeds.

## What to do with the harvest

- Three rye make [[RyeBread]].
- Bake potatoes into [[BakedPotato|Baked Potatoes]] in a [[page:smelting|furnace]].
- Feed rye to cows, sheep, goats and chickens; carrots to pigs, rabbits and horses; potatoes to pigs. See [[guide:animals|animals]].
- Sell them to a farmer: 12 rye, 10 carrots or 10 potatoes for a coin. See [[guide:trading|trading]].

{{recipe RyeBread}}

## Berry bushes

Wild [[BerryBush|Berry Bushes]] grow in forests that are not hot. Break one to take it, and plant it on dirt, grass or tilled soil. It ripens on its own in about 4.3 minutes on average (or at once with compost). Use a ripe bush to pick 2 [[Berries]]; it then starts over. Breaking a bush gives the bush, not its berries.

## Saplings and trees

- Breaking leaves gives their sapling 1 time in 12.
- [[OakLeaves]] also give an [[AppleSapling]] 1 time in 60.
- [[AppleLeaves]] give an [[Apple]] 1 time in 4 when broken. Now and then an apple also falls by itself from apple leaves with air under them.

Plant a sapling on dirt, grass or tilled soil. If it has room, it grows into a tree in about 5 minutes on average, or at once half of the time with compost.

| Sapling | Grows into |
|---|---|
| [[OakSapling]] | An oak, trunk 3 to 5 blocks |
| [[SpruceSapling]] | The same shape, in spruce wood and leaves |
| [[WillowSapling]] | The same shape, in willow |
| [[AcaciaSapling]] | The same shape, in acacia |
| [[AppleSapling]] | The same shape, oak logs with apple leaves |
| [[JungleSapling]] | A jungle tree |

## Reeds

[[Reed|Reeds]] grow wild on sand by the sea. They do not grow back or spread; break them for [[Fiber]]. There is no sugar cane.

## Rain cistern

{{recipe RainCistern}}

A [[RainCistern]] under the open sky fills up in the rain. Use a [[Bucket]] on a full one to get a [[WaterBucket]], and a water bucket on an empty one to fill it. It gives you water to bring to a field far from any lake.
