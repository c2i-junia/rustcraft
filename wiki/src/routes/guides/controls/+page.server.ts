import { withRefs } from '#lib/server/game.ts';

export function load() {
	return withRefs({}, ['CraftingTable', 'Bow', 'Shield', 'Chest', 'Donkey', 'Mule']);
}
