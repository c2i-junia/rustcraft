import { game, withRefs } from '#lib/server/game.ts';

const PIECES = ['Helmet', 'Chestplate', 'Leggings', 'Boots'];
const MATERIALS = ['Leather', 'Bronze', 'Iron', 'Diamond'];

export function load() {
	const armour = game.items.filter((i) => i.armour);
	const sets = MATERIALS.map((material) => {
		const pieces = PIECES.map((p) => armour.find((a) => a.armour!.material === material && a.armour!.piece === p)!);
		return {
			material,
			pieces: pieces.map((p) => ({ id: p.id, points: p.armour!.points, durability: p.armour!.durability })),
			total: pieces.reduce((s, p) => s + p.armour!.points, 0)
		};
	});
	const horse = game.items.filter((i) => i.horseArmour).map((i) => ({ id: i.id, points: i.horseArmour! }));
	return withRefs({ sets, pieces: PIECES, horse }, ['Anvil', 'Leather', 'BronzeIngot', 'IronIngot', 'Diamond']);
}
