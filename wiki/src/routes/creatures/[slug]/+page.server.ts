import { error } from '@sveltejs/kit';
import { biomeName, biomeSlug, creatures, game, lootOf, slugOf, withRefs } from '#lib/server/game.ts';
import { CREATURES } from '#lib/content/creatures.ts';
import type { EntryGenerator } from './$types';

const bySlug = new Map(creatures.map((m) => [slugOf(m.id), m]));

export const entries: EntryGenerator = () => creatures.map((m) => ({ slug: slugOf(m.id) }));

export function load({ params }) {
	const mob = bySlug.get(params.slug);
	if (!mob) error(404, 'No creature has this name');
	const habitats = game.biomes.flatMap((b) =>
		b.herds
			.filter((h) => h.kind === mob.id)
			.map((h) => ({ name: biomeName(b), href: `/biomes/${biomeSlug(b.key)}`, chance: h.chance, fewest: h.fewest, most: h.most }))
	);
	return withRefs(
		{
			mob,
			facts: CREATURES[mob.id] ?? null,
			loot: lootOf(mob.id),
			habitats
		},
		[`mob:${mob.id}`]
	);
}
