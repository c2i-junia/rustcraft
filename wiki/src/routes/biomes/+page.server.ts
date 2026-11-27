import { biomeName, biomeSlug, game, withRefs } from '#lib/server/game.ts';
import { BIOME_NOTES } from '#lib/content/biomes.ts';

export function load() {
	return withRefs({
		biomes: game.biomes.map((b) => ({
			name: biomeName(b),
			href: `/biomes/${biomeSlug(b.key)}`,
			colour: `rgb(${b.look.map_colour.join(' ')})`,
			note: BIOME_NOTES[b.key.replace('rustcraft:', '')] ?? '',
			surface: b.ground.surface,
			herds: [...new Set(b.herds.map((h) => h.kind))]
		}))
	});
}
