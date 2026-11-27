import { game, withRefs } from '#lib/server/game.ts';

// The recharge of a blow, in ms, by class of weapon (`shared/src/players/melee.rs`)
const RECHARGE = { Sword: 500, Tool: 650, Spear: 750, Axe: 1000, Mace: 1400 } as const;

const recharge = (id: string, kind: string | undefined) => {
	if (id === 'Spear') return RECHARGE.Spear;
	if (id === 'Mace') return RECHARGE.Mace;
	if (id === 'ThrowingAxe' || kind === 'Axe') return RECHARGE.Axe;
	if (kind === 'Sword') return RECHARGE.Sword;
	return RECHARGE.Tool;
};

// The wear of a blow: a sword or the mace 1, another tool 2, the rest nothing
const wear = (id: string, kind: string | undefined) =>
	id === 'Mace' || kind === 'Sword' ? 1 : kind ? 2 : 0;

const MELEE = [
	...['Wooden', 'Golden', 'Stone', 'Bronze', 'Iron', 'Diamond'].map((t) => `${t}Sword`),
	...['Wooden', 'Golden', 'Stone', 'Bronze', 'Iron', 'Diamond'].map((t) => `${t}Axe`),
	'IronPickaxe',
	'Spear',
	'ThrowingAxe',
	'Mace'
];

// How hard each hostile strikes, in points, on Normal (`shared/src/world/hostiles.rs`)
const STRIKES: [string, number][] = [
	['Gnawer', 2],
	['Bramble', 3],
	['Roller', 4],
	['Stumpling', 3],
	['Drowner', 2],
	['Cinderling', 1],
	['GustOwl', 2],
	['Shardling', 3],
	['Slagcrawler', 2],
	['EmberMother', 7]
];

// The points of the orb of a creature a player kills (`shared/src/players/experience.rs`)
const EXPERIENCE: [string, number][] = [
	['Glintfin', 1],
	['Shoalback', 1],
	['Mossback', 2],
	['Gnawer', 3],
	['Bramble', 4],
	['Roller', 5],
	['Stumpling', 5],
	['Shardling', 5],
	['Drowner', 6],
	['Cinderling', 6],
	['GustOwl', 6],
	['Slagcrawler', 6],
	['EmberMother', 500]
];

export function load() {
	const byId = new Map(game.items.map((i) => [i.id, i]));
	const melee = MELEE.map((id) => {
		const item = byId.get(id)!;
		const kind = item.tool?.kind;
		return {
			item: id,
			damage: item.attackDamage,
			recharge: recharge(id, kind),
			wear: wear(id, kind),
			durability: item.durability
		};
	});
	const armour = ['Leather', 'Bronze', 'Iron', 'Diamond'].map((material) => {
		const pieces = ['Helmet', 'Chestplate', 'Leggings', 'Boots'].map((piece) => {
			const item = game.items.find(
				(i) => i.armour?.material === material && i.armour.piece === piece
			)!;
			return { item: item.id, points: item.armour!.points, durability: item.armour!.durability };
		});
		return { material, pieces, total: pieces.reduce((s, p) => s + p.points, 0) };
	});
	return withRefs({ melee, armour, strikes: STRIKES, experience: EXPERIENCE }, [
		'IronSword',
		'Bow',
		'Arrow',
		'Crossbow',
		'Bolt',
		'ThrowingAxe',
		'Shield',
		'SplashPoisonPotion',
		'SplashSlownessPotion',
		'SplashWeaknessPotion',
		'EngravingTable',
		'Anvil',
		'Horse',
		'Saddle'
	]);
}
