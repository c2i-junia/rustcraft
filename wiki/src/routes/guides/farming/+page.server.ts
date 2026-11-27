import { blocks, recipesFor, withRefs } from '#lib/server/game.ts';

/// A cell gets a random tick about once every 1024 ticks: 4 cells of each 16³ chunk a tick
/// (`RANDOM_TICKS_PER_SECTION`), and a crop grows a stage one tick in 4, or 2 when watered
/// (`CROP_ODDS` in `shared/src/world/growth.rs`).
const TICK_SECONDS = 4096 / 4 / 20;
const CROP_ODDS = 4;

const CROPS = [
	{ seed: 'Rye', stages: 2, ripe: 'RipeRye' },
	{ seed: 'Carrot', stages: 3, ripe: 'RipeCarrots' },
	{ seed: 'Potato', stages: 3, ripe: 'RipePotatoes' }
];

export function load() {
	const crops = CROPS.map((c) => {
		const drops = blocks.get(c.ripe)?.drops ?? [];
		const counts = drops.map((d) => d.count);
		return {
			seed: c.seed,
			stages: c.stages,
			minutes: (c.stages * CROP_ODDS * TICK_SECONDS) / 60,
			harvest: { min: Math.min(...counts), max: Math.max(...counts) }
		};
	});
	const grass = blocks.get('TallGrass')?.drops ?? [];
	const total = grass.reduce((s, d) => s + d.chance, 0);
	return withRefs(
		{
			crops,
			grass: grass.map((d) => ({ item: d.item, share: d.chance / total })),
			stageMinutes: (CROP_ODDS * TICK_SECONDS) / 60,
			hoes: recipesFor('WoodenHoe'),
			compost: recipesFor('Compost'),
			cistern: recipesFor('RainCistern'),
			bread: recipesFor('RyeBread')
		},
		['Dirt', 'Grass', 'StoneHoe', 'IronHoe', 'BerryBush', 'Berries', 'OakSapling', 'SpruceSapling', 'JungleSapling', 'WillowSapling', 'AcaciaSapling', 'AppleSapling', 'Apple', 'OakLeaves', 'AppleLeaves', 'Bucket', 'WaterBucket', 'Reed', 'Fiber', 'BakedPotato', 'Campfire']
	);
}
