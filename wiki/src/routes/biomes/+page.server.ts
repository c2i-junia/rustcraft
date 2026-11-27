import { biomeName, biomeNote, biomeSlug, game, withRefs } from '#lib/server/game.ts';

export function load() {
	return withRefs({
		biomes: game.biomes.map((b) => ({
			name: biomeName(b),
			href: `/biomes/${biomeSlug(b.key)}`,
			colour: `rgb(${b.look.map_colour.join(' ')})`,
			note: biomeNote(b.key),
			surface: b.ground.surface,
			herds: [...new Set(b.herds.map((h) => h.kind))]
		}))
	});
}
