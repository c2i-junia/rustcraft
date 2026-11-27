import { error } from '@sveltejs/kit';
import { biomeByKey, biomeName, biomeSlug, game, withRefs } from '#lib/server/game.ts';
import { BIOME_NOTES } from '#lib/content/biomes.ts';
import type { EntryGenerator } from './$types';

const bySlug = new Map(game.biomes.map((b) => [biomeSlug(b.key), b]));
export const entries: EntryGenerator = () => game.biomes.map((b) => ({ slug: biomeSlug(b.key) }));

const TREE_SAPLING: Record<string, string> = {
	Oak: 'OakSapling',
	BigOak: 'OakSapling',
	Jungle: 'JungleSapling',
	Bush: 'JungleLeaves',
	Acacia: 'AcaciaSapling'
};
const link = (key: string) => {
	const b = biomeByKey.get(key);
	return b ? { name: biomeName(b), href: `/biomes/${biomeSlug(b.key)}` } : { name: key, href: '/biomes' };
};

export function load({ params }) {
	const b = bySlug.get(params.slug);
	if (!b) error(404, 'No biome has this name');
	const parents = game.biomes.filter(
		(o) =>
			o.sub_biomes.some(([k]) => k === b.key) ||
			o.border?.biome === b.key ||
			o.altitude_variants.some(([, k]) => k === b.key)
	);
	return withRefs(
		{
			biome: b,
			name: biomeName(b),
			note: BIOME_NOTES[b.key.replace('rustcraft:', '')] ?? '',
			colour: `rgb(${b.look.map_colour.join(' ')})`,
			trees: b.flora.trees.map(([tree, chance]) => ({ tree, sapling: TREE_SAPLING[tree] ?? null, chance })),
			subBiomes: b.sub_biomes.map(([k, share]) => ({ ...link(k), share })),
			altitude: b.altitude_variants.map(([h, k]) => ({ ...link(k), height: h })),
			parents: parents.map((p) => link(p.key)),
			border: b.border ? { ...link(b.border.biome) } : null
		},
		['Dandelion', 'Poppy', 'TallGrass', 'Fern', 'Cactus', 'Ice']
	);
}
