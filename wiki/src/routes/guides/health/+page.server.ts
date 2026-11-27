import { withRefs } from '#lib/server/game.ts';

export function load() {
	return withRefs({}, [
		'RoastBeef',
		'Bedroll',
		'MilkBucket',
		'Bucket',
		'MushroomSoup',
		'Bowl',
		'FishChowder',
		'GrilledGlowfin',
		'WaterBreathingPotion',
		'FireResistancePotion',
		'HealingPotion',
		'RegenerationPotion',
		'Magma',
		'Ladder',
		'Horse'
	]);
}
