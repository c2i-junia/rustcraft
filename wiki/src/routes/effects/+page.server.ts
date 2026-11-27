import { game, withRefs } from '#lib/server/game.ts';
import { CREATURES } from '#lib/content/creatures.ts';

export function load() {
	const effects = game.effects.map((e) => ({
		...e,
		potions: game.items.filter((i) => i.potion?.effect === e.id).map((i) => i.id),
		foods: game.items.filter((i) => i.foodEffect?.effect === e.id).map((i) => ({ id: i.id, ...i.foodEffect! })),
		mobs: Object.entries(CREATURES)
			.filter(([, f]) => f.effect && new RegExp(`^${e.id === 'Burn' ? 'Burn|Burning' : e.id}\\b`).test(f.effect.replace(/^Burning, then /, '')))
			.map(([id, f]) => ({ id, text: f.effect! }))
	}));
	return withRefs({ effects }, ['MilkBucket']);
}
