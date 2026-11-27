import { game, items, withRefs } from '#lib/server/game.ts';

const ORDER = ['Stations', 'Tools', 'Materials', 'Wood', 'Blocks', 'Nature', 'Ores', 'Mobs'];

export function load() {
	const recipes = game.recipes.map((r) => ({ ...r, category: items.get(r.output)?.category ?? 'Blocks' }));
	return withRefs({ recipes, order: ORDER, tags: game.tags });
}
