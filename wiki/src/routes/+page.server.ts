import { creatures, game, withRefs } from '#lib/server/game.ts';
import { NAV } from '#lib/nav.ts';

export function load() {
	return withRefs(
		{
			counts: {
				items: game.items.length,
				recipes: game.recipes.length,
				creatures: creatures.length,
				biomes: game.biomes.length
			}
		},
		NAV.flatMap((s) => s.links.map((l) => l.icon ?? '')).filter(Boolean)
	);
}
