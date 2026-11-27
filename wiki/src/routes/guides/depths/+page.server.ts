import { withRefs, mobs } from '#lib/server/game.ts';

// `DepthTier::strength` in `shared/src/world/depths.rs`: health, damage and experience are
// multiplied by it and rounded up (`strengthened`).
const STRENGTHS = [100, 150, 200];
const strengthened = (amount: number, strength: number) => Math.ceil((amount * strength) / 100);

// Base damage (`MobKind::strike`, `hostiles.rs`) and experience (`players/experience.rs`) of
// the hostiles of the caves.
const HOSTILES: { id: string; damage: number; xp: number }[] = [
	{ id: 'Gnawer', damage: 2, xp: 3 },
	{ id: 'Roller', damage: 4, xp: 5 },
	{ id: 'Cinderling', damage: 1, xp: 6 },
	{ id: 'Shardling', damage: 3, xp: 5 },
	{ id: 'Slagcrawler', damage: 2, xp: 6 }
];

export function load() {
	const hostiles = HOSTILES.map((h) => {
		const health = mobs.get(h.id)!.health;
		return {
			id: h.id,
			tiers: STRENGTHS.map((s) => ({
				health: strengthened(health, s),
				damage: strengthened(h.damage, s),
				xp: strengthened(h.xp, s)
			}))
		};
	});
	return withRefs({ hostiles }, [
		'Mossback',
		'Shardling',
		'Slagcrawler',
		'Gnawer',
		'Moss',
		'CaveVine',
		'Clay',
		'Glowcrystal',
		'CrystalCluster',
		'CrystalShard',
		'Geodestone',
		'Basalt',
		'Magma',
		'Gloomrock',
		'Torch',
		'Lantern',
		'IronIngot',
		'GoldIngot',
		'Diamond',
		'FireResistancePotion',
		'WaterBreathingPotion',
		'NightVisionPotion',
		'Kelp',
		'RawGlowfin',
		'WaterBucket',
		'DepthsCompass',
		'IronChestplate',
		'Leather'
	]);
}
