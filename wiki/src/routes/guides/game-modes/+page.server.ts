import { withRefs } from '#lib/server/game.ts';

// The base blows of a few hostiles, to show what each difficulty makes of them
// (the `Strike` of each kind in shared/src/world/hostiles.rs)
const BLOWS = [
	{ mob: 'Gnawer', damage: 2 },
	{ mob: 'Bramble', damage: 3 },
	{ mob: 'Roller', damage: 4 },
	{ mob: 'EmberMother', damage: 7 }
];

export function load() {
	return withRefs({ blows: BLOWS }, []);
}
