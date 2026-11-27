import { game, withRefs } from '#lib/server/game.ts';

// The recharge of each weapon kind, from `shared/src/players/melee.rs`
const RECHARGE: Record<string, number> = { Sword: 0.5, Axe: 1, Pickaxe: 0.65, Shovel: 0.65, Hoe: 0.65 };
const SPECIAL: Record<string, number> = { Spear: 0.75, ThrowingAxe: 1, Mace: 1.4 };

export function load() {
	const weapons = game.items
		.filter((i) => i.attackDamage > 1)
		.map((i) => {
			const recharge = SPECIAL[i.id] ?? (i.tool ? RECHARGE[i.tool.kind] : 0.65);
			return { id: i.id, damage: i.attackDamage, recharge, durability: i.durability, kind: i.tool?.kind ?? 'Special' };
		})
		.sort((a, b) => b.damage / b.recharge - a.damage / a.recharge);
	return withRefs({ weapons }, ['Bow', 'Crossbow', 'Arrow', 'Bolt', 'ThrowingAxe', 'Shield', 'Spear', 'Mace']);
}
