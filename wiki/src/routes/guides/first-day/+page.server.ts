import { recipesFor, withRefs } from '#lib/server/game.ts';

/// The first recipe of `id`: the one the guide shows.
const first = (id: string) => recipesFor(id)[0];

export function load() {
	return withRefs(
		{
			wood: [first('OakPlanks'), first('Stick'), first('CraftingTable')],
			woodTools: [first('WoodenPickaxe'), first('WoodenAxe'), first('WoodenSword')],
			stoneTools: [first('StonePickaxe'), first('StoneSword'), first('Furnace')],
			torches: recipesFor('Torch'),
			campfire: first('Campfire'),
			bedroll: [first('Rope'), first('WhiteBedroll')],
			shelter: [first('Door'), first('Chest')]
		},
		[
			'OakLog',
			'Cobblestone',
			'Stone',
			'CoalOre',
			'TallGrass',
			'Fiber',
			'Wool',
			'Bedroll',
			'RawPork',
			'RawBeef',
			'RawMutton',
			'RawChicken',
			'RoastPork',
			'Apple',
			'Berries',
			'Pig',
			'Cow',
			'Sheep',
			'Chicken',
			'Stumpling',
			'Bramble',
			'Gnawer',
			'Torch',
			'Charcoal',
			'Coal'
		]
	);
}
