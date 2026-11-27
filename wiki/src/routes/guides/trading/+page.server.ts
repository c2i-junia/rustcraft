import { creatures, withRefs } from '#lib/server/game.ts';

export function load() {
	const villagers = creatures
		.filter((m) => m.villager)
		.map((m) => ({ id: m.id, trades: m.trades }));
	return withRefs({ villagers }, ['Coin', 'Compass']);
}
