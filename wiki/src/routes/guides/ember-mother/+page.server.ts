import { mobs, recipesFor, withRefs } from '#lib/server/game.ts';

/// From `server/src/mob/boss.rs`: her health grows by `HEALTH_PER_PLAYER` for each player inside
/// but the first, and she calls at most one Cinderling more than the players, up to `MOST_CALLED`.
const HEALTH_PER_PLAYER = 80;
const MOST_CALLED = 5;

export function load() {
	const base = mobs.get('EmberMother')?.health ?? 0;
	const scaling = [1, 2, 3, 4, 5].map((players) => ({
		players,
		health: base + HEALTH_PER_PLAYER * (players - 1),
		cinderlings: Math.min(1 + players, MOST_CALLED)
	}));
	return withRefs({ base, scaling, compass: recipesFor('DepthsCompass') }, [
		'EmberMother',
		'Cinderling',
		'EmberHeart',
		'DepthsCompass',
		'Obsidian',
		'Campfire',
		'HewnStone',
		'FireResistancePotion',
		'HealingPotion',
		'RegenerationPotion',
		'StrengthPotion',
		'Moss',
		'Magma',
		'DiamondChestplate',
		'IronChestplate',
		'Shield',
		'Bow',
		'Arrow',
		'DiamondSword',
		'RoastBeef',
		'Stew',
		'WaterBucket',
		'Bedroll'
	]);
}
