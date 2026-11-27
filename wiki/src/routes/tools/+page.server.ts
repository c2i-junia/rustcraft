import { game, withRefs } from '#lib/server/game.ts';

const TIERS = ['Wood', 'Gold', 'Stone', 'Bronze', 'Iron', 'Diamond'];
const KINDS = ['Pickaxe', 'Axe', 'Shovel', 'Hoe', 'Sword'];

export function load() {
	const tools = game.items.filter((i) => i.tool);
	const tiers = TIERS.map((tier) => {
		const of = tools.filter((t) => t.tool!.tier === tier);
		const one = of[0];
		return {
			tier,
			speed: one?.tool!.speed ?? 0,
			level: one?.tool!.miningLevel ?? 0,
			durability: one?.durability ?? 0,
			kinds: KINDS.map((k) => of.find((t) => t.tool!.kind === k)?.id ?? null)
		};
	});
	// The blocks that need a pickaxe of each level, as examples
	const needs = [0, 1, 2, 3].map((level) =>
		game.blocks
			.filter((b) => b.requiredTool?.level === level && game.items.some((i) => i.id === b.id))
			.map((b) => b.id)
			.slice(0, 14)
	);
	return withRefs({ tiers, kinds: KINDS, needs }, ['FlintAndSteel', 'Shears', 'FishingRod', 'PortalGun', 'Bucket', 'Compass', 'DepthsCompass']);
}
