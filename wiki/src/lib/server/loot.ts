// What creatures drop when they die, as `server/src/mob/combat.rs` (`kind_loot`) rolls it. The
// game rolls it in code, not in a table, so it is written again here: keep the two together.

export type LootEntry = {
	item: string;
	min: number;
	max: number;
	/// The chance of the drop, when it is not certain.
	chance?: number;
	note?: string;
};

export const LOOT: Record<string, LootEntry[]> = {
	Sheep: [
		{ item: 'RawMutton', min: 1, max: 2 },
		{ item: 'Wool', min: 1, max: 1, note: 'unless sheared' }
	],
	Pig: [{ item: 'RawPork', min: 1, max: 3 }],
	Cow: [
		{ item: 'RawBeef', min: 1, max: 3 },
		{ item: 'Leather', min: 0, max: 2 }
	],
	Chicken: [
		{ item: 'RawChicken', min: 1, max: 1 },
		{ item: 'Feather', min: 0, max: 2 }
	],
	Rabbit: [{ item: 'RawRabbit', min: 1, max: 1 }],
	Bramble: [{ item: 'Stick', min: 1, max: 3 }],
	Stumpling: [
		{ item: 'Stick', min: 2, max: 4 },
		{ item: 'Charcoal', min: 1, max: 1, chance: 0.3 }
	],
	Roller: [
		{ item: 'Cobblestone', min: 1, max: 2 },
		{ item: 'Flint', min: 1, max: 1, chance: 0.2 }
	],
	GustOwl: [{ item: 'Feather', min: 1, max: 3 }],
	Cinderling: [
		{ item: 'Coal', min: 1, max: 2 },
		{ item: 'Flint', min: 1, max: 1, chance: 0.1 }
	],
	EmberMother: [{ item: 'EmberHeart', min: 1, max: 1 }],
	Glintfin: [
		{ item: 'RawGlowfin', min: 1, max: 1, chance: 0.25 },
		{ item: 'RawRoach', min: 1, max: 1, chance: 0.75 }
	],
	Shoalback: [{ item: 'RawPerch', min: 1, max: 1 }],
	Mossback: [
		{ item: 'Moss', min: 1, max: 2 },
		{ item: 'Leather', min: 1, max: 1, chance: 1 / 3 }
	],
	Shardling: [{ item: 'CrystalShard', min: 1, max: 3 }],
	Slagcrawler: [
		{ item: 'Coal', min: 0, max: 2 },
		{ item: 'Magma', min: 1, max: 1, chance: 0.25 }
	],
	Horse: [
		{ item: 'Leather', min: 0, max: 2 },
		{ item: 'Saddle', min: 1, max: 1, note: 'if saddled' }
	],
	Donkey: [
		{ item: 'Leather', min: 0, max: 2 },
		{ item: 'Saddle', min: 1, max: 1, note: 'if saddled' },
		{ item: 'Chest', min: 1, max: 1, note: 'if it carries one' }
	],
	Mule: [
		{ item: 'Leather', min: 0, max: 2 },
		{ item: 'Saddle', min: 1, max: 1, note: 'if saddled' },
		{ item: 'Chest', min: 1, max: 1, note: 'if it carries one' }
	],
	Gnawer: [{ item: 'GnawerFang', min: 1, max: 1, chance: 0.5 }]
};
