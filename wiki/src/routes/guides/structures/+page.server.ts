import { withRefs, game, biomeSlug, biomeName } from '#lib/server/game.ts';

type Loot = { item: string; chance: number; min: number; max: number };
const t = (rows: [string, number, number, number][]): Loot[] =>
	rows.map(([item, chance, min, max]) => ({ item, chance, min, max }));

// The tables of the containers, as the game rolls them: `generation/village/loot.rs`,
// `generation/dungeon/loot.rs` and `generation/ocean.rs`. Each row rolls on its own.
const VILLAGE = [
	{
		name: 'Forge',
		loot: t([
			['BronzeIngot', 0.7, 1, 4],
			['CopperIngot', 0.5, 1, 5],
			['TinIngot', 0.4, 1, 3],
			['IronIngot', 0.25, 1, 3],
			['Coal', 0.6, 2, 8],
			['BronzePickaxe', 0.15, 1, 1],
			['BronzeSword', 0.15, 1, 1],
			['BronzeAxe', 0.1, 1, 1],
			['Flint', 0.3, 1, 3]
		])
	},
	{
		name: 'Granary',
		loot: t([
			['Rye', 0.9, 4, 16],
			['RyeBread', 0.6, 1, 4],
			['Berries', 0.4, 2, 8],
			['Fiber', 0.3, 2, 6]
		])
	},
	{
		name: 'Houses and other buildings',
		loot: t([
			['RyeBread', 0.6, 1, 3],
			['Berries', 0.4, 1, 5],
			['Torch', 0.4, 2, 6],
			['Stick', 0.3, 2, 6],
			['Wool', 0.2, 1, 3],
			['Rope', 0.2, 1, 2],
			['WoodenHoe', 0.1, 1, 1]
		])
	}
];

const DUNGEONS = [
	{
		name: 'Ruin',
		loot: t([
			['RyeBread', 0.6, 1, 3],
			['Torch', 0.6, 2, 8],
			['Rope', 0.4, 1, 3],
			['Coal', 0.5, 2, 6],
			['CopperIngot', 0.5, 1, 4],
			['TinIngot', 0.3, 1, 3],
			['BronzeIngot', 0.3, 1, 3],
			['BronzeSword', 0.15, 1, 1],
			['BronzePickaxe', 0.1, 1, 1],
			['Spear', 0.15, 1, 1],
			['FlintAndSteel', 0.1, 1, 1],
			['Arrow', 0.35, 2, 8],
			['Bow', 0.12, 1, 1],
			['LeatherHelmet', 0.1, 1, 1],
			['LeatherBoots', 0.1, 1, 1],
			['BronzeHelmet', 0.05, 1, 1],
			['Saddle', 0.15, 1, 1],
			['LeatherHorseArmour', 0.1, 1, 1],
			['IronHorseArmour', 0.05, 1, 1],
			['Lead', 0.15, 1, 2]
		])
	},
	{
		name: 'Crypt',
		loot: t([
			['Torch', 0.5, 4, 10],
			['RyeBread', 0.4, 1, 3],
			['Coal', 0.5, 3, 10],
			['BronzeIngot', 0.5, 2, 5],
			['IronIngot', 0.7, 2, 6],
			['GoldIngot', 0.3, 1, 3],
			['IronSword', 0.2, 1, 1],
			['IronPickaxe', 0.15, 1, 1],
			['IronAxe', 0.1, 1, 1],
			['Lantern', 0.2, 1, 2],
			['Diamond', 0.06, 1, 1],
			['Arrow', 0.5, 4, 12],
			['Bow', 0.2, 1, 1],
			['BronzeChestplate', 0.12, 1, 1],
			['IronHelmet', 0.1, 1, 1],
			['IronBoots', 0.1, 1, 1],
			['Saddle', 0.12, 1, 1],
			['IronHorseArmour', 0.1, 1, 1],
			['GoldHorseArmour', 0.06, 1, 1],
			['Lead', 0.1, 1, 1]
		])
	},
	{
		name: 'Sanctuary',
		loot: t([
			['IronIngot', 0.8, 3, 8],
			['GoldIngot', 0.8, 2, 6],
			['Diamond', 0.55, 1, 3],
			['GoldenSword', 0.15, 1, 1],
			['DiamondSword', 0.12, 1, 1],
			['DiamondPickaxe', 0.12, 1, 1],
			['DiamondAxe', 0.08, 1, 1],
			['Obsidian', 0.4, 2, 6],
			['GoldBlock', 0.1, 1, 1],
			['RoastBeef', 0.5, 1, 4],
			['Arrow', 0.6, 8, 20],
			['Bow', 0.25, 1, 1],
			['IronChestplate', 0.15, 1, 1],
			['IronLeggings', 0.15, 1, 1],
			['DiamondHelmet', 0.06, 1, 1],
			['DiamondBoots', 0.06, 1, 1]
		])
	}
];

const SEA = [
	{
		name: 'Shipwreck chest',
		loot: t([
			['Coal', 0.5, 2, 8],
			['IronIngot', 0.45, 1, 5],
			['GoldIngot', 0.25, 1, 3],
			['CopperIngot', 0.4, 2, 6],
			['Rope', 0.4, 1, 4],
			['Paper', 0.4, 2, 6],
			['Leather', 0.3, 1, 3],
			['RyeBread', 0.4, 1, 3],
			['Coin', 0.5, 2, 9],
			['Compass', 0.1, 1, 1],
			['Saddle', 0.08, 1, 1]
		])
	},
	{
		name: 'Buried treasure',
		loot: t([
			['GoldIngot', 0.9, 3, 9],
			['Diamond', 0.6, 1, 3],
			['IronIngot', 0.7, 3, 8],
			['Coin', 0.9, 8, 24],
			['GoldBlock', 0.15, 1, 1],
			['DiamondSword', 0.08, 1, 1],
			['CrystalShard', 0.3, 2, 6]
		])
	}
];

const STYLES: Record<string, string> = {
	Plains: 'Oak plank walls, oak log posts, thatch roofs; gravel streets',
	Savanna: 'Rammed earth walls, acacia posts and floors, thatch roofs; gravel streets',
	Desert: 'Rammed earth walls, hewn stone floors, brick posts and roofs; sand streets',
	Snowy: 'Spruce planks and logs, spruce roofs; gravel streets'
};

export function load() {
	const styles = Object.entries(STYLES).map(([style, look]) => ({
		style,
		look,
		biomes: game.biomes
			.filter((b) => b.village === style)
			.map((b) => ({ name: biomeName(b), slug: biomeSlug(b.key) }))
	}));
	return withRefs({ village: VILLAGE, dungeons: DUNGEONS, sea: SEA, styles }, [
		'Sheep',
		'Pig',
		'Chicken',
		'Cow',
		'Goat',
		'Rabbit',
		'Farmer',
		'Smith',
		'Cartographer',
		'Bramble',
		'Gnawer',
		'Roller',
		'Cinderling',
		'TreasureMap',
		'Chest',
		'Barrel',
		'Torch',
		'Lantern',
		'Campfire',
		'EmptyMap',
		'FilledMap',
		'DepthsCompass',
		'Ladder',
		'GoldBlock',
		'BronzeBlock',
		'Obsidian',
		'Rye'
	]);
}
