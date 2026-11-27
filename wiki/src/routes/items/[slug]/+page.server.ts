import { error } from '@sveltejs/kit';
import {
	blocks,
	blockSources,
	creatures,
	game,
	members,

	mobSources,
	recipesFor,
	slugOf,
	smeltingInputs,
	tradesWith,
	usesOf,
	withRefs
} from '#lib/server/game.ts';
import { FISHING } from '#lib/server/fishing.ts';

import { ITEM_NOTES } from '#lib/content/items.ts';
import type { EntryGenerator } from './$types';

const bySlug = new Map(game.items.map((i) => [slugOf(i.id), i]));

export const entries: EntryGenerator = () => game.items.map((i) => ({ slug: slugOf(i.id) }));

export function load({ params }) {
	const item = bySlug.get(params.slug);
	if (!item) error(404, 'No item has this name');
	const block = item.block ? (blocks.get(item.block) ?? null) : null;
	const fishing = FISHING.flatMap((pool) =>
		pool.entries.filter((e) => e.item === item.id).map((e) => ({ pool: pool.name, share: pool.share, ...e, total: pool.entries.reduce((s, x) => s + x.weight, 0) }))
	);
	const alchemyMade = game.alchemy.filter((a) => a.result === item.id);
	const alchemyUses = game.alchemy.filter((a) => a.ingredient === item.id || a.vial === item.id);
	const modifier =
		item.id === game.alchemyLonger ? 'longer' : item.id === game.alchemyStronger ? 'stronger' : null;
	const eatenBy = creatures.filter((m) => m.eats.includes(item.id)).map((m) => m.id);
	const repairs = game.items.filter((i) => i.repair !== null && members(i.repair).includes(item.id)).map((i) => i.id);
	const enchantments = game.enchantments.filter((e) => item.enchantments.includes(e.id));
	const ownRepair = item.repair;
	return withRefs({
		item,
		block,
		recipes: recipesFor(item.id),
		uses: usesOf(item.id),
		smeltedFrom: smeltingInputs(item.id).map((i) => i.id),
		blockSources: blockSources(item.id),
		mobSources: mobSources(item.id),
		trades: tradesWith(item.id),
		fishing,
		alchemyMade,
		alchemyUses,
		modifier,
		eatenBy,
		repairs,
		ownRepair,
		enchantments,
		spawns: item.spawns,
		placedBlockDrops: block?.drops ?? [],
		notes: ITEM_NOTES[item.id] ?? null,
		cookSeconds: game.cookSeconds,
		distillSeconds: game.distillSeconds,
		breakSeconds: block?.breakTime != null ? block.breakTime * 0.05 : null,
		bestTools: block ? bestTools(block.fasterWith, block.requiredTool) : []
	});
}

/// The tools of each tier that break the block faster, and how long each takes.
function bestTools(kinds: string[], required: { kind: string; level: number } | null) {
	return game.items
		.filter((i) => i.tool && kinds.includes(i.tool.kind))
		.map((i) => ({
			id: i.id,
			speed: i.tool!.speed,
			harvests: !required || (required.kind === i.tool!.kind && i.tool!.miningLevel >= required.level)
		}))
		.sort((a, b) => a.speed - b.speed);
}

export const prerender = true;
