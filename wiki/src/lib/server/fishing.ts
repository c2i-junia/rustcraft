// What a fishing rod brings back, as `shared/src/world/fishing.rs` rolls it: first the kind of
// catch, then an entry of its pool by weight. Its tables are private to the game, so they are
// written again here: keep the two together.

export type FishingPool = {
	name: string;
	/// The share of the catches of this kind.
	share: number;
	entries: { item: string; weight: number; count: number; night?: boolean }[];
};

export const FISHING: FishingPool[] = [
	{
		name: 'Fish',
		share: 0.84,
		entries: [
			{ item: 'RawRoach', weight: 40, count: 1 },
			{ item: 'RawPerch', weight: 28, count: 1 },
			{ item: 'RawPike', weight: 15, count: 1 },
			{ item: 'RawEel', weight: 12, count: 1 },
			// By day it hides: the other four share its weight
			{ item: 'RawGlowfin', weight: 5, count: 1, night: true }
		]
	},
	{
		name: 'Junk',
		share: 0.11,
		entries: [
			{ item: 'Stick', weight: 1, count: 2 },
			{ item: 'Leather', weight: 1, count: 1 },
			{ item: 'Rope', weight: 1, count: 1 },
			{ item: 'Bowl', weight: 1, count: 1 },
			{ item: 'Fiber', weight: 1, count: 3 }
		]
	},
	{
		name: 'Treasure',
		share: 0.05,
		entries: [
			{ item: 'Coin', weight: 1, count: 4 },
			{ item: 'GoldIngot', weight: 1, count: 1 },
			{ item: 'Saddle', weight: 1, count: 1 },
			{ item: 'Bow', weight: 1, count: 1 },
			{ item: 'Diamond', weight: 1, count: 1 }
		]
	}
];
