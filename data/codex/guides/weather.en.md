# Day, night and weather

A day in Rustcraft lasts 20 minutes: 10 of night, then 10 of day. Every now and then the sky clouds over for rain or a thunderstorm.

## The day

The cycle starts at sunset:

| Into the cycle | What happens |
|---|---|
| 0 min | Sunset: night begins |
| 5 min | Midnight |
| 10 min | Sunrise: day begins |
| 15 min | Noon |
| 20 min | Sunset again |

A new world starts 75 seconds after sunrise, so your first day leaves you a little under 9 minutes before dark. See [[guide:first-day|your first day]].

## When hostiles come out

A hostile needs a dark spot to appear:

- under the open sky, only at night;
- under a roof or underground, at any time of day;
- and never with a light source (a [[Torch]], a lantern, a campfire, a lit furnace, fire, lava…) within 8 blocks along each axis.

At night, [[mob:Stumpling|Stumplings]] appear on grass, and in woods half of them are [[mob:Bramble|Brambles]] instead. Above y 110, half of the night spawns are [[mob:GustOwl|Gust Owls]]. [[mob:Drowner|Drowners]] need no darkness: they rise from water at least 3 blocks deep, twice as often at night. At dawn, Stumplings sink back into the ground. The other hostiles stay until they are killed or you go far away (96 blocks).

Staying alive from the first minute of a night until dawn earns *Still Here at Dawn*, but not if the night was slept away.

## Sleeping

Use a [[Bedroll]] to make it your respawn point, at any time of day. Used at night, you also fall asleep. When every player online is asleep, the night ends at once and the sun rises.

{{recipe Bedroll}}

Any colour of wool makes a bedroll of that colour.

## Weather

A new world starts clear. Then the weather changes on its own:

| Weather | Lasts | Comes next |
|---|---|---|
| Clear | 10 to 100 min | Rain (2 in 3) or a thunderstorm (1 in 3) |
| Rain | 5 to 15 min | Clear |
| Thunderstorm | 5 to 15 min | Clear |

### Lightning

During a thunderstorm, lightning strikes every 3 to 15 seconds, 16 to 160 blocks from a player. It is seen and heard up to 1500 blocks away. Lightning is only a sight and a sound: it hurts no one and sets nothing on fire.

### Rain and snow

{{table weather.precipitation | Snow falls instead of rain in {biomes}. | Nothing falls in {biomes}. | Everywhere else it rains, and the rain turns to snow above about y 122, on the high peaks. | and}}

Rain and snow are only how the weather looks. Its effects are the same in every biome, deserts included, wherever a block has nothing but sky above it.

### What rain does

Under the open sky, while it rains or storms:

- it puts out a burning player, unless you are still in lava;
- it puts out fire blocks;
- it waters crops, which grow twice as fast, as they do with water within 4 blocks. Both together are no faster. See [[guide:farming|farming]];
- it kills [[mob:Cinderling|Cinderlings]], which then drop nothing;
- it fills a [[RainCistern]]. Use a [[Bucket]] on a full one to get a [[WaterBucket]]: water even where there is none around;
- it puts out a [[Campfire]]. Light it again with a torch, a [[Flint]] or [[FlintAndSteel]].

## Commands

On a server these need an operator; in a single-player world you can always use them. See [[guide:commands|commands]].

| Command | What it does |
|---|---|
| /time set day | Moves the clock forward to the next noon |
| /time set night | Moves the clock forward to the next midnight |
| /weather clear | Clears the sky |
| /weather rain | Starts rain |
| /weather thunder | Starts a thunderstorm |

The clock only goes forward. A weather set by command lasts as long as a natural one of its kind.
