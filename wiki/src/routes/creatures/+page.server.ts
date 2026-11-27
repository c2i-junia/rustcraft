import { creatures, factsOf, withRefs } from '#lib/server/game.ts';

const group = (m: (typeof creatures)[number]) =>
	m.boss ? 'Boss' : m.hostile ? 'Hostile' : m.villager ? 'Villagers' : m.aquatic ? 'Fish' : 'Animals';

export function load() {
	const list = creatures.map((m) => ({
		id: m.id,
		group: group(m),
		health: m.health,
		summary: factsOf(m).summary,
		damage: m.damage,
		xp: m.xp
	}));
	return withRefs({ list, groups: ['Animals', 'Fish', 'Hostile', 'Boss', 'Villagers'] });
}
