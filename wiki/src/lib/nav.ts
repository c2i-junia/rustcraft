// The wiki's table of contents: the sidebar, and the sections of the home page.
export type NavLink = { href: string; title: string; icon?: string; blurb?: string };
export type NavSection = { title: string; links: NavLink[] };

export const NAV: NavSection[] = [
	{
		title: 'Getting started',
		links: [
			{ href: '/guides/first-day', title: 'Your first day', icon: 'WoodenPickaxe', blurb: 'From bare hands to a roof before the first night.' },
			{ href: '/guides/controls', title: 'Controls', icon: 'Compass', blurb: 'Every key, and how to change them.' },
			{ href: '/guides/game-modes', title: 'Game modes', icon: 'Grass', blurb: 'Survival, creative, and the difficulty.' },
			{ href: '/guides/progression', title: 'Progression', icon: 'Diamond', blurb: 'The road from wood to the Ember Mother.' }
		]
	},
	{
		title: 'World',
		links: [
			{ href: '/biomes', title: 'Biomes', icon: 'OakSapling', blurb: 'Eighteen climates, from deserts to ice spikes.' },
			{ href: '/guides/depths', title: 'The depths', icon: 'Gloomrock', blurb: 'The tiers below the surface, and what waits there.' },
			{ href: '/guides/ores', title: 'Ores', icon: 'IronOre', blurb: 'Where each ore lies, and what mines it.' },
			{ href: '/guides/structures', title: 'Structures', icon: 'Chest', blurb: 'Villages, dungeons, and their loot.' },
			{ href: '/guides/weather', title: 'Day, night and weather', icon: 'Snowball', blurb: 'The clock, the rain, the storms.' }
		]
	},
	{
		title: 'Items',
		links: [
			{ href: '/items', title: 'All items', icon: 'CraftingTable', blurb: 'Every block and item, searchable.' },
			{ href: '/tools', title: 'Tools', icon: 'IronPickaxe', blurb: 'Tiers, mining levels and durability.' },
			{ href: '/weapons', title: 'Weapons', icon: 'IronSword', blurb: 'Swords, axes, the bow and the spear.' },
			{ href: '/armour', title: 'Armour', icon: 'IronChestplate', blurb: 'Points, absorption and durability.' },
			{ href: '/food', title: 'Food', icon: 'Apple', blurb: 'What each food restores.' }
		]
	},
	{
		title: 'Crafting',
		links: [
			{ href: '/crafting', title: 'Crafting', icon: 'CraftingTable', blurb: 'The grid, and every recipe.' },
			{ href: '/smelting', title: 'Smelting', icon: 'Furnace', blurb: 'The furnace, its fuels, its results.' },
			{ href: '/alchemy', title: 'Alchemy', icon: 'Alembic', blurb: 'The alembic and every potion.' },
			{ href: '/enchanting', title: 'Enchanting and the anvil', icon: 'EngravingTable', blurb: 'Engrave, repair, rename.' }
		]
	},
	{
		title: 'Creatures',
		links: [
			{ href: '/creatures', title: 'All creatures', icon: 'SheepSpawnEgg', blurb: 'Animals, monsters, villagers.' },
			{ href: '/guides/animals', title: 'Taming, breeding and riding', icon: 'Saddle', blurb: 'Make friends, raise herds, ride.' },
			{ href: '/guides/trading', title: 'Villagers and trade', icon: 'Coin', blurb: 'What villagers buy and sell.' },
			{ href: '/guides/ember-mother', title: 'The Ember Mother', icon: 'EmberHeart', blurb: 'The boss in the deepest lair.' }
		]
	},
	{
		title: 'Survival',
		links: [
			{ href: '/guides/health', title: 'Health and hunger', icon: 'RoastBeef', blurb: 'Hearts, hunger, falls, fire and air.' },
			{ href: '/guides/combat', title: 'Combat', icon: 'Shield', blurb: 'Blows, blocking, arrows and armour.' },
			{ href: '/effects', title: 'Status effects', icon: 'HealingPotion', blurb: 'Every effect, and where it comes from.' },
			{ href: '/guides/farming', title: 'Farming', icon: 'Rye', blurb: 'Crops, saplings and growth.' },
			{ href: '/guides/fishing', title: 'Fishing', icon: 'FishingRod', blurb: 'Rods, bites and catches.' },
			{ href: '/achievements', title: 'Achievements', icon: 'Book', blurb: 'All of them, and how to earn each.' }
		]
	},
	{
		title: 'Playing together',
		links: [
			{ href: '/guides/multiplayer', title: 'Multiplayer', icon: 'Bedroll', blurb: 'Join or run a server.' },
			{ href: '/guides/commands', title: 'Commands', icon: 'Book', blurb: 'Every chat command.' }
		]
	}
];
