import { creatures, withRefs } from '#lib/server/game.ts';
import { CREATURES } from '#lib/content/creatures.ts';

const group = (m: (typeof creatures)[number]) =>
	m.boss ? 'Boss' : m.hostile ? 'Hostile' : m.villager ? 'Villagers' : m.aquatic ? 'Fish' : 'Animals';

export function load() {
	const list = creatures.map((m) => ({
		id: m.id,
		group: group(m),
		health: m.health,
		summary: CREATURES[m.id]?.summary ?? '',
		damage: CREATURES[m.id]?.damage ?? null,
		xp: CREATURES[m.id]?.xp ?? 0
	}));
	return withRefs({ list, groups: ['Animals', 'Fish', 'Hostile', 'Boss', 'Villagers'] });
}
