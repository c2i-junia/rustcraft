import { game, withRefs } from '#lib/server/game.ts';

// The order of the game's list, and the item each one is about
const ORDER: [string, string][] = [
	['Tool', 'WoodenPickaxe'],
	['CraftingTable', 'CraftingTable'],
	['Furnace', 'Furnace'],
	['Night', 'Torch'],
	['Bed', 'Bedroll'],
	['Hostile', 'StoneSword'],
	['Bow', 'Bow'],
	['Armour', 'LeatherHelmet'],
	['FullSuit', 'IronChestplate'],
	['Iron', 'IronIngot'],
	['Trade', 'Coin'],
	['Animal', 'Lead'],
	['Caves', 'Cobblestone'],
	['Dungeon', 'Chest'],
	['Depths', 'Gloomrock'],
	['Diamond', 'Diamond'],
	['Enchantment', 'EngravingTable'],
	['FurnaceTier', 'Magma'],
	['Compass', 'DepthsCompass'],
	['Lair', 'Campfire'],
	['Boss', 'EmberHeart']
];

export function load() {
	const byId = new Map(game.achievements.map((a) => [a.id, a]));
	const list = ORDER.filter(([id]) => byId.has(id)).map(([id, icon]) => ({ ...byId.get(id)!, icon }));
	// Any the list above does not know yet, at the end
	for (const a of game.achievements) if (!ORDER.some(([id]) => id === a.id)) list.push({ ...a, icon: 'Book' });
	return withRefs({ list });
}
