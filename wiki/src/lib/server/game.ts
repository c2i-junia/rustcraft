// The game's data, as `shared/examples/wiki_export.rs` writes it, and the relations between its
// tables that the pages show: what makes an item, what it goes into, where it drops. Server-only:
// a page receives what it shows from its load, never the whole file.
import raw from '#lib/data/game.json';
import { plain, type Block as TextBlock, type Line } from '#lib/markup.ts';
import type { Ref } from '#lib/refs.ts';

export type Text = { en: string | null; fr: string | null };

export type Item = {
	id: string;
	slug: string;
	name: Text;
	category: string;
	icon: string | null;
	iconTop: string | null;
	block: string | null;
	maxStack: number;
	attackDamage: number;
	durability: number | null;
	food: { hunger: number; saturation: number } | null;
	foodEffect: { effect: string; level: number; seconds: number } | null;
	eatenRemains: string | null;
	tool: { kind: string; tier: string; speed: number; miningLevel: number } | null;
	armour: { piece: string; material: string; points: number; durability: number } | null;
	horseArmour: number | null;
	spawns: string | null;
	fuelSeconds: number | null;
	smeltsInto: string | null;
	potion: { effect: string; seconds: number; splash: boolean; stronger: boolean; longer: boolean } | null;
	potionColor: [number, number, number] | null;
	repair: string | null;
	enchantments: string[];
	/// What the Codex says of it, in markup (`codex.item.*` of `data/lang`).
	note: TextBlock[] | null;
};

export type Block = {
	id: string;
	breakTime: number | null;
	requiredTool: { kind: string; level: number } | null;
	fasterWith: string[];
	drops: { chance: number; item: string; count: number }[];
	light: number;
	container: boolean;
	usable: boolean;
	texture: string | null;
};

/// A cell of a recipe: an item id, `#Tag` for any item of a family, or null for an empty cell.
export type Recipe = {
	output: string;
	count: number;
	width: number;
	height: number;
	cells: (string | null)[];
	table: boolean;
};

export type Trade = {
	cost: { item: string; count: number };
	goods: { item: string; count: number } | { directions: true };
	stock: number;
};

export type Mob = {
	id: string;
	name: Text;
	health: number;
	hostile: boolean;
	aquatic: boolean;
	boss: boolean;
	villager: boolean;
	vehicle: boolean;
	animal: boolean;
	tameable: boolean;
	mount: boolean;
	leashable: boolean;
	carriesChest: boolean;
	weakToAxes: boolean;
	size: [number, number, number];
	eats: string[];
	egg: string | null;
	trades: Trade[];
	/// What the Codex says of it (`codex.mob.*` of `data/lang`).
	codex: { summary: Line | null; spawns: Line | null; behaviour: Line | null; speed: Line | null };
	loot: { item: string; count: { min: number; max: number } | { chance: number }; when: 'Always' | 'NotSheared' | 'Saddled' | 'Otherwise' }[];
	damage: number | null;
	effect: { effect: string; level: number; seconds: number } | null;
	xp: number;
	sight: { sight: number; forget: number; speed: number } | null;
	walk: number;
};

type Span = { min: number; max: number };
type Chance = { edge: number; heart: number };

export type Biome = {
	key: string;
	name: string;
	climate: Record<string, Span>[];
	tags: string[];
	altitude_variants: [number, string][];
	sub_biomes: [string, number][];
	border: { biome: string; width: number } | null;
	ground: { surface: string; sub_surface: string; frozen: boolean };
	flora: {
		flowers: number;
		tall_grass: Chance;
		ferns: Chance;
		cacti: number;
		ice_spikes: number;
		trees: [string, Chance][];
		giant_trees: boolean;
	};
	herds: { kind: string; chance: number; fewest: number; most: number }[];
	look: { colormap: [number, number]; precipitation: string; map_colour: [number, number, number] };
	village: string | null;
};

export type Game = {
	ticksPerSecond: number;
	cookSeconds: number;
	distillSeconds: number;
	alchemyLonger: string;
	alchemyStronger: string;
	items: Item[];
	blocks: Block[];
	recipes: Recipe[];
	tags: Record<string, string[]>;
	alchemy: { vial: string; ingredient: string; result: string }[];
	mobs: Mob[];
	biomes: Biome[];
	biomeNotes: Record<string, Line | null>;
	enchantments: { id: string; name: string; levels: string[]; items: string[] }[];
	effects: { id: string; name: Text; beneficial: boolean; instant: boolean; does: Text }[];
	achievements: { id: string; title: Text; goal: Text }[];
	/// What a fishing rod brings back (`shared/src/world/fishing.rs`): first the kind of catch,
	/// by its share, then an entry of its pool by weight.
	fishing: { name: string; share: number; entries: { item: string; weight: number; count: number; night: boolean }[] }[];
	guides: { slug: string; en: TextBlock[]; fr: TextBlock[] | null }[];
};

export const game = raw as unknown as Game;

const byId = <T extends { id: string }>(list: T[]) => new Map(list.map((x) => [x.id, x]));
export const items = byId(game.items);
export const blocks = byId(game.blocks);
export const mobs = byId(game.mobs);

/// The URL part of a name: `oak_log` and `OakLog` become `oak-log`.
export const slugOf = (id: string) =>
	id
		.replace(/([a-z0-9])([A-Z])/g, '$1-$2')
		.replace(/_/g, '-')
		.toLowerCase();

/// `IcePlain` reads `Ice Plain`.
export const spaced = (name: string) => name.replace(/([a-z])([A-Z])/g, '$1 $2');

export const itemName = (id: string) => items.get(id)?.name.en ?? spaced(id);
export const mobName = (id: string) => mobs.get(id)?.name.en ?? spaced(id);

export const biomeSlug = (key: string) => key.replace(/^rustcraft:/, '').replace(/_/g, '-');
export const biomeName = (biome: Biome) => spaced(biome.name);
export const biomeByKey = new Map(game.biomes.map((b) => [b.key, b]));

/// The creatures the wiki has a page for: not the boat, which is an item.
export const creatures = game.mobs.filter((m) => !m.vehicle);

// The textures the game tints by biome, drawn in the colours of the plains.
const GRASS = '#7fb35a';
const FOLIAGE = '#5f9e35';
const TINTS: Record<string, string> = {
	OakLeaves: FOLIAGE,
	JungleLeaves: FOLIAGE,
	WillowLeaves: FOLIAGE,
	AcaciaLeaves: FOLIAGE,
	SpruceLeaves: '#61875a',
	TallGrass: GRASS,
	Fern: GRASS,
	Vine: FOLIAGE,
	Grass: GRASS
};

export function itemRef(item: Item): Ref {
	return {
		kind: 'item',
		id: item.id,
		href: `/items/${slugOf(item.id)}`,
		name: item.name.en ?? spaced(item.id),
		icon: item.icon,
		top: item.iconTop,
		tint: TINTS[item.id] ?? null,
		cube: item.block !== null && item.icon?.startsWith('blocks/') === true && isCube(item.block),
		potion: item.potionColor ? `rgb(${item.potionColor.join(' ')})` : null
	};
}

export function mobRef(mob: Mob): Ref {
	const egg = mob.egg ? items.get(mob.egg) : undefined;
	return {
		kind: 'mob',
		id: mob.id,
		href: `/creatures/${slugOf(mob.id)}`,
		name: mob.name.en ?? spaced(mob.id),
		icon: egg?.icon ?? null,
		top: null,
		tint: null,
		cube: false,
		potion: null,
		picture: `/creatures/${mob.id}.png`
	};
}

/// Whether a block is drawn as a whole cube: the plants, the panes, the furniture and the like
/// are flat icons.
function isCube(block: string): boolean {
	const b = blocks.get(block);
	if (!b?.texture) return false;
	return !FLAT.test(block);
}
const FLAT =
	/(Sapling|Flower|Dandelion|Poppy|Tulip|TallGrass|Fern|Vine|Torch|Door|Trapdoor|Pane|Rail|Ladder|Mushroom|Sprout|Bush|Reef|Kelp|Seagrass|Rye|Wheat|Carrot|Potato|Heather|Cocoa|Fire|Banner|Lantern|Lily|Cactus$|Sugar|Shrub|Chair|Table$|Bedroll|Carpet|CaveVine|Cluster|Pod)/;

/// Every ref, for the links of every page: small enough to send once.
export const refs: Record<string, Ref> = Object.fromEntries([
	...game.items.map((i) => [i.id, itemRef(i)] as const),
	...creatures.map((m) => [`mob:${m.id}`, mobRef(m)] as const)
]);

/// The items a recipe cell takes.
export const members = (cell: string) =>
	cell.startsWith('#') ? (game.tags[cell.slice(1)] ?? []) : [cell];

export const recipesFor = (id: string) => game.recipes.filter((r) => r.output === id);
export const usesOf = (id: string) =>
	game.recipes.filter((r) => r.cells.some((c) => c !== null && members(c).includes(id)));

export const smeltingInputs = (id: string) => game.items.filter((i) => i.smeltsInto === id);

/// The blocks whose breaking may give `id`, and the odds of it.
export function blockSources(id: string) {
	return game.blocks.flatMap((b) => {
		const total = b.drops.reduce((sum, d) => sum + d.chance, 0);
		const drop = b.drops.find((d) => d.item === id);
		// A block that drops itself is no news on its page
		if (!drop || b.id === id) return [];
		return [{ block: b.id, item: itemFor(b.id), chance: drop.chance / total, count: drop.count }];
	});
}

/// The item a block is placed from, if one is.
export const itemFor = (block: string) => game.items.find((i) => i.block === block)?.id ?? null;

/// A line of the loot of a creature, as the pages show it.
export type LootEntry = { item: string; min: number; max: number; chance?: number; note?: string };

/// What a creature drops (`MobKind::loot`), and what it may wear.
export function lootOf(id: string): LootEntry[] {
	const mob = mobs.get(id);
	if (!mob) return [];
	const lines: LootEntry[] = mob.loot.map((l, i) => {
		const entry: LootEntry =
			'min' in l.count ? { item: l.item, min: l.count.min, max: l.count.max } : { item: l.item, min: 1, max: 1, chance: l.count.chance };
		if (l.when === 'NotSheared') entry.note = 'unless sheared';
		if (l.when === 'Saddled') entry.note = 'if saddled';
		// The line before failed: its chance's complement
		const before = mob.loot[i - 1]?.count;
		if (l.when === 'Otherwise' && before && 'chance' in before) entry.chance = 1 - before.chance;
		return entry;
	});
	if (mob.carriesChest) lines.push({ item: 'Chest', min: 1, max: 1, note: 'if it carries one' });
	return lines;
}

export function mobSources(id: string) {
	return creatures.flatMap((m) => lootOf(m.id).filter((l) => l.item === id).map((l) => ({ mob: m.id, ...l })));
}

const ROMAN = ['', 'I', 'II', 'III', 'IV', 'V'];

/// What the creature pages say of a creature, its texts in markup.
export function factsOf(mob: Mob) {
	const effect = mob.effect;
	const effectName = effect ? (game.effects.find((e) => e.id === effect.effect)?.name.en ?? effect.effect) : null;
	return {
		summary: plain(mob.codex.summary),
		spawns: mob.codex.spawns,
		behaviour: mob.codex.behaviour,
		speed: plain(mob.codex.speed),
		damage: mob.damage,
		effect: effect ? `${effectName} ${ROMAN[effect.level] ?? effect.level} for ${effect.seconds} s` : null,
		xp: mob.xp,
		sight: mob.sight ? (mob.sight.sight === mob.sight.forget ? `${mob.sight.sight}` : `${mob.sight.sight} / ${mob.sight.forget}`) : null
	};
}

/// A biome's few words.
export const biomeNote = (key: string) => plain(game.biomeNotes[key.replace(/^rustcraft:/, '')]);

export function tradesWith(id: string) {
	return creatures.flatMap((m) =>
		m.trades
			.filter((t) => t.cost.item === id || ('item' in t.goods && t.goods.item === id))
			.map((t) => ({ mob: m.id, trade: t }))
	);
}

/// `data` with the refs of every item and creature it names, for the links and icons its page
/// draws: a page sends those it shows, not all of them.
export function withRefs<T extends object>(data: T, extra: string[] = []): T & { refs: Record<string, Ref> } {
	const picked: Record<string, Ref> = {};
	const add = (s: string) => {
		if (s.startsWith('#')) {
			for (const m of game.tags[s.slice(1)] ?? []) add(m);
			picked[s] = tagRef(s.slice(1));
			return;
		}
		if (refs[s]) picked[s] = refs[s];
		if (refs[`mob:${s}`]) picked[`mob:${s}`] = refs[`mob:${s}`];
	};
	const walk = (v: unknown): void => {
		if (typeof v === 'string') add(v);
		else if (Array.isArray(v)) v.forEach(walk);
		else if (v && typeof v === 'object') Object.values(v).forEach(walk);
	};
	walk(data);
	extra.forEach(add);
	return { ...data, refs: picked };
}

/// A family of items, drawn with its first member's icon.
function tagRef(tag: string): Ref {
	const first = refs[game.tags[tag]?.[0] ?? ''];
	return {
		...(first ?? itemRef(game.items[0])),
		id: `#${tag}`,
		href: `/items?tag=${tag}`,
		name: `Any ${TAG_NAMES[tag] ?? tag}`
	};
}
const TAG_NAMES: Record<string, string> = {
	Planks: 'planks',
	Cobble: 'cobblestone',
	Wool: 'wool',
	RawFish: 'raw fish',
	RoastMeat: 'roast meat'
};
