import { game, withRefs } from '#lib/server/game.ts';

export function load() {
	const smelts = game.items.filter((i) => i.smeltsInto).map((i) => ({ input: i.id, output: i.smeltsInto! }));
	const fuels = game.items
		.filter((i) => i.fuelSeconds)
		.map((i) => ({ id: i.id, seconds: i.fuelSeconds! }))
		.sort((a, b) => b.seconds - a.seconds);
	return withRefs({ smelts, fuels, cookSeconds: game.cookSeconds }, ['Furnace', 'Cobblestone', 'LavaBucket', 'Bucket']);
}
