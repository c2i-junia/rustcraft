import { game, withRefs } from '#lib/server/game.ts';

const ORDER = ['Blocks', 'Wood', 'Nature', 'Ores', 'Materials', 'Stations', 'Tools', 'Mobs'];

export function load() {
	return withRefs({
		categories: ORDER,
		items: game.items.map((i) => ({ id: i.id, category: i.category, fr: i.name.fr })),
		tags: game.tags
	});
}
