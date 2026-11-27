// The index the search box reads: every page's title, where it is, and what it is.
import { json } from '@sveltejs/kit';
import { NAV } from '#lib/nav.ts';
import { biomeName, biomeSlug, creatures, game, refs, slugOf } from '#lib/server/game.ts';

export const prerender = true;

export function GET() {
	const entries = [
		...NAV.flatMap((s) => s.links.map((l) => ({ t: l.title, h: l.href, k: 'Guide', i: l.icon && refs[l.icon]?.icon ? `/textures/${refs[l.icon].icon}` : undefined }))),
		...game.items.map((i) => ({
			t: i.name.en ?? i.id,
			f: i.name.fr ?? undefined,
			h: `/items/${slugOf(i.id)}`,
			k: i.category,
			i: refs[i.id].icon ? `/textures/${refs[i.id].icon}` : undefined
		})),
		...creatures.map((m) => ({
			t: m.name.en ?? m.id,
			f: m.name.fr ?? undefined,
			h: `/creatures/${slugOf(m.id)}`,
			k: m.boss ? 'Boss' : m.hostile ? 'Hostile' : m.villager ? 'Villager' : 'Animal',
			i: refs[`mob:${m.id}`]?.picture
		})),
		...game.biomes.map((b) => ({ t: biomeName(b), h: `/biomes/${biomeSlug(b.key)}`, k: 'Biome' }))
	];
	return json(entries);
}
