import { game, withRefs } from '#lib/server/game.ts';

export function load() {
	const repairs = Object.entries(
		game.items
			.filter((i) => i.repair)
			.reduce<Record<string, string[]>>((acc, i) => {
				(acc[i.repair!] ??= []).push(i.id);
				return acc;
			}, {})
	);
	return withRefs(
		{
			enchantments: game.enchantments,
			repairs,
			table: game.recipes.find((r) => r.output === 'EngravingTable') ?? null,
			anvil: game.recipes.find((r) => r.output === 'Anvil') ?? null
		},
		['EngravingTable', 'Anvil']
	);
}
