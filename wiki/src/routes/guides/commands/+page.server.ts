import { game, withRefs, biomeSlug, biomeName } from '#lib/server/game.ts';

/// The name `/give` takes, as `ItemId::name` writes it: `IronIngot` is `iron_ingot`.
const commandName = (id: string) => id.replace(/(?<!^)([A-Z])/g, '_$1').toLowerCase();

export function load() {
	const biomes = game.biomes.map((b) => ({
		key: b.key.replace(/^rustcraft:/, ''),
		name: biomeName(b),
		href: `/biomes/${biomeSlug(b.key)}`
	}));
	// The items whose command name is not their English name in snake case
	const renamed = game.items
		.filter((i) => i.name.en && commandName(i.id) !== i.name.en.toLowerCase().replace(/ /g, '_'))
		.map((i) => ({ item: i.id, command: commandName(i.id) }));
	return withRefs({ biomes, renamed }, ['OakLog', 'IronIngot', 'Compass']);
}
