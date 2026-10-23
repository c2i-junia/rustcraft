//! Biomes as data. Each biome is a `BiomeDef`: where it stands in the space of the climate
//! parameters (one or more `ClimateBox`es), what covers its ground, what grows and lives on it,
//! and how the client draws it. A `BiomeRegistry` gathers them, the base game's first
//! (`vanilla`), then any other; frozen into `Biomes`, it picks the biome of a climate point.
//!
//! The server sends the frozen definitions to each client that joins (`Biomes::defs`), which
//! builds the same `Biomes` from them: both find the same biome for every column.

pub mod vanilla;

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::world::{BlockId, MobKind};

/// The start of the keys of the biomes of the base game, which a bare name stands for.
pub const BASE_NAMESPACE: &str = "rustcraft:";

/// A biome of a world: its index in the `Biomes` of that world. Saved data names a biome by its
/// key, never by its id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BiomeId(pub u16);

/// Tags group biomes for whatever treats them alike, so that a new biome gets that treatment
/// by carrying the tag rather than by being named in the code.
pub mod tags {
    pub const OCEAN: &str = "ocean";
    pub const RIVER: &str = "river";
    pub const MOUNTAIN: &str = "mountain";
    pub const FOREST: &str = "forest";
    pub const SNOWY: &str = "snowy";
    pub const HOT: &str = "hot";
    pub const DRY: &str = "dry";
}

/// The climate of a column, every parameter in `[-1, 1]` and spread evenly over it, so that a
/// biome's share of the world is its share of the space of the parameters.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ClimatePoint {
    /// Cold to hot.
    pub temperature: f64,
    /// Dry to wet.
    pub humidity: f64,
    /// Deep sea to far inland; the coast is at `vanilla::COAST`.
    pub continentalness: f64,
    /// Rugged to flat.
    pub erosion: f64,
    /// Picks between biomes of the same climate, and the rare ones at its ends.
    pub weirdness: f64,
    /// 0 at the surface, which is all there is for now: the caves will have their own biomes.
    pub depth: f64,
}

/// How much each parameter weighs in the distance of a point to a box, in the order of
/// `ClimatePoint::axes`: the land and the sea apart first, then the climate, the rest last.
const AXIS_WEIGHTS: [f64; 6] = [1.0, 1.0, 1.5, 0.8, 0.5, 1.0];

impl ClimatePoint {
    pub fn axes(&self) -> [f64; 6] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.weirdness,
            self.depth,
        ]
    }
}

/// A range of one parameter, both ends in `[-1, 1]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub min: f64,
    pub max: f64,
}

impl Span {
    /// The whole range: no constraint.
    pub const ALL: Span = Span::new(-1.0, 1.0);

    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    /// How far `value` lies outside the span, 0 inside it.
    fn gap(&self, value: f64) -> f64 {
        (self.min - value).max(value - self.max).max(0.0)
    }

    /// How far `value`, inside the span, lies from its nearest end that is a border, not an
    /// end of the whole range; `None` when it has none.
    fn margin(&self, value: f64) -> Option<f64> {
        let low = (self.min > -1.0).then_some(value - self.min);
        let high = (self.max < 1.0).then_some(self.max - value);
        match (low, high) {
            (Some(low), Some(high)) => Some(low.min(high)),
            (low, high) => low.or(high),
        }
    }
}

impl Default for Span {
    fn default() -> Self {
        Span::ALL
    }
}

/// Where a biome may stand: a range of each parameter.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ClimateBox {
    pub temperature: Span,
    pub humidity: Span,
    pub continentalness: Span,
    pub erosion: Span,
    pub weirdness: Span,
    pub depth: Span,
}

impl ClimateBox {
    fn spans(&self) -> [Span; 6] {
        [
            self.temperature,
            self.humidity,
            self.continentalness,
            self.erosion,
            self.weirdness,
            self.depth,
        ]
    }

    /// The weighted distance of `point` to the box, 0 inside it.
    pub fn distance(&self, point: &ClimatePoint) -> f64 {
        self.spans()
            .iter()
            .zip(point.axes())
            .zip(AXIS_WEIGHTS)
            .map(|((span, value), weight)| weight * span.gap(value).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    /// How far `point`, inside the box, lies from its nearest border, on the axes `axes` only:
    /// 1 when none of them has a border.
    fn margin(&self, point: &ClimatePoint, axes: &[usize]) -> f64 {
        let (spans, values) = (self.spans(), point.axes());
        axes.iter()
            .filter_map(|&axis| spans[axis].margin(values[axis]))
            .fold(1.0, f64::min)
    }
}

/// A chance that grows from `edge`, at the border of the biome, to `heart` deep inside it (see
/// `ColumnShape::depth_in_biome`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Chance {
    pub edge: f32,
    pub heart: f32,
}

impl Chance {
    pub const fn new(edge: f32, heart: f32) -> Self {
        Self { edge, heart }
    }

    pub const fn flat(chance: f32) -> Self {
        Self::new(chance, chance)
    }

    /// The chance `depth` deep in the biome.
    pub fn at(&self, depth: f32) -> f32 {
        self.edge + (self.heart - self.edge) * depth
    }
}

/// The trees the generation knows how to grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tree {
    Oak,
    BigOak,
    Jungle,
    Bush,
}

/// What grows on a biome. The chances are per column.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Flora {
    /// Dandelions and poppies, on grass.
    pub flowers: f32,
    pub tall_grass: Chance,
    /// Ferns, on grass, in place of the tall grass.
    pub ferns: Chance,
    /// Cacti, on sand.
    pub cacti: f32,
    /// Spikes of ice, on snow.
    pub ice_spikes: f32,
    /// The trees, on grass, each after the previous one on the same roll.
    pub trees: Vec<(Tree, Chance)>,
    /// Whether giant jungle trees grow deep in the biome, before any other tree.
    pub giant_trees: bool,
}

/// The ground of a biome above the water.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Ground {
    pub surface: BlockId,
    pub sub_surface: BlockId,
    /// Its water freezes at the surface, and its shores stay as they are instead of turning to
    /// sand.
    pub frozen: bool,
}

impl Ground {
    pub const fn of(surface: BlockId, sub_surface: BlockId) -> Self {
        Self {
            surface,
            sub_surface,
            frozen: false,
        }
    }
}

impl Default for Ground {
    fn default() -> Self {
        Ground::of(BlockId::Grass, BlockId::Dirt)
    }
}

/// A herd that may live on a column of chunks of the biome: its chance and its size, both ends
/// included.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Herd {
    pub kind: MobKind,
    pub chance: f32,
    pub fewest: u64,
    pub most: u64,
}

/// What falls on a biome from a rainy sky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Precipitation {
    #[default]
    Rain,
    Snow,
    None,
}

/// How the client draws a biome.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BiomeLook {
    /// Where its grass and leaves are read on the colour maps: the temperature and the downfall
    /// Minecraft gives a biome like it, each in `[0, 1]`.
    pub colormap: [f64; 2],
    pub precipitation: Precipitation,
    /// Its colour on maps of the world, such as those of `worldgen_preview.py`.
    pub map_colour: [u8; 3],
}

impl BiomeLook {
    pub const fn new(colormap: [f64; 2], map_colour: [u8; 3]) -> Self {
        Self {
            colormap,
            precipitation: Precipitation::Rain,
            map_colour,
        }
    }
}

impl Default for BiomeLook {
    fn default() -> Self {
        BiomeLook::new([0.5, 0.5], [128, 128, 128])
    }
}

/// A biome put where its own meets another land biome: `width` is how far into the climate of
/// its own it reaches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BorderRule {
    pub biome: String,
    pub width: f64,
}

/// Everything about one biome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BiomeDef {
    /// Stable across versions and mods: `rustcraft:jungle`, `mymod:mangrove`.
    pub key: String,
    /// The name shown to the players.
    pub name: String,
    /// Every box the biome may stand in. A biome with none is only ever reached from another
    /// one: an altitude variant, a sub-biome, a border, the rivers.
    pub climate: Vec<ClimateBox>,
    /// Added to the distance of a point outside every box to this biome's: negative favours it.
    pub offset: f64,
    /// Among the boxes holding a point, only those of the highest priority are kept: a biome
    /// takes a part of another's climate by having a higher one.
    pub priority: i16,
    /// The share of a climate it wins against the other biomes of the same priority whose boxes
    /// hold it too, in patches (1 by default).
    pub weight: f64,
    /// Whether its border with another dithering biome is scattered over a few blocks.
    pub dither: bool,
    pub tags: Vec<String>,
    /// The biome a column becomes from a height on, the highest first.
    pub altitude_variants: Vec<(i32, String)>,
    /// Biomes that take patches deep inside this one: their key and their share of it.
    pub sub_biomes: Vec<(String, f64)>,
    pub border: Option<BorderRule>,
    pub ground: Ground,
    pub flora: Flora,
    pub herds: Vec<Herd>,
    pub look: BiomeLook,
}

impl BiomeDef {
    /// A biome of grass over dirt where nothing grows and nothing lives, that stands nowhere.
    pub fn new(key: &str, name: &str) -> Self {
        Self {
            key: key.to_string(),
            name: name.to_string(),
            climate: Vec::new(),
            offset: 0.0,
            priority: 0,
            weight: 1.0,
            dither: true,
            tags: Vec::new(),
            altitude_variants: Vec::new(),
            sub_biomes: Vec::new(),
            border: None,
            ground: Ground::default(),
            flora: Flora::default(),
            herds: Vec::new(),
            look: BiomeLook::default(),
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }
}

/// The biomes of a world as they are being gathered.
#[derive(Default, Clone)]
pub struct BiomeRegistry {
    defs: Vec<BiomeDef>,
}

impl BiomeRegistry {
    /// Adds a biome. Its key must be new.
    pub fn register(&mut self, def: BiomeDef) {
        assert!(
            !self.defs.iter().any(|other| other.key == def.key),
            "the biome {} is registered twice",
            def.key
        );
        self.defs.push(def);
    }

    /// Changes the biome `key` before the registry is frozen, such as to shrink a biome of the
    /// base game to make room for another. Returns whether there is one.
    pub fn modify(&mut self, key: &str, change: impl FnOnce(&mut BiomeDef)) -> bool {
        let def = self.defs.iter_mut().find(|def| def.key == key);
        def.map(change).is_some()
    }

    /// Freezes the registry. The biomes of the base game keep their ids; the others follow, in
    /// the order of their keys, so that the same biomes get the same ids whatever the order they
    /// were registered in.
    pub fn finish(mut self) -> Result<Biomes, String> {
        let vanilla = vanilla::KEYS.len().min(self.defs.len());
        self.defs[vanilla..].sort_by(|a, b| a.key.cmp(&b.key));
        Biomes::from_defs(self.defs)
    }
}

/// The links of a biome to others, resolved to ids.
#[derive(Debug, Clone, Default)]
struct Links {
    altitude_variants: Vec<(i32, BiomeId)>,
    sub_biomes: Vec<(BiomeId, f64)>,
    border: Option<(BiomeId, f64)>,
}

/// The biome a climate point falls in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pick {
    pub biome: BiomeId,
    /// How far the point lies inside the biome, in units of the parameters: 0 on its border.
    pub margin: f64,
    /// The same, measured only across the borders with other land biomes.
    pub border_margin: f64,
}

/// The axes whose borders `Pick::margin` and `Pick::border_margin` measure.
const ALL_AXES: [usize; 6] = [0, 1, 2, 3, 4, 5];
const LAND_AXES: [usize; 4] = [0, 1, 3, 4];

/// The frozen biomes of a world.
#[derive(Debug, Clone)]
pub struct Biomes {
    defs: Vec<BiomeDef>,
    links: Vec<Links>,
    by_key: HashMap<String, BiomeId>,
    /// Every box, with its biome.
    boxes: Vec<(BiomeId, ClimateBox)>,
}

impl Biomes {
    /// The biomes of `defs`, each with its index as id. Fails when a biome names one that is not
    /// among them or names itself, when its altitude variants lead back to it, and when no biome
    /// has a climate or a number of the climate is not finite: the definitions may come from a
    /// server, and the column of any of these would never be found.
    pub fn from_defs(defs: Vec<BiomeDef>) -> Result<Self, String> {
        if defs.len() > usize::from(u16::MAX) {
            return Err(format!("{} biomes, more than ids can tell", defs.len()));
        }
        if defs.iter().all(|def| def.climate.is_empty()) {
            return Err("no biome has a climate".into());
        }
        for def in &defs {
            let spans = def.climate.iter().flat_map(|climate| climate.spans());
            let bounds = spans.flat_map(|span| [span.min, span.max]);
            let shares = def.sub_biomes.iter().map(|(_, share)| *share);
            let width = def.border.iter().map(|rule| rule.width);
            let numbers = bounds.chain(shares).chain(width);
            if !numbers.chain([def.offset]).all(f64::is_finite) {
                return Err(format!("{} has a climate that is not a number", def.key));
            }
            if !(def.weight.is_finite() && def.weight > 0.0) {
                return Err(format!("{} has a weight that is not positive", def.key));
            }
        }
        let by_key: HashMap<String, BiomeId> = defs
            .iter()
            .enumerate()
            .map(|(i, def)| (def.key.clone(), BiomeId(i as u16)))
            .collect();
        if by_key.len() != defs.len() {
            return Err("two biomes share a key".into());
        }
        let find = |from: &BiomeDef, key: &str| {
            by_key
                .get(key)
                .copied()
                .filter(|id| defs[id.0 as usize].key != from.key)
                .ok_or_else(|| format!("{} names the unknown biome {key}", from.key))
        };
        let mut links = Vec::with_capacity(defs.len());
        for def in &defs {
            let mut altitude_variants = def
                .altitude_variants
                .iter()
                .map(|(height, key)| Ok((*height, find(def, key)?)))
                .collect::<Result<Vec<_>, String>>()?;
            altitude_variants.sort_by_key(|variant| std::cmp::Reverse(variant.0));
            let sub_biomes = def
                .sub_biomes
                .iter()
                .map(|(key, share)| Ok((find(def, key)?, *share)))
                .collect::<Result<Vec<_>, String>>()?;
            let border = def
                .border
                .as_ref()
                .map(|rule| Ok::<_, String>((find(def, &rule.biome)?, rule.width)))
                .transpose()?;
            links.push(Links {
                altitude_variants,
                sub_biomes,
                border,
            });
        }
        // the altitude variants of a biome, then theirs, never lead back to one of them
        let mut done = vec![false; defs.len()];
        for start in 0..defs.len() {
            let mut path = vec![start];
            let mut next = vec![0];
            while let (Some(&biome), Some(edge)) = (path.last(), next.last_mut()) {
                let Some(&(_, variant)) = links[biome].altitude_variants.get(*edge) else {
                    done[biome] = true;
                    path.pop();
                    next.pop();
                    continue;
                };
                *edge += 1;
                let variant = variant.0 as usize;
                if path.contains(&variant) {
                    let key = &defs[variant].key;
                    return Err(format!("the altitude variants of {key} lead back to it"));
                }
                if !done[variant] {
                    path.push(variant);
                    next.push(0);
                }
            }
        }
        let boxes = defs
            .iter()
            .enumerate()
            .flat_map(|(i, def)| def.climate.iter().map(move |b| (BiomeId(i as u16), *b)))
            .collect();
        Ok(Self {
            defs,
            links,
            by_key,
            boxes,
        })
    }

    /// The biomes of the base game alone.
    pub fn vanilla() -> Arc<Biomes> {
        static VANILLA: OnceLock<Arc<Biomes>> = OnceLock::new();
        VANILLA
            .get_or_init(|| Arc::new(vanilla::registry().finish().unwrap()))
            .clone()
    }

    pub fn get(&self, id: BiomeId) -> &BiomeDef {
        &self.defs[id.0 as usize]
    }

    /// The biome `key`; a bare name is that of a biome of the base game: `jungle` is
    /// `rustcraft:jungle`.
    pub fn id(&self, key: &str) -> Option<BiomeId> {
        self.by_key
            .get(key)
            .or_else(|| self.by_key.get(&format!("{BASE_NAMESPACE}{key}")))
            .copied()
    }

    /// Every definition, by id: what the server sends to the clients.
    pub fn defs(&self) -> &[BiomeDef] {
        &self.defs
    }

    pub fn ids(&self) -> impl Iterator<Item = BiomeId> {
        (0..self.defs.len() as u16).map(BiomeId)
    }

    /// The altitude variants of `id`, the highest first.
    pub fn altitude_variants(&self, id: BiomeId) -> &[(i32, BiomeId)] {
        &self.links[id.0 as usize].altitude_variants
    }

    pub fn sub_biomes(&self, id: BiomeId) -> &[(BiomeId, f64)] {
        &self.links[id.0 as usize].sub_biomes
    }

    pub fn border(&self, id: BiomeId) -> Option<(BiomeId, f64)> {
        self.links[id.0 as usize].border
    }

    /// The biome `point` falls in, `selector` in `[0, 1)` sharing the climates several biomes
    /// claim:
    ///
    /// 1. of the boxes holding the point, only those of the highest priority are kept;
    /// 2. one biome holds it: that one;
    /// 3. several do: `selector` picks one, each taking a slice of `[0, 1)` as wide as its
    ///    weight, in the order of their ids;
    /// 4. none does: the nearest box, its biome's offset added to its distance, the higher
    ///    priority then the lower id breaking ties.
    pub fn pick(&self, point: &ClimatePoint, selector: f64) -> Pick {
        let holding = self.boxes.iter().filter(|(_, b)| b.distance(point) == 0.0);
        let Some(priority) = holding.clone().map(|(id, _)| self.get(*id).priority).max() else {
            return self.nearest(point);
        };
        // each biome once, with the margin of the box that holds the point the deepest
        let mut candidates: Vec<(BiomeId, f64, f64)> = Vec::new();
        for (id, climate) in holding.filter(|(id, _)| self.get(*id).priority == priority) {
            let margins = (
                climate.margin(point, &ALL_AXES),
                climate.margin(point, &LAND_AXES),
            );
            match candidates.iter_mut().find(|(other, ..)| other == id) {
                Some(found) => {
                    found.1 = found.1.max(margins.0);
                    found.2 = found.2.max(margins.1);
                }
                None => candidates.push((*id, margins.0, margins.1)),
            }
        }
        candidates.sort_by_key(|(id, ..)| *id);
        if let [(biome, margin, border_margin)] = candidates[..] {
            return Pick {
                biome,
                margin,
                border_margin,
            };
        }
        let total: f64 = candidates.iter().map(|(id, ..)| self.get(*id).weight).sum();
        let target = selector.clamp(0.0, 1.0) * total;
        let mut start = 0.0;
        for (i, &(biome, margin, border_margin)) in candidates.iter().enumerate() {
            let end = start + self.get(biome).weight;
            if target < end || i == candidates.len() - 1 {
                // the edges of a slice are borders too, in units of the selector
                let slice = ((target - start).min(end - target) / total).max(0.0);
                return Pick {
                    biome,
                    margin: margin.min(slice),
                    border_margin: border_margin.min(slice),
                };
            }
            start = end;
        }
        unreachable!("the last candidate takes what is left")
    }

    fn nearest(&self, point: &ClimatePoint) -> Pick {
        let score = |(id, climate): &(BiomeId, ClimateBox)| {
            let def = self.get(*id);
            (climate.distance(point) + def.offset, -def.priority, *id)
        };
        let biome = self
            .boxes
            .iter()
            .min_by(|a, b| score(a).partial_cmp(&score(b)).unwrap())
            .map_or(BiomeId(0), |(id, _)| *id);
        Pick {
            biome,
            margin: 0.0,
            border_margin: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::vanilla::*;
    use super::*;

    #[test]
    fn biomes_whose_columns_could_never_be_found_are_refused() {
        let with = |change: &dyn Fn(&mut Vec<BiomeDef>)| {
            let mut defs = definitions();
            change(&mut defs);
            Biomes::from_defs(defs)
        };
        assert!(with(&|_| ()).is_ok());
        let looped = with(&|defs| {
            let high = &mut defs[HIGH_MOUNTAIN_GRASS.0 as usize];
            high.altitude_variants = vec![(120, KEYS[MEDIUM_MOUNTAIN.0 as usize].into())];
        });
        assert!(looped.unwrap_err().contains("lead back"));
        let nan = with(&|defs| defs[PLAINS.0 as usize].climate[0].humidity.max = f64::NAN);
        assert!(nan.is_err());
        let weightless = with(&|defs| defs[FLOWER_PLAINS.0 as usize].weight = 0.0);
        assert!(weightless.is_err());
        let climateless = with(&|defs| defs.iter_mut().for_each(|def| def.climate.clear()));
        assert!(climateless.is_err());
        assert!(Biomes::from_defs(Vec::new()).is_err());
    }

    fn point(temperature: f64, humidity: f64, continentalness: f64) -> ClimatePoint {
        ClimatePoint {
            temperature,
            humidity,
            continentalness,
            ..ClimatePoint::default()
        }
    }

    #[test]
    fn the_vanilla_biomes_have_the_ids_of_their_constants() {
        let biomes = Biomes::vanilla();
        assert_eq!(biomes.defs().len(), KEYS.len());
        for (i, key) in KEYS.iter().enumerate() {
            assert_eq!(biomes.id(key), Some(BiomeId(i as u16)), "{key}");
        }
        assert_eq!(biomes.id("jungle"), Some(JUNGLE));
        assert_eq!(biomes.get(JUNGLE).name, "Jungle");
        assert_eq!(biomes.id("nowhere"), None);
    }

    #[test]
    fn the_climate_picks_the_biome_of_the_box_it_falls_in() {
        let biomes = Biomes::vanilla();
        let pick = |t, h, c| biomes.pick(&point(t, h, c), 0.0).biome;
        assert_eq!(pick(0.0, 0.0, -0.9), DEEP_OCEAN);
        assert_eq!(pick(0.0, 0.0, -0.7), OCEAN);
        assert_eq!(pick(0.0, 0.0, -0.5), SHALLOW_OCEAN);
        assert_eq!(pick(0.9, -0.9, 0.5), DESERT);
        assert_eq!(pick(0.9, -0.3, 0.5), SAVANNA);
        assert_eq!(pick(0.9, 0.6, 0.0), JUNGLE);
        // hot and wet, but far inland
        assert_eq!(pick(0.9, 0.6, 0.6), FOREST);
        assert_eq!(pick(-0.8, 0.5, 0.5), ICE_PLAIN);
        assert_eq!(pick(-0.8, -0.8, 0.5), PLAINS);
        assert_eq!(pick(0.0, 0.5, 0.5), FOREST);
    }

    #[test]
    fn every_climate_has_a_biome_even_outside_every_box() {
        let mut biomes = vanilla::registry();
        biomes.modify("rustcraft:desert", |def| def.climate.clear());
        let biomes = biomes.finish().unwrap();
        // where the desert was, the savanna next to it is the nearest
        let pick = biomes.pick(&point(0.9, -0.9, 0.5), 0.5);
        assert_eq!(pick.biome, SAVANNA);
        assert_eq!(pick.margin, 0.0);
    }

    #[test]
    fn a_higher_priority_takes_its_box_from_the_others() {
        let biomes = Biomes::vanilla();
        let cold = |weirdness| ClimatePoint {
            weirdness,
            ..point(-0.9, 0.5, 0.5)
        };
        assert_eq!(biomes.pick(&cold(0.9), 0.0).biome, ICE_SPIKES);
        assert_eq!(biomes.pick(&cold(0.9), 0.99).biome, ICE_SPIKES);
        assert_eq!(biomes.pick(&cold(0.5), 0.0).biome, ICE_PLAIN);
    }

    /// A registry with two biomes sharing a box, of weights 1 and `weight`.
    fn shared(weight: f64) -> Biomes {
        let mut registry = BiomeRegistry::default();
        let climate = vec![ClimateBox::default()];
        registry.register(BiomeDef {
            climate: climate.clone(),
            ..BiomeDef::new("test:a", "A")
        });
        registry.register(BiomeDef {
            climate,
            weight,
            ..BiomeDef::new("test:b", "B")
        });
        registry.finish().unwrap()
    }

    #[test]
    fn biomes_sharing_a_climate_split_it_by_weight() {
        for weight in [1.0, 0.25, 3.0] {
            let biomes = shared(weight);
            let b = biomes.id("test:b").unwrap();
            let samples = 10_000;
            let won = (0..samples)
                .filter(|i| {
                    biomes
                        .pick(&ClimatePoint::default(), *i as f64 / samples as f64)
                        .biome
                        == b
                })
                .count();
            let expected = weight / (1.0 + weight);
            let share = won as f64 / samples as f64;
            assert!((share - expected).abs() < 0.01, "{weight}: {share}");
        }
    }

    #[test]
    fn the_edge_of_a_slice_is_a_border() {
        let biomes = shared(1.0);
        assert!(biomes.pick(&ClimatePoint::default(), 0.499).margin < 0.01);
        assert!(biomes.pick(&ClimatePoint::default(), 0.25).margin > 0.2);
    }

    #[test]
    fn the_margin_grows_from_the_border_inward_and_ignores_the_ends_of_the_range() {
        let biomes = Biomes::vanilla();
        let margin = |h| biomes.pick(&point(0.9, h, 0.0), 0.0).margin;
        // the jungle's humidity starts at 0.2; its continentalness ends at 0.3
        assert!(margin(0.21) < margin(0.25));
        assert!((margin(0.25) - 0.05).abs() < 1e-9);
        assert!((margin(0.99) - 0.3).abs() < 1e-9, "{}", margin(0.99));
        // across the coast, which is no border with another land biome
        let coast = biomes.pick(&point(0.9, 0.6, COAST + 0.01), 0.0);
        assert!(
            coast.margin < 0.02 && coast.border_margin > 0.2,
            "{coast:?}"
        );
    }

    #[test]
    fn ids_follow_the_keys_whatever_the_order_of_registration() {
        let def = |key: &str| BiomeDef::new(key, key);
        let mut first = vanilla::registry();
        first.register(def("mod:b"));
        first.register(def("mod:a"));
        let mut second = vanilla::registry();
        second.register(def("mod:a"));
        second.register(def("mod:b"));
        let (first, second) = (first.finish().unwrap(), second.finish().unwrap());
        assert_eq!(first.defs(), second.defs());
        assert_eq!(first.id("mod:a"), Some(BiomeId(KEYS.len() as u16)));
        assert_eq!(first.id("jungle"), Some(JUNGLE));
    }

    #[test]
    fn a_biome_naming_an_unknown_one_is_refused() {
        let mut registry = vanilla::registry();
        registry.register(BiomeDef {
            sub_biomes: vec![("mod:nowhere".into(), 0.5)],
            ..BiomeDef::new("mod:somewhere", "Somewhere")
        });
        let err = registry.finish().unwrap_err();
        assert!(err.contains("mod:nowhere"), "{err}");
    }

    #[test]
    fn a_mod_can_add_a_biome_and_shrink_a_vanilla_one() {
        let mut registry = vanilla::registry();
        assert!(registry.modify("rustcraft:desert", |def| {
            def.climate[0].humidity.max = -0.7;
        }));
        registry.register(BiomeDef {
            climate: vec![ClimateBox {
                temperature: Span::new(0.2, 1.0),
                humidity: Span::new(-0.7, -0.5),
                continentalness: Span::new(COAST, 1.0),
                ..ClimateBox::default()
            }],
            ..BiomeDef::new("mymod:badlands", "Badlands")
        });
        let biomes = registry.finish().unwrap();
        let badlands = biomes.id("mymod:badlands").unwrap();
        assert_eq!(biomes.pick(&point(0.9, -0.6, 0.5), 0.0).biome, badlands);
        assert_eq!(biomes.pick(&point(0.9, -0.9, 0.5), 0.0).biome, DESERT);
    }

    #[test]
    fn the_definitions_survive_the_trip_to_the_client() {
        let biomes = Biomes::vanilla();
        let sent = bincode::serialize(biomes.defs()).unwrap();
        let received: Vec<BiomeDef> = bincode::deserialize(&sent).unwrap();
        let rebuilt = Biomes::from_defs(received).unwrap();
        assert_eq!(rebuilt.defs(), biomes.defs());
        for (t, h) in [(0.9, 0.6), (-0.8, 0.5), (0.0, 0.0)] {
            assert_eq!(
                rebuilt.pick(&point(t, h, 0.2), 0.3),
                biomes.pick(&point(t, h, 0.2), 0.3)
            );
        }
    }
}
