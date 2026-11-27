import { game, items, withRefs } from '#lib/server/game.ts';

export function load() {
	const potions = game.alchemy
		.filter((a) => a.vial === 'NeutralBase')
		.map((a) => ({ ...a, potion: items.get(a.result)!.potion!, color: items.get(a.result)!.potionColor }));
	return withRefs(
		{
			base: game.alchemy.find((a) => a.vial === 'WaterVial')!,
			potions,
			longer: game.alchemyLonger,
			stronger: game.alchemyStronger,
			seconds: game.distillSeconds,
			alembic: game.recipes.find((r) => r.output === 'Alembic')!,
			vial: game.recipes.find((r) => r.output === 'GlassVial')!
		},
		['Alembic', 'GlassVial', 'WaterVial', 'MilkBucket']
	);
}
