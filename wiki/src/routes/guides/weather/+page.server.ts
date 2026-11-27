import { withRefs, game, biomeSlug, biomeName, recipesFor } from '#lib/server/game.ts';

export function load() {
	// What falls in each biome while it rains (`look.precipitation`), as the client draws it
	const biomes = (kind: string) =>
		game.biomes
			.filter((b) => b.look.precipitation === kind)
			.map((b) => ({ name: biomeName(b), slug: biomeSlug(b.key) }));
	return withRefs(
		{
			snow: biomes('Snow'),
			none: biomes('None'),
			bedroll: recipesFor('Bedroll')[0]
		},
		[
			'Bedroll',
			'RainCistern',
			'Campfire',
			'Cinderling',
			'Stumpling',
			'GustOwl',
			'Drowner',
			'Bramble',
			'Torch',
			'Rye',
			'Bucket',
			'WaterBucket',
			'Flint',
			'FlintAndSteel'
		]
	);
}
