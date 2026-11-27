import { game, items, recipesFor, withRefs } from '#lib/server/game.ts';

/// The tool tiers, from the first to the last, by the material of their pickaxe and sword.
const TIERS = [
	{ pickaxe: 'WoodenPickaxe', sword: 'WoodenSword', material: '#Planks' },
	{ pickaxe: 'StonePickaxe', sword: 'StoneSword', material: '#Cobble' },
	{ pickaxe: 'BronzePickaxe', sword: 'BronzeSword', material: 'BronzeIngot' },
	{ pickaxe: 'IronPickaxe', sword: 'IronSword', material: 'IronIngot' },
	{ pickaxe: 'DiamondPickaxe', sword: 'DiamondSword', material: 'Diamond' },
	{ pickaxe: 'GoldenPickaxe', sword: 'GoldenSword', material: 'GoldIngot' }
];

const ARMOUR = ['Leather', 'Bronze', 'Iron', 'Diamond'];
const PIECES = ['Helmet', 'Chestplate', 'Leggings', 'Boots'];

/// The achievements in the order the game lists them (`Achievement::ALL` in
/// shared/src/players/progress.rs), with the item it shows each one as (`Achievement::icon`).
const ACHIEVEMENTS: [string, string][] = [
	['Tool', 'WoodenPickaxe'],
	['CraftingTable', 'CraftingTable'],
	['Furnace', 'Furnace'],
	['Night', 'Torch'],
	['Bed', 'Bedroll'],
	['Hostile', 'StoneSword'],
	['Bow', 'Bow'],
	['Armour', 'LeatherChestplate'],
	['FullSuit', 'IronChestplate'],
	['Iron', 'IronIngot'],
	['Trade', 'Coin'],
	['Animal', 'Wool'],
	['Caves', 'Cobblestone'],
	['Dungeon', 'HewnStone'],
	['Depths', 'Gloomrock'],
	['Diamond', 'Diamond'],
	['Enchantment', 'DiamondSword'],
	['FurnaceTier', 'LavaBucket'],
	['Compass', 'DepthsCompass'],
	['Lair', 'Obsidian'],
	['Boss', 'EmberHeart']
];

export function load() {
	const tools = TIERS.map((t) => {
		const pickaxe = items.get(t.pickaxe)!;
		return {
			...t,
			tier: pickaxe.tool!.tier,
			level: pickaxe.tool!.miningLevel,
			speed: pickaxe.tool!.speed,
			durability: pickaxe.durability ?? 0,
			damage: items.get(t.sword)!.attackDamage
		};
	});
	const armour = ARMOUR.map((material) => {
		const pieces = PIECES.map((p) => items.get(`${material}${p}`)!);
		return {
			material,
			pieces: pieces.map((p) => ({ id: p.id, points: p.armour!.points, durability: p.durability ?? 0 })),
			points: pieces.reduce((s, p) => s + p.armour!.points, 0)
		};
	});
	const byId = new Map(game.achievements.map((a) => [a.id, a]));
	const achievements = ACHIEVEMENTS.map(([id, icon]) => {
		const a = byId.get(id)!;
		return { id, icon, title: a.title.en ?? id, goal: a.goal.en ?? '' };
	});
	const first = (id: string) => recipesFor(id)[0];
	return withRefs(
		{
			tools,
			armour,
			achievements,
			recipes: {
				bronze: first('BronzeIngot'),
				engraving: first('EngravingTable'),
				alembic: first('Alembic'),
				compass: first('DepthsCompass')
			}
		},
		[
			'CopperOre',
			'TinOre',
			'IronOre',
			'GoldOre',
			'DiamondOre',
			'CoalOre',
			'Obsidian',
			'Furnace',
			'Anvil',
			'EmberMother',
			'Cinderling',
			'Gnawer',
			'Leather',
			'Cow',
			'Coin'
		]
	);
}
