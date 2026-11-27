import { withRefs } from '#lib/server/game.ts';
import { FISHING } from '#lib/server/fishing.ts';

export function load() {
	return withRefs({ pools: FISHING }, ['FishingRod', 'Stick', 'Rope', 'Glintfin', 'Shoalback']);
}
