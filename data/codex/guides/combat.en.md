# Combat

A blow does its full damage only once your weapon has recharged. Wait between swings, strike while falling for a critical hit, and let armour and a shield take what the creatures give back. Every weapon is listed on the [[page:weapons|weapons page]], every piece of armour on the [[page:armour|armour page]].

## Melee

Left click a creature within reach to hit it. The damage of a blow is built in four steps:

1. **The weapon's damage**: 1 bare-handed or with anything that is not a weapon, plus 1 per level of Sharpness.
2. **Effects**: [[effect:Strength]] adds 3 per level, [[effect:Weakness]] takes 2 per level away, never below 1.
3. **The recharge**: a full blow does all of it, a blow swung at once only a fifth. In between it grows with the square of the charge: half recharged gives 40%.
4. **A critical hit** multiplies the result by 1.5. Every blow does at least 1.

### Weapons and recharge

Holding the button swings again as soon as the weapon has recharged. Switching to another item starts the recharge again from zero. A bare hand recharges in 0.4 s.

{{table combat.melee | Weapon | Damage | Recharge | Damage per second | Wear per blow | Durability | None | Never wears}}

Swords do 4, axes 6 and the other tools 2, plus 1 for stone or bronze, 2 for iron and 3 for diamond; wood and gold add nothing. The pickaxe stands for the shovel and the hoe, which hit the same.

### Critical hits

A blow at least 90% recharged while you are **falling** (in the air, going down, not flying nor swimming) is critical: **x1.5 damage**. Jump, then strike on the way down.

### Sweeps

A sword blow at least 90% recharged while you stand **on the ground** also sweeps: every other creature within 2 blocks of the target, and in front of you, takes a third of the damage (at least 1).

### Reach and knockback

- You reach creatures **3 blocks** from your eyes, or **5 with a spear**.
- A full blow throws the creature back at 6 blocks a second, a hurried one at half that. The mace throws 2.2 times as far.
- A creature you hit cannot be hurt again for 0.5 s, and an animal you hit runs away for 5 s.
- The [[mob:Stumpling]]'s bark parries blows from the front. An axe knocks its guard aside for 4 seconds, and axes deal it 1.5 times their damage.

### Weapon wear

Each blow that lands wears a sword or the mace by 1, and any other tool by 2: an axe or a pickaxe used as a weapon wears out twice as fast. The spear and the throwing axe never wear. Repair worn weapons at the [[Anvil]] (see [[page:enchanting|enchanting and the anvil]]).

## Ranged weapons

Hold the use button (right click) to draw, release it to shoot.

### Bow

A [[Bow]] is fully drawn after 1 second; released before 0.15 s it shoots nothing. The arrow flies at up to 42 blocks a second and falls in a curve. Each Bow Power level adds 1 damage. Each shot uses one [[Arrow]] and wears the bow by 1. While you draw, you walk at 40% of your speed.

{{table combat.arrows | Draw | Arrow damage}}

### Crossbow

A [[Crossbow]] takes 1.25 s to load and shoots only once fully loaded. Its [[Bolt]] always does **11 damage** and goes through 2 creatures before stopping in a third.

### Throwing axe

The [[ThrowingAxe]] is swung back for up to 0.6 s and thrown: 4 damage on a quick throw, 8 on a full one. It falls to the ground as itself, to be picked up again. In melee it hits for 5 at the pace of an axe.

### Splash potions

[[SplashPoisonPotion]], [[SplashSlownessPotion]] and [[SplashWeaknessPotion]] are thrown like the throwing axe (0.6 s for a full throw) and burst where they land. They do no damage themselves: everyone within 4 blocks gets the effect, in full at the centre and half of it at the edge. See [[page:alchemy|alchemy]].

Arrows, bolts and thrown axes hit players as well as creatures, and a shield stops them. In creative, shooting takes no ammunition.

## Shield

Hold the use button to raise a [[Shield]]. It takes 0.25 s to come up, and while it is up you walk at half speed and cannot sprint.

- It stops every blow and shot from the **front half** of you, measured flat on the ground.
- It wears by the damage it stops (at least 1 per hit): 336 points in all.
- A [[mob:Roller]]'s charge or the [[mob:EmberMother]]'s blow knocks it down for 5 seconds.
- It works from **any hotbar slot** while your hand holds a sword, an axe, a spear, the mace or nothing, so you can fight and block at once.
- It cannot be raised while you ride, draw a bow, or hold food.

## Armour

Each point of armour absorbs **4%** of a hit. Each level of Protection on any piece adds 1% more. Armour points count up to 20, and the total never goes past 90%. What is absorbed below a whole point is carried over to the next hit, so the reduction is exact in the long run.

{{table combat.armour | Material | Helmet | Chestplate | Leggings | Boots | Full suit | Absorbed | Leather | Bronze | Iron | Diamond}}

A full diamond suit with Protection III on all four pieces absorbs 76 + 12 = 88% of a hit.

### Durability

Each hit armour reduces wears every piece you wear by 1.

{{table combat.durability | Material | Helmet | Chestplate | Leggings | Boots | Leather | Bronze | Iron | Diamond}}

### What armour does not reduce

Armour absorbs the blows and throws of creatures, arrows, fire and lava. It does **nothing** against falls, the void, drowning, starvation or poison. See [[guide:health|health and hunger]].

## Hostile creatures and difficulty

The difficulty of the world changes how hard hostiles strike. On **Peaceful** there are none. **Easy** roughly halves a blow (half, plus 1), **Hard** adds half again. Below ground, a hostile born in the Depths is 1.5 times as strong and one born in the Furnace twice as strong, before the difficulty is applied (see [[guide:depths|the depths]]).

{{table combat.strikes | Creature | Easy | Normal | Hard}}

Damage of one blow at the surface, in points of health (2 per heart), before armour. Every creature is described on the [[page:creatures|creatures page]].

## Experience

A creature you kill leaves an orb of experience. Only hostiles, the [[mob:Mossback]] and fish give any: other animals and villagers give none. Hostiles born in the Depths give 1.5 times as much, in the Furnace twice as much.

{{table combat.experience | Creature | Experience}}

Spend experience at the [[EngravingTable]] and the [[Anvil]].

## Riding into battle

While you ride a [[mob:Horse]], a [[mob:Donkey|donkey]] or a [[mob:Mule|mule]], your steed takes half of every blow a creature aims at you, rounded up, and you take the rest: a blow of 3 hurts the steed for 2 and you for 1. A shield cannot be raised in the saddle. See [[guide:animals|taming, breeding and riding]].
