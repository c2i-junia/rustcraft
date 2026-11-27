import { withRefs } from '#lib/server/game.ts';

export function load() {
	return withRefs({}, ['Compass', 'Bedroll', 'Bow', 'Crossbow', 'ThrowingAxe']);
}
