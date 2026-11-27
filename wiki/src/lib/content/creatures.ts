// What the game decides about each creature in code rather than in a table: where it lives, how
// it behaves, how hard it hits. Written from `server/src/mob/*`, `shared/src/world/hostiles.rs`
// and `shared/src/world/mobs.rs`; keep it with them.

export type CreatureFacts = {
	/// One line under the title.
	summary: string;
	/// Where and when it appears.
	spawns: string;
	/// How it moves and fights, a few sentences of HTML.
	behaviour: string;
	/// Damage of a blow at Normal difficulty, before armour.
	damage?: number;
	/// The effect its blow gives.
	effect?: string;
	xp?: number;
	/// Walk and flee, or hunt, speed in blocks per second.
	speed?: string;
	/// How far it notices you, and how far before it gives up, in blocks.
	sight?: string;
};

export const CREATURES: Record<string, CreatureFacts> = {
	Fox: {
		summary: 'A quick, shy animal of the woods and the snow.',
		spawns: 'In herds of 1 to 2 in forests, groves and flower forests, and in the ice plains.',
		behaviour: 'It runs from players. It eats berries, but cannot be tamed.',
		speed: '1.6 walking, 4 fleeing'
	},
	Sheep: {
		summary: 'The source of wool and mutton.',
		spawns: 'In herds of 2 to 4 in the plains, groves, mountains and forests, and in the pens of plains and snowy villages.',
		behaviour:
			'Use almost any item on an adult to shear it for 1 to 2 wool; the wool grows back once it has grazed. It eats rye.',
		speed: '1 walking, 2 fleeing'
	},
	Cow: {
		summary: 'Beef, leather and milk.',
		spawns: 'In herds of 2 to 4 in the plains, the savanna, groves and forest clearings, and in village pens.',
		behaviour:
			'Use a <a href="/items/bucket">bucket</a> on an adult for a milk bucket, which clears every status effect. It eats rye.',
		speed: '1 walking, 2 fleeing'
	},
	Pig: {
		summary: 'An easy source of pork.',
		spawns: 'In herds of 2 to 4 in the plains and forests, and in plains village pens.',
		behaviour: 'It eats carrots and potatoes.',
		speed: '1 walking, 2 fleeing'
	},
	Chicken: {
		summary: 'Feathers, chicken, and eggs.',
		spawns: 'In herds of 2 to 4 in the plains and forests, and in most village pens.',
		behaviour:
			'An adult hen lays an <a href="/items/egg">egg</a> about every five minutes. It eats rye.',
		speed: '1 walking, 2 fleeing'
	},
	Rabbit: {
		summary: 'A small, fast animal of the open land.',
		spawns: 'In groups of 1 to 3 in the plains, flower plains, forest clearings and ice plains.',
		behaviour: 'It runs from players. It eats carrots.',
		speed: '1.6 walking, 4 fleeing'
	},
	Wolf: {
		summary: 'Tame one, and it fights at your side.',
		spawns: 'In packs of 2 to 4 in forests, groves and the ice plains.',
		behaviour:
			'Feed it meat, raw or roast: each time there is a one in three chance it becomes yours. A tamed wolf follows you, bites for 4 every second at what you hit and at the hostiles that go after you, and can be told to sit. See <a href="/guides/animals">taming</a>.',
		speed: '1.6 walking, 4 running'
	},
	Horse: {
		summary: 'The fastest way across the land.',
		spawns: 'In herds of 2 to 4 in the plains, flower plains and the savanna.',
		behaviour:
			'Feed it carrots, rye or apples to tame it, put a saddle on it and ride. Each horse has its own speed, jump and endurance, read from its coat. See <a href="/guides/animals">riding</a>.',
		speed: '1.5 walking; 6 to 12 ridden'
	},
	Goat: {
		summary: 'A hardy grazer of the heights and dry lands.',
		spawns: 'In herds of 2 to 4 in the mountains and the savanna, and in savanna and desert village pens.',
		behaviour: 'It eats rye.',
		speed: '1.2 walking, 3 fleeing'
	},
	Cat: {
		summary: 'Keeps the vermin away.',
		spawns: 'In groups of 1 to 2 in forests, groves and forest clearings.',
		behaviour:
			'Tame it with raw chicken or rabbit. A tamed cat follows you, and Gnawers and Gust Owls keep 8 blocks away from it.',
		speed: '1.6 walking, 4 fleeing'
	},
	Gnawer: {
		summary: 'A cave rat that hunts in packs.',
		spawns: 'In the dark underground below y 50, often in packs of 2 to 4, and as a guard of ruins and crypts.',
		behaviour:
			'It is weak alone but quick and comes in numbers. Its bite weakens you. It keeps away from a tamed cat. Its fang is the ingredient of the potion of strength.',
		damage: 2,
		effect: 'Weakness I for 10 s',
		xp: 3,
		speed: '2.8 hunting',
		sight: '16 / 24'
	},
	Bramble: {
		summary: 'A bush that bites whoever comes close at night.',
		spawns: 'At night on grass in the woods, where it is one of two (with the Stumpling); as a guard of ruins.',
		behaviour:
			'It never moves: it waits for you to come within 2.5 blocks, then strikes and poisons you. Poison never kills, but leaves you at one point of health.',
		damage: 3,
		effect: 'Poison I for 6 s',
		xp: 4,
		speed: 'Never moves',
		sight: '2.5'
	},
	Roller: {
		summary: 'A boulder that winds up and rolls at its prey.',
		spawns: 'Above y 100 in the mountains, and in the dark caves; as a guard of sanctuaries.',
		behaviour:
			'It winds up for a second, then rolls at you at 7 blocks a second for up to three seconds, knocking you far back. If it hits a block, it is stunned for two seconds: that is the moment to strike. Its charge knocks a shield down.',
		damage: 4,
		xp: 5,
		speed: '7 rolling',
		sight: '12 / 24'
	},
	Stumpling: {
		summary: 'A walking stump that comes out of the grass at night.',
		spawns: 'At night on grass. It sinks back into the ground at dawn.',
		behaviour:
			'Its bark turns away blows from the front: an axe knocks its guard down for four seconds, and axes deal half again their damage to it. It flees fire and lava within 6 blocks, and it is the only creature that opens doors.',
		damage: 3,
		xp: 5,
		speed: '1.2 hunting',
		sight: '16 / 24'
	},
	Drowner: {
		summary: 'A drowned thing that drags swimmers down.',
		spawns: 'In water at least 3 blocks deep, at any hour: one time in four by day, one in two at night.',
		behaviour:
			'It swims fast at you and grabs you: you are slowed and pulled down for two seconds, unable to swim up. On land it is slow and goes back to the water.',
		damage: 2,
		effect: 'Slowness II for 6 s, and dragged down',
		xp: 6,
		speed: '3 in water, 0.8 on land',
		sight: '16 / 24'
	},
	Cinderling: {
		summary: 'A burning rock that hovers in the deep and throws embers.',
		spawns: 'In the dark of the Depths and below; summoned by the Ember Mother; as a guard of sanctuaries.',
		behaviour:
			'It floats 2 blocks up, keeps 6 blocks away from you and throws embers in arcs from up to 16 blocks. Embers set you burning and light what they land on. Water kills it, and so does rain on one under the sky: then it drops nothing.',
		damage: 1,
		effect: 'Burning, then Burn I for 6 s',
		xp: 6,
		speed: '2',
		sight: '16 / 24'
	},
	GustOwl: {
		summary: 'An owl of the peaks that circles over its prey at night and dives.',
		spawns: 'At night under the open sky above y 110.',
		behaviour:
			'It circles 8 blocks above you, then dives from 6 to 10 blocks up at 10 blocks a second and knocks you back: dangerous on a cliff. It keeps away from a tamed cat.',
		damage: 2,
		xp: 6,
		speed: '4 circling, 10 diving',
		sight: '24 / 32'
	},
	Farmer: {
		summary: 'A villager who buys crops and sells food.',
		spawns: 'In villages: each house gets one villager, the professions in turn.',
		behaviour:
			'Villagers walk around their home by day and go in at night. Nothing hurts them and hostiles ignore them. Use one to trade. See <a href="/guides/trading">trading</a>.',
		speed: '1.5'
	},
	Smith: {
		summary: 'A villager who buys coal and iron and sells iron tools and armour.',
		spawns: 'In villages: each house gets one villager, the professions in turn.',
		behaviour:
			'Villagers walk around their home by day and go in at night. Nothing hurts them and hostiles ignore them. Use one to trade. See <a href="/guides/trading">trading</a>.',
		speed: '1.5'
	},
	Cartographer: {
		summary: 'A villager who sells torches, and tells where the nearest dungeon is.',
		spawns: 'In villages: each house gets one villager, the professions in turn.',
		behaviour:
			'Villagers walk around their home by day and go in at night. Nothing hurts them and hostiles ignore them. Use one to trade. See <a href="/guides/trading">trading</a>.',
		speed: '1.5'
	},
	EmberMother: {
		summary: 'The mother of the Cinderlings, asleep in her lair at the bottom of the world.',
		spawns: 'Once per world, in her lair by the lava. She wakes when a player in survival enters it.',
		behaviour:
			'She throws volleys of embers and calls Cinderlings out of the lava; below half her health she charges, strikes hard and sets the floor on fire. She has 80 more health for each player after the first. See <a href="/guides/ember-mother">the boss guide</a>.',
		damage: 7,
		xp: 500,
		speed: '3, 3.8 in fury',
		sight: '36'
	},
	Glintfin: {
		summary: 'Small bright fish of the reefs, in shoals.',
		spawns: 'In shoals of 3 to 6 over reefs and in water less than 10 deep.',
		behaviour: 'It darts away when hurt.',
		xp: 1,
		speed: '1.6 swimming, 4.5 darting'
	},
	Shoalback: {
		summary: 'Silver fish of the open sea, in shoals.',
		spawns: 'In shoals of 3 to 6 in the open sea.',
		behaviour: 'It darts away when hurt.',
		xp: 1,
		speed: '1.6 swimming, 4.5 darting'
	},
	Mossback: {
		summary: 'A slow shelled grazer of the damp caves, its back grown with moss.',
		spawns: 'In the lush caves of the deep, with its chunk.',
		behaviour: 'It never attacks. Its moss makes potions last longer at the alembic.',
		xp: 2,
		speed: '0.7 walking, 1.2 fleeing'
	},
	Shardling: {
		summary: 'A crab of crystal that guards the crystal caverns.',
		spawns: 'In the crystal caves of the deep, with its chunk, and in the dark on geodestone.',
		behaviour: 'It pinches and knocks you back. Its shards make potions stronger at the alembic.',
		damage: 3,
		xp: 5,
		speed: '2.2 hunting',
		sight: '12 / 20'
	},
	Slagcrawler: {
		summary: 'A crawler of slag from the basalt galleries.',
		spawns: 'In the dark on basalt or magma, in the basalt galleries of the deep.',
		behaviour: 'Its bite sets you burning. Fire resistance makes it harmless.',
		damage: 2,
		effect: 'Burn I for 4 s',
		xp: 6,
		speed: '2.4 hunting',
		sight: '14 / 22'
	},
	Donkey: {
		summary: 'A steady steed that carries a chest.',
		spawns: 'In groups of 1 to 3 in the plains.',
		behaviour:
			'Tame it like a horse. It is slower but lasts longer at a gallop, and with a chest on its back it carries 27 slots. With a horse, it breeds a mule.',
		speed: '1.3 walking; 6 to 7 ridden'
	},
	Mule: {
		summary: 'Born of a horse and a donkey.',
		spawns: 'Never in the wild: breed a horse with a donkey.',
		behaviour: 'It carries a chest like the donkey, and never breeds.',
		speed: '1.3 walking'
	}
};
