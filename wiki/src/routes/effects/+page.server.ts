import { creatures, factsOf, game, withRefs } from '#lib/server/game.ts';

export function load() {
	const effects = game.effects.map((e) => ({
		...e,
		potions: game.items.filter((i) => i.potion?.effect === e.id).map((i) => i.id),
		foods: game.items.filter((i) => i.foodEffect?.effect === e.id).map((i) => ({ id: i.id, ...i.foodEffect! })),
		mobs: creatures.filter((m) => m.effect?.effect === e.id).map((m) => ({ id: m.id, text: factsOf(m).effect! }))
	}));
	return withRefs({ effects }, ['MilkBucket']);
}
