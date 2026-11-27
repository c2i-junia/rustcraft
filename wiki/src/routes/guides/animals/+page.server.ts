import { creatures, game, recipesFor, withRefs } from '#lib/server/game.ts';

/// The coats of the horses and what each gives on average, from `Coat::base` in
/// `shared/src/world/mobs.rs`: speed in blocks a second, endurance in seconds of gallop.
const COATS = [
	{ coat: 'Bay', speed: 8.5, endurance: 8 },
	{ coat: 'Chestnut', speed: 9.5, endurance: 6.5 },
	{ coat: 'Black', speed: 10, endurance: 5 },
	{ coat: 'Grey', speed: 8, endurance: 11 },
	{ coat: 'White', speed: 9, endurance: 8.5 },
	{ coat: 'Dun', speed: 7.5, endurance: 13 }
];

export function load() {
	const animals = creatures
		.filter((m) => m.eats.length > 0)
		.map((m) => ({ id: m.id, eats: m.eats, tameable: m.tameable, mount: m.mount, chest: m.carriesChest }));
	const armour = game.items
		.filter((i) => i.horseArmour !== null)
		.map((i) => ({ id: i.id, points: i.horseArmour ?? 0 }));
	return withRefs(
		{
			animals,
			coats: COATS,
			armour,
			saddle: recipesFor('Saddle'),
			lead: recipesFor('Lead'),
			boat: recipesFor('Boat')
		},
		['Sheep', 'Cow', 'Chicken', 'Wolf', 'Cat', 'Horse', 'Donkey', 'Mule', 'Gnawer', 'GustOwl', 'Wool', 'Bucket', 'MilkBucket', 'Egg', 'FriedEgg', 'Chest', 'Fence', 'Rye', 'Carrot', 'Apple', 'Boat', 'Mossback']
	);
}
