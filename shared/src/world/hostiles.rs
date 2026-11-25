//! The rules of the hostile mobs that the server applies: when it is night, how hard the world
//! is, where a hostile may appear and of which kind, whom it hunts and how it strikes.

use bevy::math::{IVec3, Vec3};
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::players::Player;
use crate::world::{BlockHitbox, BlockId, BlockTransparency, ChunkMap, MobKind};
use crate::DAY_DURATION_IN_TICKS;

/// Whether `tick` falls in the night: the first half of the cycle, which starts at sunset.
pub fn is_night(tick: u64) -> bool {
    tick % DAY_DURATION_IN_TICKS < DAY_DURATION_IN_TICKS / 2
}

/// How hard a world is, saved with it. Append only: it is saved by index.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Difficulty {
    /// No hostile mob.
    Peaceful,
    /// Hostile mobs appear in the dark.
    #[default]
    Normal,
}

impl Difficulty {
    pub const ALL: [Difficulty; 2] = [Difficulty::Peaceful, Difficulty::Normal];

    /// The name the `/difficulty` command takes.
    pub fn name(self) -> &'static str {
        match self {
            Difficulty::Peaceful => "peaceful",
            Difficulty::Normal => "normal",
        }
    }

    pub fn from_name(name: &str) -> Option<Difficulty> {
        Self::ALL
            .into_iter()
            .find(|difficulty| difficulty.name().eq_ignore_ascii_case(name))
    }

    /// Whether hostile mobs live in a world this hard.
    pub fn has_hostiles(self) -> bool {
        self != Difficulty::Peaceful
    }
}

/// How a hostile hunts: how far it sees a prey from, how far it follows one before it forgets
/// it, and how fast it goes at it, in blocks and blocks per second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hunt {
    pub sight: f32,
    pub forget: f32,
    pub speed: f32,
}

/// How a hostile strikes: the points of health it takes, how near the player's box its centre
/// must be, the ticks before it strikes again, and the knock back of the blow in blocks per
/// second (none at 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    pub damage: u32,
    pub reach: f32,
    pub cooldown: u32,
    pub knock_back: f32,
}

impl MobKind {
    /// How a hostile of this kind hunts; `None` for an animal.
    pub fn hunt(self) -> Option<Hunt> {
        match self {
            MobKind::Gnawer => Some(Hunt {
                sight: 16.0,
                forget: 24.0,
                speed: 2.8,
            }),
            // It never moves: it bites who comes within reach
            MobKind::Bramble => Some(Hunt {
                sight: 2.5,
                forget: 2.5,
                speed: 0.0,
            }),
            MobKind::Roller => Some(Hunt {
                sight: 12.0,
                forget: 24.0,
                speed: 7.0,
            }),
            MobKind::Stumpling => Some(Hunt {
                sight: 16.0,
                forget: 24.0,
                speed: 1.2,
            }),
            // It hovers and keeps its distance (`hostile::hover_and_throw`)
            MobKind::Cinderling => Some(Hunt {
                sight: 16.0,
                forget: 24.0,
                speed: 2.0,
            }),
            // Its speed as it circles 8 blocks over its prey (`hostile::circle_and_dive`)
            MobKind::GustOwl => Some(Hunt {
                sight: 24.0,
                forget: 32.0,
                speed: 4.0,
            }),
            // Its speed in water: it walks at 0.8 on land, only back to the water
            MobKind::Drowner => Some(Hunt {
                sight: 16.0,
                forget: 24.0,
                speed: 3.0,
            }),
            _ => None,
        }
    }

    /// How a hostile of this kind strikes; `None` for an animal.
    pub fn strike(self) -> Option<Strike> {
        match self {
            MobKind::Gnawer => Some(Strike {
                damage: 2,
                reach: 1.0,
                cooldown: 15,
                knock_back: 0.0,
            }),
            MobKind::Bramble => Some(Strike {
                damage: 3,
                reach: 2.5,
                cooldown: 20,
                knock_back: 0.0,
            }),
            // Its roll hits what its box touches
            MobKind::Roller => Some(Strike {
                damage: 4,
                reach: 0.0,
                cooldown: 0,
                knock_back: 8.0,
            }),
            MobKind::Stumpling => Some(Strike {
                damage: 3,
                reach: 1.2,
                cooldown: 20,
                knock_back: 0.0,
            }),
            // Its ember, thrown as far as it reaches, sets the player burning too
            MobKind::Cinderling => Some(Strike {
                damage: 1,
                reach: 16.0,
                cooldown: 60,
                knock_back: 0.0,
            }),
            // Its dive: the cooldown is the circling between two dives
            MobKind::GustOwl => Some(Strike {
                damage: 2,
                reach: 0.5,
                cooldown: 60,
                knock_back: 8.0,
            }),
            // Its grab also drags the player down (`Player::drag`)
            MobKind::Drowner => Some(Strike {
                damage: 2,
                reach: 1.5,
                cooldown: 20,
                knock_back: 0.0,
            }),
            _ => None,
        }
    }
}

/// Whether a hostile hunts `player`: a living player in survival. Creative and dead players
/// are left alone.
pub fn is_prey(player: &Player) -> bool {
    !player.is_dead() && player.game_mode.takes_damage()
}

/// How far `point` is from the box of centre `centre` and half size `half`: 0 inside it.
pub fn distance_to_box(point: Vec3, centre: Vec3, half: Vec3) -> f32 {
    let nearest = point.clamp(centre - half, centre + half);
    point.distance(nearest)
}

/// The length of the steps along which `line_of_sight` looks at the cells.
const SIGHT_STEP: f32 = 0.25;

/// Whether nothing stands between `from` and `to`: no block with a hitbox in the cells the
/// segment crosses, looked at every `SIGHT_STEP` blocks, the ends left out. Unloaded cells hide
/// nothing. Bounded by the length of the segment, which the hunts keep short.
pub fn line_of_sight(world: &ChunkMap, from: Vec3, to: Vec3) -> bool {
    let length = from.distance(to);
    let steps = (length / SIGHT_STEP).ceil() as u32;
    let (start, end) = (from.floor().as_ivec3(), to.floor().as_ivec3());
    (1..steps).all(|step| {
        let cell = from.lerp(to, step as f32 / steps as f32).floor().as_ivec3();
        cell == start
            || cell == end
            || !world
                .get_block(cell)
                .is_some_and(|block| !matches!(block.get_hitbox(), BlockHitbox::None))
    })
}

/// How far around a spot, along each axis, a light emitter keeps the hostiles away, and leaves
/// tell a wood, in blocks.
pub const LIGHT_RADIUS: i32 = 8;

/// What lies around a spot, within `LIGHT_RADIUS` along each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Surroundings {
    /// A block that gives light (`BlockId::light_emission`): a torch, a lit furnace, a fire,
    /// lava, a lantern, a campfire.
    pub light: bool,
    /// Leaves of a tree.
    pub leaves: bool,
}

/// What lies in the cube of `LIGHT_RADIUS` around `pos`: 17³ cells at most, the scan stopping
/// at the first light, which is all a spawn needs to know then.
pub fn surroundings(world: &ChunkMap, pos: IVec3) -> Surroundings {
    let mut found = Surroundings::default();
    let r = LIGHT_RADIUS;
    for x in -r..=r {
        for y in -r..=r {
            for z in -r..=r {
                let Some(block) = world.get_block(pos + IVec3::new(x, y, z)) else {
                    continue;
                };
                if block.id.light_emission() > 0 {
                    found.light = true;
                    return found;
                }
                found.leaves |= block.id.sapling().is_some();
            }
        }
    }
    found
}

/// Whether a spot is dark enough for a hostile: at night under the open sky, or under a block
/// at any time, and no light near.
pub fn is_dark(night: bool, sees_sky: bool, light_near: bool) -> bool {
    !light_near && (night || !sees_sky)
}

/// What a spawn attempt found where a hostile could stand.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpawnPlace {
    /// The height of the cell of its feet.
    pub y: i32,
    pub night: bool,
    /// Whether a block stands above it (`!ticks::sees_sky`).
    pub covered: bool,
    /// The block it would stand on.
    pub floor: BlockId,
    /// Whether leaves are near (`Surroundings::leaves`): a forest or a jungle.
    pub wooded: bool,
    /// How deep the water is, for a place at the top of a column of water; 0 on land.
    pub water_depth: u32,
}

/// How deep water must be for a Drowner, in blocks.
pub const DROWNER_DEPTH: u32 = 3;
/// The odds of a Drowner in deep enough water by day; at night they are doubled.
pub const DROWNER_ODDS: (u32, u32) = (1, 4);

/// Above this height, at night under the sky, the peaks: the Gust Owls' home.
pub const OWL_HEIGHT: i32 = 110;
/// Above this height, the mountains: the Rollers' home.
pub const MOUNTAIN_HEIGHT: i32 = 100;
/// Below this height and under a block, the deep caves: the Cinderlings' home.
pub const CINDER_HEIGHT: i32 = 20;
/// Below this height and under a block, the caves.
pub const CAVE_HEIGHT: i32 = 50;

/// The kind of hostile a dark `place` gets, if any: on the peaks at night under the sky Gust
/// Owls (1 in 2); on grass at night Stumplings, or in the
/// woods Brambles and Stumplings half each; Rollers in the mountains, Gnawers (3 in 4) or
/// Rollers in the caves, and in the deep caves Cinderlings (1 in 3); Drowners in water `DROWNER_DEPTH` deep, at `DROWNER_ODDS` by day and
/// twice that at night. Each later kind adds its place here.
pub fn spawn_kind(place: &SpawnPlace, rng: &mut impl Rng) -> Option<MobKind> {
    if place.water_depth > 0 {
        let (chances, draws) = DROWNER_ODDS;
        let chances = if place.night { chances * 2 } else { chances };
        let drowner = place.water_depth >= DROWNER_DEPTH && rng.gen_ratio(chances, draws);
        drowner.then_some(MobKind::Drowner)
    } else if place.night && !place.covered && place.y > OWL_HEIGHT && rng.gen_bool(0.5) {
        Some(MobKind::GustOwl)
    } else if place.night && place.floor == BlockId::Grass {
        Some(if place.wooded && rng.gen_bool(0.5) {
            MobKind::Bramble
        } else {
            MobKind::Stumpling
        })
    } else if place.y > MOUNTAIN_HEIGHT {
        Some(MobKind::Roller)
    } else if place.covered && place.y < CINDER_HEIGHT && rng.gen_ratio(1, 3) {
        Some(MobKind::Cinderling)
    } else if place.covered && place.y < CAVE_HEIGHT {
        Some(if rng.gen_ratio(3, 4) {
            MobKind::Gnawer
        } else {
            MobKind::Roller
        })
    } else {
        None
    }
}

/// How many hostiles of `kind` appear together: 2 to 4 Gnawers, one of the others.
pub fn pack_size(kind: MobKind, rng: &mut impl Rng) -> u32 {
    match kind {
        MobKind::Gnawer => rng.gen_range(2..=4),
        _ => 1,
    }
}

/// Whether a mob may stand with its feet in the cell `feet`: a block with a hitbox under it,
/// not a liquid, and the two cells from its feet up free of hitboxes and liquids. The chunks
/// must be loaded: unknown cells are no floor.
pub fn may_spawn_at(world: &ChunkMap, feet: IVec3) -> Option<BlockId> {
    let floor = world.get_block(feet - IVec3::Y)?;
    if matches!(floor.get_hitbox(), BlockHitbox::None)
        || floor.id.get_visibility() == BlockTransparency::Liquid
    {
        return None;
    }
    let free = |pos: IVec3| {
        world.get_block(pos).is_none_or(|block| {
            matches!(block.get_hitbox(), BlockHitbox::None)
                && block.id.get_visibility() != BlockTransparency::Liquid
        })
    };
    (free(feet) && free(feet + IVec3::Y)).then_some(floor.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::game_mode::GameMode;
    use crate::world::{BlockData, BlockDirection};
    use crate::MIDNIGHT_IN_DAY;
    use crate::NOON_IN_DAY;
    use bevy::prelude::Transform;
    use rand::{rngs::StdRng, SeedableRng};

    #[test]
    fn the_night_is_the_first_half_of_each_cycle() {
        assert!(is_night(0));
        assert!(is_night(MIDNIGHT_IN_DAY));
        assert!(is_night(DAY_DURATION_IN_TICKS / 2 - 1));
        assert!(!is_night(DAY_DURATION_IN_TICKS / 2));
        assert!(!is_night(NOON_IN_DAY));
        assert!(!is_night(DAY_DURATION_IN_TICKS - 1));
        assert!(is_night(5 * DAY_DURATION_IN_TICKS + MIDNIGHT_IN_DAY));
        assert!(!is_night(5 * DAY_DURATION_IN_TICKS + NOON_IN_DAY));
    }

    #[test]
    fn the_difficulty_is_normal_by_default_and_read_by_its_name() {
        assert_eq!(Difficulty::default(), Difficulty::Normal);
        assert_eq!(
            Difficulty::from_name("peaceful"),
            Some(Difficulty::Peaceful)
        );
        assert_eq!(Difficulty::from_name("Normal"), Some(Difficulty::Normal));
        assert_eq!(Difficulty::from_name("hard"), None);
        assert!(!Difficulty::Peaceful.has_hostiles());
        assert!(Difficulty::Normal.has_hostiles());
    }

    #[test]
    fn hostiles_hunt_and_strike_and_animals_do_not() {
        for kind in MobKind::ALL {
            assert_eq!(kind.is_hostile(), kind.hunt().is_some(), "{kind:?}");
            assert_eq!(kind.is_hostile(), kind.strike().is_some(), "{kind:?}");
        }
        let gnawer = MobKind::Gnawer.strike().unwrap();
        assert_eq!((gnawer.damage, gnawer.cooldown), (2, 15));
        assert_eq!(MobKind::Gnawer.hunt().unwrap().speed, 2.8);
        let bramble = MobKind::Bramble.strike().unwrap();
        assert_eq!(
            (bramble.damage, bramble.cooldown, bramble.reach),
            (3, 20, 2.5)
        );
        assert_eq!(MobKind::Bramble.hunt().unwrap().speed, 0.0);
        let roller = MobKind::Roller.strike().unwrap();
        assert_eq!((roller.damage, roller.knock_back), (4, 8.0));
        assert_eq!(MobKind::Roller.hunt().unwrap().sight, 12.0);
        let drowner = MobKind::Drowner.strike().unwrap();
        assert_eq!((drowner.damage, drowner.knock_back), (2, 0.0));
        assert_eq!(MobKind::Drowner.hunt().unwrap().speed, 3.0);
        assert_eq!(MobKind::Drowner.health(), 18);
        let stumpling = MobKind::Stumpling.strike().unwrap();
        assert_eq!((stumpling.damage, stumpling.cooldown), (3, 20));
        let owl = MobKind::GustOwl.strike().unwrap();
        assert_eq!((owl.damage, owl.knock_back), (2, 8.0));
        assert_eq!(MobKind::GustOwl.health(), 14);
        // The Cinderling's strike is its ember, thrown every 60 ticks
        let cinderling = MobKind::Cinderling.strike().unwrap();
        assert_eq!((cinderling.damage, cinderling.cooldown), (1, 60));
        assert_eq!(MobKind::Cinderling.health(), 10);
        assert_eq!(MobKind::Stumpling.hunt().unwrap().speed, 1.2);
        assert_eq!(
            [
                MobKind::Gnawer,
                MobKind::Bramble,
                MobKind::Roller,
                MobKind::Stumpling
            ]
            .map(MobKind::health),
            [6, 12, 16, 24]
        );
    }

    #[test]
    fn only_living_players_in_survival_are_prey() {
        let mut player = Player::new(1, "p".into(), Vec3::ZERO, Transform::default());
        assert!(is_prey(&player));
        player.game_mode = GameMode::Creative;
        assert!(!is_prey(&player));
        player.game_mode = GameMode::Survival;
        player.health = 0;
        assert!(!is_prey(&player));
    }

    #[test]
    fn the_distance_to_a_box_is_zero_inside_and_to_its_nearest_face_outside() {
        let half = Vec3::new(0.5, 1.0, 0.5);
        assert_eq!(distance_to_box(Vec3::ZERO, Vec3::ZERO, half), 0.0);
        assert_eq!(
            distance_to_box(Vec3::new(2.0, 0.5, 0.0), Vec3::ZERO, half),
            1.5
        );
    }

    fn stone_wall_world() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::new(5, 0, -3), IVec3::new(5, 3, 3), BlockId::Stone);
        world
    }

    #[test]
    fn a_block_between_two_points_hides_one_from_the_other() {
        let world = stone_wall_world();
        let eye = Vec3::new(0.5, 1.5, 0.5);
        assert!(!line_of_sight(&world, eye, Vec3::new(10.5, 1.5, 0.5)));
        // Over the wall, beside it, and short of it
        assert!(line_of_sight(&world, eye, Vec3::new(10.5, 9.5, 0.5)));
        assert!(line_of_sight(&world, eye, Vec3::new(10.5, 1.5, 8.5)));
        assert!(line_of_sight(&world, eye, Vec3::new(4.5, 1.5, 0.5)));
        // Tall grass hides nothing
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::new(5, 1, 0), IVec3::new(5, 1, 0), BlockId::TallGrass);
        assert!(line_of_sight(&world, eye, Vec3::new(10.5, 1.5, 0.5)));
    }

    #[test]
    fn a_light_within_eight_blocks_is_found_and_leaves_tell_a_wood() {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        assert_eq!(surroundings(&world, IVec3::ZERO), Surroundings::default());
        for light in [
            BlockId::Torch,
            BlockId::LitFurnace,
            BlockId::Fire,
            BlockId::Lava,
            BlockId::Lantern,
            BlockId::Campfire,
        ] {
            world.set_block(
                IVec3::new(8, -8, 8),
                BlockData::new(light, BlockDirection::Front),
            );
            assert!(surroundings(&world, IVec3::ZERO).light, "{light:?}");
        }
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::new(9, 0, 0), IVec3::new(9, 0, 0), BlockId::Torch);
        assert!(!surroundings(&world, IVec3::ZERO).light, "9 blocks away");
        world.fill_box(IVec3::new(0, 6, 0), IVec3::new(0, 6, 0), BlockId::OakLeaves);
        assert_eq!(
            surroundings(&world, IVec3::ZERO),
            Surroundings {
                light: false,
                leaves: true
            }
        );
    }

    #[test]
    fn a_spot_is_dark_at_night_under_the_sky_or_under_a_block_and_never_by_a_light() {
        assert!(is_dark(true, true, false));
        assert!(!is_dark(false, true, false));
        assert!(is_dark(false, false, false));
        assert!(is_dark(true, false, false));
        assert!(!is_dark(true, true, true));
        assert!(!is_dark(false, false, true));
    }

    fn place(y: i32, night: bool, covered: bool, floor: BlockId, wooded: bool) -> SpawnPlace {
        SpawnPlace {
            y,
            night,
            covered,
            floor,
            wooded,
            water_depth: 0,
        }
    }

    /// The top of water `depth` deep.
    fn water(depth: u32, night: bool) -> SpawnPlace {
        SpawnPlace {
            water_depth: depth,
            ..place(62, night, false, BlockId::Water, false)
        }
    }

    /// How many of 1000 draws at `place` give a Drowner.
    fn drowners(place: SpawnPlace) -> usize {
        let mut rng = StdRng::seed_from_u64(3);
        (0..1000)
            .filter(|_| spawn_kind(&place, &mut rng) == Some(MobKind::Drowner))
            .count()
    }

    #[test]
    fn water_three_deep_gets_drowners_at_any_time_and_twice_as_many_at_night() {
        assert_eq!(kinds(water(3, false)), [None, Some(MobKind::Drowner)]);
        assert_eq!(kinds(water(2, true)), [None]);
        let (day, night) = (drowners(water(5, false)), drowners(water(5, true)));
        assert!((200..300).contains(&day), "{day} by day");
        assert!((450..550).contains(&night), "{night} at night");
    }

    /// The kinds `place` gets over many draws.
    fn kinds(place: SpawnPlace) -> Vec<Option<MobKind>> {
        let mut rng = StdRng::seed_from_u64(1);
        let mut kinds: Vec<_> = (0..200).map(|_| spawn_kind(&place, &mut rng)).collect();
        kinds.sort_by_key(|kind| format!("{kind:?}"));
        kinds.dedup();
        kinds
    }

    #[test]
    fn the_place_picks_the_kind() {
        use MobKind::*;
        // Caves: Gnawers and Rollers
        assert_eq!(
            kinds(place(30, false, true, BlockId::Stone, false)),
            [Some(Gnawer), Some(Roller)]
        );
        // The woods at night, on grass, even under the leaves: Brambles and Stumplings
        assert_eq!(
            kinds(place(70, true, false, BlockId::Grass, true)),
            [Some(Bramble), Some(Stumpling)]
        );
        assert_eq!(
            kinds(place(70, true, true, BlockId::Grass, true)),
            [Some(Bramble), Some(Stumpling)]
        );
        // Mountains: Gust Owls too above 110 at night under the sky
        assert_eq!(
            kinds(place(120, true, false, BlockId::Stone, false)),
            [Some(GustOwl), Some(Roller)]
        );
        assert_eq!(
            kinds(place(OWL_HEIGHT, true, false, BlockId::Stone, false)),
            [Some(Roller)]
        );
        assert_eq!(
            kinds(place(120, true, true, BlockId::Stone, false)),
            [Some(Roller)]
        );
        assert_eq!(
            kinds(place(120, false, true, BlockId::Stone, false)),
            [Some(Roller)]
        );
        // Open plains at night: Stumplings
        assert_eq!(
            kinds(place(70, true, false, BlockId::Grass, false)),
            [Some(Stumpling)]
        );
        // Deep caves: Cinderlings too
        assert_eq!(
            kinds(place(10, false, true, BlockId::Stone, false)),
            [Some(Cinderling), Some(Gnawer), Some(Roller)]
        );
        assert_eq!(
            kinds(place(CINDER_HEIGHT, false, true, BlockId::Stone, false)),
            [Some(Gnawer), Some(Roller)]
        );
        // Under the trees at day, open stone at night, a cave too high: none
        assert_eq!(kinds(place(70, false, true, BlockId::Grass, true)), [None]);
        assert_eq!(kinds(place(70, true, false, BlockId::Stone, false)), [None]);
        assert_eq!(kinds(place(60, false, true, BlockId::Stone, false)), [None]);
    }

    #[test]
    fn gnawers_come_in_packs_of_two_to_four_and_the_others_alone() {
        let mut rng = StdRng::seed_from_u64(2);
        let mut sizes: Vec<u32> = (0..100)
            .map(|_| pack_size(MobKind::Gnawer, &mut rng))
            .collect();
        sizes.sort();
        sizes.dedup();
        assert_eq!(sizes, [2, 3, 4]);
        assert_eq!(pack_size(MobKind::Bramble, &mut rng), 1);
        assert_eq!(pack_size(MobKind::Roller, &mut rng), 1);
        assert_eq!(pack_size(MobKind::Stumpling, &mut rng), 1);
    }

    #[test]
    fn a_mob_spawns_on_a_floor_with_two_free_cells_above_and_not_in_water() {
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(31));
        world.fill_box(IVec3::new(-4, 0, -4), IVec3::new(4, 0, 4), BlockId::Stone);
        assert_eq!(
            may_spawn_at(&world, IVec3::new(0, 1, 0)),
            Some(BlockId::Stone)
        );
        // In the air, in the floor
        assert_eq!(may_spawn_at(&world, IVec3::new(0, 2, 0)), None);
        assert_eq!(may_spawn_at(&world, IVec3::new(0, 0, 0)), None);
        // Under a ceiling one block high
        world.fill_box(IVec3::new(1, 2, 0), IVec3::new(1, 2, 0), BlockId::Stone);
        assert_eq!(may_spawn_at(&world, IVec3::new(1, 1, 0)), None);
        // In water
        world.fill_box(IVec3::new(2, 1, 0), IVec3::new(2, 1, 0), BlockId::Water);
        assert_eq!(may_spawn_at(&world, IVec3::new(2, 1, 0)), None);
        // On water
        world.fill_box(IVec3::new(3, 1, 0), IVec3::new(3, 1, 0), BlockId::Water);
        assert_eq!(may_spawn_at(&world, IVec3::new(3, 2, 0)), None);
        // Over unloaded chunks
        assert_eq!(may_spawn_at(&world, IVec3::new(0, 100, 0)), None);
    }
}
