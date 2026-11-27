import { game, withRefs } from '#lib/server/game.ts';

export function load() {
	const foods = game.items
		.filter((i) => i.food)
		.map((i) => ({
			id: i.id,
			hunger: i.food!.hunger,
			saturation: i.food!.saturation,
			effect: i.foodEffect,
			remains: i.eatenRemains,
			cooked: game.items.find((o) => o.smeltsInto === i.id)?.id ?? null,
			crafted: game.recipes.some((r) => r.output === i.id)
		}))
		.sort((a, b) => b.hunger + b.saturation - (a.hunger + a.saturation));
	return withRefs({ foods }, ['MilkBucket']);
}
