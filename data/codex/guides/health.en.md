# Health and hunger

In survival you have **10 hearts** (20 points of health) and **10 drumsticks** of food (20 points). Food is what brings health back, and everything you do slowly uses it up. In creative you take no damage and never get hungry.

## Health

Health comes back by itself, **1 point every 4 seconds**, but only while your food is at **18 or more** (9 full drumsticks). Each point healed this way costs 6 exhaustion, so healing is what empties a full food bar fastest. Below 18 food, nothing heals you but potions and food effects.

- A [[HealingPotion]] gives 4 points at once per level; a [[RegenerationPotion]] heals over time.
- [[FishChowder]] gives [[effect:Regeneration]] II for 10 s, [[GrilledGlowfin]] [[effect:Regeneration]] I for 8 s. See [[page:effects|status effects]].

### Invulnerability

After a hit you ignore the next ones for **0.5 seconds**. The void, starvation and poison do not wait: they hurt you even then.

## Hunger

Hunger has two parts. **Food** is the row of drumsticks you see. **Saturation** is a hidden reserve on top of it, spent first. You start, and respawn, with full food and 5 saturation.

Your efforts add up as **exhaustion**. Every 4 exhaustion takes 1 point of saturation, or 1 point of food once the saturation is gone.

| What you do | Exhaustion |
|---|---:|
| Sprinting on the ground | 0.1 per block |
| Moving in water or lava | 0.01 per block |
| Jumping | 0.05 |
| Jumping while sprinting | 0.2 |
| Breaking a block | 0.005 |
| A blow that lands on a creature | 0.1 |
| Taking damage | 0.1 per hit |
| Healing 1 point of health | 6 |

Walking costs nothing, and neither does flying. Sprinting 40 blocks costs one point of saturation or food.

### Sprinting and starving

- You need **7 food or more** to sprint: at 6 or less (3 drumsticks) you cannot.
- With **no food at all** you starve: 1 damage every 4 seconds. Starvation **can kill you**.

## Eating

Hold the food and press the use button (right click). Eating is instant: one item per use.

- You cannot eat with a full food bar, unless the food also gives an effect.
- Food goes up to 20. Saturation goes up, but never above your food: eat when your bar is low to keep all of it.
- Soups and stews such as [[MushroomSoup]] give back their [[Bowl]].
- A [[MilkBucket]] clears every status effect and gives back the [[Bucket]]. It cannot be drunk when you have no effect.

What each food restores is on the [[page:food|food page]].

## Falls

A fall of 3 blocks or less does no harm. Beyond that you take **1 point per block**, rounded to the nearest block, so a fall of 23 blocks kills you from full health. Armour does not soften a landing.

{{table health.falls | Fall (blocks) | Damage | Hearts | All 10}}

- Landing in water, climbing a [[Ladder]] or flying resets the fall.
- Riding a [[mob:Horse]] takes 3 blocks off your fall.
- A fatal fall within 3 seconds of a hit by a creature is blamed on that creature: "threw you off a ledge".

## Air and drowning

You hold your breath for **15 seconds**, shown as 10 bubbles of 1.5 s each. Air runs out only while your *eyes* are in water or lava, and comes back in 2.5 s once you breathe again. With no air left you take **2 damage every second**.

A [[WaterBreathingPotion]] stops the air from running out. Sneaking lowers your eyes, so it can put your head under a surface you would otherwise breathe above.

## Fire, lava and magma

| Source | Damage | Burning afterwards |
|---|---|---|
| Touching fire | 1 at once, then 1 every second while you burn | At least 4 s |
| Lava | 4 at once, then 4 every 0.5 s while you are in it | At least 15 s |
| [[Magma]] | As touching fire, while you stand on it without sneaking | At least 4 s |

- Lava also slows you to 30% of your speed.
- Touching water puts the fire out, and so does rain on your head under the open sky. Neither works while you are still in lava.
- Armour does reduce fire and lava damage.
- A [[FireResistancePotion]] makes you immune to fire, lava and magma.

## The void

Falling below **Y -114**, 50 blocks under the bottom of the world, kills you at once in survival, whatever your health and armour. In creative you are simply brought back to the world spawn.

## Death

The death screen says what killed you:

| Cause | Message |
|---|---|
| A fall | You hit the ground too hard |
| A fall within 3 s of a hit by a creature | A … threw you off a ledge |
| A creature | A … got you |
| An arrow | … shot you, or An arrow got you |
| Fire or burning | You went up in flames |
| Lava | You tried to swim in lava |
| No air | You ran out of air |
| No food | You starved to death |
| The void | You fell out of the world |

Poison alone never kills: it stops at 1 point of health.

### What you drop

- Everything you carried: every inventory slot, your armour, the crafting grid and the stack on your cursor. It lies where you died for **5 minutes**, then vanishes.
- All your experience is lost. An orb worth **half of it**, at most 400 points, is left where you died.
- The place of your death is marked as a waypoint on your map.
- In creative, nothing drops.

## Respawn and the bedroll

You come back with full health, full food, 5 saturation and full air, no longer burning and with no status effects.

You respawn at your [[Bedroll]] if you have used one, or else at the world spawn. Using a bedroll, by day or by night, makes it your respawn point ("You will come back here"). If the bedroll has been broken since, you go back to the world spawn.

At night, using a bedroll also puts you to sleep. Once every player online has slept that night, the time jumps to sunrise. A night slept away does not count for the "Still Here at Dawn" achievement.

## Tips

- Keep your food at 18 or above before a fight, or you will not heal between blows.
- Roasted meat fills far more than raw meat: cook it in a [[page:smelting|furnace]].
- Pour water down a long drop, or climb down a ladder, rather than jumping. See also [[guide:combat|combat]] for armour and [[guide:depths|the depths]] for the lava lakes.
