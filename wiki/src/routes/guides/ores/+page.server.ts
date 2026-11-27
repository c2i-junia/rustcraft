import { withRefs, blocks, items } from '#lib/server/game.ts';

type Span = { from: number; to: number; tries: number };

// As `server/src/world/generation/ores.rs` scatters them: `DEEP_ORES` for a new world,
// `ORES` for one saved before the deep underground.
const ORES: {
	block: string;
	deep: Span;
	shallow: Span;
	cluster: [number, number];
	vein: [number, number] | null;
	veins: number;
}[] = [
	{ block: 'CoalOre', deep: { from: 0, to: 128, tries: 6 }, shallow: { from: 5, to: 128, tries: 6 }, cluster: [6, 14], vein: [8, 14], veins: 0.2 },
	{ block: 'CopperOre', deep: { from: -16, to: 96, tries: 5 }, shallow: { from: 5, to: 96, tries: 5 }, cluster: [5, 10], vein: [6, 12], veins: 0.3 },
	{ block: 'TinOre', deep: { from: -16, to: 64, tries: 4 }, shallow: { from: 5, to: 64, tries: 4 }, cluster: [4, 8], vein: [5, 10], veins: 0.3 },
	{ block: 'IronOre', deep: { from: -24, to: 72, tries: 4 }, shallow: { from: 5, to: 64, tries: 5 }, cluster: [4, 8], vein: [6, 12], veins: 0.4 },
	{ block: 'GoldOre', deep: { from: -60, to: -8, tries: 1 }, shallow: { from: 5, to: 32, tries: 1 }, cluster: [3, 6], vein: [6, 10], veins: 0.5 },
	{ block: 'DiamondOre', deep: { from: -58, to: 16, tries: 1 }, shallow: { from: 5, to: 16, tries: 1 }, cluster: [2, 6], vein: null, veins: 0 }
];

export function load() {
	const ores = ORES.map((ore) => {
		const block = blocks.get(ore.block)!;
		const drop = block.drops[0].item;
		return {
			...ore,
			level: block.requiredTool?.level ?? null,
			drop,
			smelted: items.get(drop)?.smeltsInto ?? null
		};
	});
	return withRefs({ ores }, [
		'Slate',
		'Limestone',
		'Gloomrock',
		'CobbledGloomrock',
		'Gravel',
		'Flint',
		'Arrow',
		'Spear',
		'FlintAndSteel',
		'StoneShovel',
		'IronPickaxe',
		'StonePickaxe',
		'BronzePickaxe',
		'BronzeIngot',
		'Diamond',
		'GoldIngot',
		'Roller',
		'Cinderling',
		'Furnace',
		'Torch',
		'DepthsCompass'
	]);
}
