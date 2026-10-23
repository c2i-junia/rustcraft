//! Which chunks the client of a player is sent and keeps: those within its render distance, and
//! those around the twin of every open portal within it, which the portal shows. The server
//! sends, unloads, loads and forgets chunks by this one rule; the client keeps what it is sent
//! until the server tells it to forget it.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use bevy::math::IVec3;

use crate::players::{movement::top_speed, Player};
use crate::world::{chunk_in_reach, render_distance_offsets, vertical_render_distance, Portals};
use crate::CHUNK_SIZE;

/// How far around the twin of an open portal the chunks are sent to the players that see the
/// portal, in chunks, while they are not near it (`PORTAL_APPROACH_RADIUS`).
pub const PORTAL_VIEW_RADIUS: i32 = 4;

/// How near its player an open portal is approached at least, in chunks along x, y and z between
/// the player's chunk and the portal's (`Portal::chunk`): the twin's chunks are then sent out to
/// the player's own render distance (`MAX_APPROACHED_EXTRA_CHUNKS` at most), so that the view
/// through the portal is the view after crossing and crossing needs no load. 2 chunks are 17 to
/// 47 blocks from the portal: at least 2.6 s at a sprint on foot (6.5 blocks a second). A faster
/// player approaches from farther (`approach_radius`).
pub const PORTAL_APPROACH_RADIUS: i32 = 2;

/// The widest approach, in chunks: 8 holds the twin's area `APPROACH_LEAD_SECS` ahead up to
/// 56 blocks a second, past a flying sprint (40) and most of a fall.
pub const MAX_PORTAL_APPROACH_RADIUS: i32 = 8;

/// How long before reaching a portal its twin's area is to be asked for, in seconds: the server
/// makes and sends a capped area (`MAX_APPROACHED_EXTRA_CHUNKS`) in 0.7 s (101 ms to make,
/// 12 broadcasts of 50 ms), and the client meshes what comes in the rest.
pub const APPROACH_LEAD_SECS: f32 = 2.0;

/// The most chunks the areas of the twins approached add, together, to those within the
/// player's own render distance. 10 625 is a render distance of 12 (25 × 25 × 17), which was
/// measured: made in 101 ms, sent in 12 broadcasts, 1.3 MiB. A render distance up to 12 is not
/// capped, nor a twin within the player's own area; at 64, where the player holds 283 000
/// chunks, the cap adds 3.8 % at most, however many portals are approached.
pub const MAX_APPROACHED_EXTRA_CHUNKS: usize = 10_625;

/// The approach (`ChunkReach::approaching`) of a player moving at `speed` blocks a second: a
/// portal is approached `APPROACH_LEAD_SECS` before reaching it, from 1 chunk farther, as the
/// chunk of the portal is reached from as far as 1 block away; between `PORTAL_APPROACH_RADIUS`
/// and `MAX_PORTAL_APPROACH_RADIUS`.
pub fn approach_radius_at(speed: f32) -> i32 {
    let chunks = (speed * APPROACH_LEAD_SECS / CHUNK_SIZE as f32).ceil() as i32 + 1;
    chunks.clamp(PORTAL_APPROACH_RADIUS, MAX_PORTAL_APPROACH_RADIUS)
}

/// The speed past which a player's velocity counts towards its approach, in blocks a second: a
/// fall of 2.4 blocks or more, or a throw. A jump (`JUMP_VELOCITY`, 8.66) and its fall, even at
/// a sprint, stay under it, so that they do not change the approach on every hop.
pub const FAST_SPEED: f32 = 12.0;

/// The approach of `player`, by how fast it may go: the speed of its stance (flying, sprinting)
/// whether it moves or not, or its velocity, once past `FAST_SPEED`, when falling or thrown.
pub fn approach_radius(player: &Player) -> i32 {
    let velocity = player.velocity.length();
    let fast = if velocity > FAST_SPEED { velocity } else { 0.0 };
    approach_radius_at(top_speed(player).max(fast))
}

/// The step the radius of a capped area is rounded down to (`ChunkReach::approached_radius`):
/// the overlap with the player's own area changes it by about a shell for every chunk the
/// player crosses, and each radius sorts its offsets anew (`twin_area_offsets`).
pub const CAPPED_RADIUS_STEP: i32 = 4;

/// How many chunks a box of `horizontal` chunks along x and z and `vertical` along y around a
/// chunk holds.
fn box_volume(horizontal: i32, vertical: i32) -> usize {
    let side = (2 * horizontal + 1) as usize;
    side * side * (2 * vertical + 1) as usize
}

/// How many chunks the boxes around `a` and `b`, of (horizontal, vertical) half sizes `ha` and
/// `hb`, share.
fn boxes_overlap(a: IVec3, ha: (i32, i32), b: IVec3, hb: (i32, i32)) -> usize {
    let half = |(h, v): (i32, i32)| IVec3::new(h, v, h);
    let low = (a - half(ha)).max(b - half(hb));
    let high = (a + half(ha)).min(b + half(hb));
    let span = (high - low + IVec3::ONE).max(IVec3::ZERO);
    span.x as usize * span.y as usize * span.z as usize
}

/// The chunks around the twin of an open portal that a player is sent: `radius` chunks along x
/// and z, `vertical_render_distance(radius)` along y, around the twin's chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwinArea {
    /// The chunk of the twin (`Portal::chunk`): the one in front of its opening.
    pub twin: IVec3,
    /// The way the twin faces, a unit axis: what its portal shows lies that way.
    pub facing: IVec3,
    /// The player's render distance when its portal is approached, else `PORTAL_VIEW_RADIUS`.
    pub radius: i32,
}

impl TwinArea {
    /// Whether `chunk` is in the area grown by `margin`.
    pub fn contains(&self, chunk: IVec3, margin: i32) -> bool {
        chunk_in_reach(
            chunk,
            self.twin,
            self.radius + margin,
            vertical_render_distance(self.radius) + margin,
        )
    }

    /// The chunks of the area, in the order they are sent (`twin_area_offsets`).
    pub fn chunks(&self) -> impl Iterator<Item = IVec3> + '_ {
        let offsets = twin_area_offsets(self.radius, self.facing);
        let twin = self.twin;
        (0..offsets.len()).map(move |i| twin + offsets[i])
    }
}

/// The order of the chunk `offset` from a twin facing `facing` in its area: by shell (its
/// largest offset along x, y and z, the boxes the client's fog counts), a shell behind the twin's
/// plane, which its portal does not show, counting twice as far; then nearest first.
pub fn twin_area_key(offset: IVec3, facing: IVec3) -> (i32, i32) {
    let shell = offset.abs().max_element();
    let behind = offset.dot(facing) < 0;
    (
        if behind { 2 * shell } else { shell },
        offset.length_squared(),
    )
}

/// How many lists of offsets `twin_area_offsets` keeps: a few facings at a few radii are in use
/// at once.
const OFFSETS_KEPT: usize = 16;

/// The lists of offsets of `twin_area_offsets`, the least recently used going first once
/// `OFFSETS_KEPT` are held: the list of the client's own render distance, used every frame its
/// fog is measured, stays while the server's capped radii come and go.
#[derive(Default)]
struct OffsetsCache {
    /// Each list, with when it was last used.
    entries: HashMap<(i32, IVec3), (Arc<[IVec3]>, u64)>,
    /// Counts the uses.
    clock: u64,
}

impl OffsetsCache {
    fn get(&mut self, radius: i32, facing: IVec3) -> Arc<[IVec3]> {
        self.clock += 1;
        let clock = self.clock;
        if !self.entries.contains_key(&(radius, facing)) && self.entries.len() >= OFFSETS_KEPT {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| *key);
            if let Some(oldest) = oldest {
                self.entries.remove(&oldest);
            }
        }
        let (offsets, used) = self.entries.entry((radius, facing)).or_insert_with(|| {
            let mut offsets = render_distance_offsets(radius).to_vec();
            // Stable: the same order on the server and the client
            offsets.sort_by_key(|offset| twin_area_key(*offset, facing));
            (offsets.into(), 0)
        });
        *used = clock;
        offsets.clone()
    }
}

/// The offsets from a twin facing `facing` of the chunks of its area of `radius`, in the order
/// they are sent (`twin_area_key`). Each is computed once, while in use (`OffsetsCache`).
pub fn twin_area_offsets(radius: i32, facing: IVec3) -> Arc<[IVec3]> {
    static OFFSETS: LazyLock<Mutex<OffsetsCache>> = LazyLock::new(Default::default);
    OFFSETS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(radius, facing)
}

/// An open portal seen from a reach: its chunk, and its twin's chunk and facing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct View {
    portal: IVec3,
    twin: IVec3,
    facing: IVec3,
}

/// The reach of a player in the chunk `center` with the render distance `distance`, among the
/// portals of the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkReach {
    pub center: IVec3,
    pub distance: i32,
    /// The open portals, sorted.
    views: Vec<View>,
    /// How near a portal is approached, in chunks (`approach_radius`).
    approach: i32,
}

impl ChunkReach {
    pub fn new(center: IVec3, distance: i32, portals: &Portals) -> Self {
        let mut views: Vec<View> = portals
            .iter()
            .filter_map(|portal| {
                let twin = portals.twin(portal)?;
                Some(View {
                    portal: portal.chunk(),
                    twin: twin.chunk(),
                    facing: twin.normal().round().as_ivec3(),
                })
            })
            .collect();
        views.sort_by_key(|view| {
            (
                view.portal.to_array(),
                view.twin.to_array(),
                view.facing.to_array(),
            )
        });
        Self {
            center,
            distance,
            views,
            approach: PORTAL_APPROACH_RADIUS,
        }
    }

    /// The reach of `player`, with the render distance `distance`, its approach after its speed
    /// (`approach_radius`).
    pub fn of_player(player: &Player, distance: i32, portals: &Portals) -> Self {
        Self::new(
            crate::world::world_position_to_chunk_position(player.position),
            distance,
            portals,
        )
        .approaching(approach_radius(player))
    }

    /// This reach, with portals approached within `approach` chunks. With no portal within the
    /// widest approach, no approach changes anything: it is `PORTAL_APPROACH_RADIUS`, so that the
    /// reach, compared to know whether to walk the chunks again, stays the same.
    pub fn approaching(mut self, approach: i32) -> Self {
        let near = self.views.iter().any(|view| {
            chunk_in_reach(
                view.portal,
                self.center,
                MAX_PORTAL_APPROACH_RADIUS,
                MAX_PORTAL_APPROACH_RADIUS,
            )
        });
        self.approach = if near {
            approach
        } else {
            PORTAL_APPROACH_RADIUS
        };
        self
    }

    /// Whether `chunk` is within the render distance plus `margin`: the player's own chunks,
    /// the margin's included, which its own camera draws; the others are only seen through
    /// portals.
    pub fn within(&self, chunk: IVec3, margin: i32) -> bool {
        chunk_in_reach(
            chunk,
            self.center,
            self.distance + margin,
            vertical_render_distance(self.distance) + margin,
        )
    }

    /// Whether the portal in the chunk `portal` is approached (`approaching`); with a `margin`,
    /// whether it is within the widest approach grown by it: what was sent while the player went
    /// faster is kept until it is that far, so that releasing a sprint or landing does not unload
    /// it, while an area never sent costs nothing to keep.
    fn approaches(&self, portal: IVec3, margin: i32) -> bool {
        let radius = if margin > 0 {
            MAX_PORTAL_APPROACH_RADIUS + margin
        } else {
            self.approach
        };
        chunk_in_reach(portal, self.center, radius, radius)
    }

    /// The radius of the area around the approached twin in the chunk `twin`: the render
    /// distance, or as many chunks as the chunks it adds to the player's own fit in `budget`,
    /// which they are taken from, rounded down to `CAPPED_RADIUS_STEP`; `PORTAL_VIEW_RADIUS`
    /// at least.
    fn approached_radius(&self, twin: IVec3, budget: &mut usize) -> i32 {
        let own = (self.distance, vertical_render_distance(self.distance));
        let capped = (PORTAL_VIEW_RADIUS + 1..self.distance)
            .rev()
            .filter(|radius| radius % CAPPED_RADIUS_STEP == 0);
        let whole = (self.distance > PORTAL_VIEW_RADIUS).then_some(self.distance);
        for radius in whole.into_iter().chain(capped) {
            let size = (radius, vertical_render_distance(radius));
            let added = box_volume(size.0, size.1) - boxes_overlap(twin, size, self.center, own);
            if added <= *budget {
                *budget -= added;
                return radius;
            }
        }
        PORTAL_VIEW_RADIUS
    }

    /// The areas around the twins of the open portals within the render distance plus `margin`,
    /// one per twin: the player's render distance around the twins of the portals approached
    /// (with a margin, within the widest approach grown by it: `approaches`), as far as
    /// `MAX_APPROACHED_EXTRA_CHUNKS` allows, the nearest portal first; `PORTAL_VIEW_RADIUS`
    /// around the others. Those approached only with the margin are farther than the others, so
    /// each area sent has the same radius whatever the margin; with a margin, a capped area is
    /// a `CAPPED_RADIUS_STEP` wider, what keeping it needs across a chunk border.
    pub fn areas_within(&self, margin: i32) -> Vec<TwinArea> {
        let mut areas: Vec<TwinArea> = Vec::new();
        let mut views: Vec<&View> = self
            .views
            .iter()
            .filter(|view| self.within(view.portal, margin))
            .collect();
        // Stable: the same order on the server and the client
        views.sort_by_key(|view| (view.portal - self.center).abs().max_element());
        let mut budget = MAX_APPROACHED_EXTRA_CHUNKS;
        for view in views {
            let radius = if self.approaches(view.portal, margin) {
                let radius = self.approached_radius(view.twin, &mut budget);
                if margin > 0 && radius < self.distance {
                    // Kept a step wider: a step back across a chunk border can round the capped
                    // radius down a whole step, and what was sent at the wider one stays
                    (radius + CAPPED_RADIUS_STEP).min(self.distance)
                } else {
                    radius
                }
            } else {
                PORTAL_VIEW_RADIUS
            };
            match areas.iter_mut().find(|area| area.twin == view.twin) {
                Some(area) if area.radius < radius => {
                    area.radius = radius;
                    area.facing = view.facing;
                }
                Some(_) => {}
                None => areas.push(TwinArea {
                    twin: view.twin,
                    facing: view.facing,
                    radius,
                }),
            }
        }
        areas
    }

    /// The areas around the twins of the open portals within the render distance
    /// (`areas_within`).
    pub fn areas(&self) -> Vec<TwinArea> {
        self.areas_within(0)
    }

    /// The chunks of the twins of the open portals the player approaches (`approaching`), each
    /// once: where it may be in a moment.
    pub fn approached_twins(&self) -> Vec<IVec3> {
        self.approached_twins_within(0)
    }

    /// `approached_twins`, the approach and the render distance grown by `margin`.
    pub fn approached_twins_within(&self, margin: i32) -> Vec<IVec3> {
        let mut twins: Vec<IVec3> = Vec::new();
        for view in &self.views {
            if self.within(view.portal, margin)
                && self.approaches(view.portal, margin)
                && !twins.contains(&view.twin)
            {
                twins.push(view.twin);
            }
        }
        twins
    }

    /// The chunks of the twins of the open portals within the render distance, each once.
    pub fn twins(&self) -> Vec<IVec3> {
        self.areas().into_iter().map(|area| area.twin).collect()
    }

    /// The chunks of the areas around the twins (`areas`) that are not within the render
    /// distance, each once, in the order they are to be sent: those of every area in their
    /// order in it (`twin_area_key`), merged, so that a small area is not kept waiting behind a
    /// large one.
    pub fn portal_chunks(&self) -> Vec<IVec3> {
        let areas = self.areas();
        let offsets: Vec<Arc<[IVec3]>> = areas
            .iter()
            .map(|area| twin_area_offsets(area.radius, area.facing))
            .collect();
        let mut next = vec![0; areas.len()];
        let mut chunks = Vec::new();
        // The area whose next chunk comes first
        let first = |next: &[usize]| {
            (0..areas.len())
                .filter(|&index| next[index] < offsets[index].len())
                .min_by_key(|&index| {
                    twin_area_key(offsets[index][next[index]], areas[index].facing)
                })
        };
        while let Some(index) = first(&next) {
            let chunk = areas[index].twin + offsets[index][next[index]];
            next[index] += 1;
            // Each chunk is listed by the first area it is in
            if !self.within(chunk, 0) && !areas[..index].iter().any(|area| area.contains(chunk, 0))
            {
                chunks.push(chunk);
            }
        }
        chunks
    }

    /// What a client keeps, `margin` chunks beyond the reach (`keeps`), worked out once for many
    /// chunks.
    pub fn kept(&self, margin: i32) -> Kept<'_> {
        Kept {
            reach: self,
            areas: self.areas_within(margin),
            margin,
        }
    }

    /// Whether a client keeps `chunk`, `margin` chunks beyond the reach: it is within the render
    /// distance plus `margin`, or within the area of a twin of an open portal within the render
    /// distance plus `margin` (`areas_within`), grown by `margin`. A portal approached keeps
    /// the twin's render distance while it is within the widest approach plus `margin`.
    pub fn keeps(&self, chunk: IVec3, margin: i32) -> bool {
        self.kept(margin).contains(chunk)
    }
}

/// The chunks a client keeps, `margin` chunks beyond a reach (`ChunkReach::kept`).
pub struct Kept<'a> {
    reach: &'a ChunkReach,
    areas: Vec<TwinArea>,
    margin: i32,
}

impl Kept<'_> {
    pub fn contains(&self, chunk: IVec3) -> bool {
        self.reach.within(chunk, self.margin)
            || self
                .areas
                .iter()
                .any(|area| area.contains(chunk, self.margin))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{within_render_distance, FaceDirection, Portal, PortalColor};
    use crate::CHUNK_SIZE;

    const DISTANCE: i32 = 2;

    /// A portal of `owner` and `color` facing +Z, opening onto the chunk `chunk`.
    fn portal(owner: u64, color: PortalColor, chunk: IVec3) -> Portal {
        let portal = Portal {
            owner,
            color,
            anchor: chunk * CHUNK_SIZE + IVec3::new(4, 4, 4),
            face: FaceDirection::PlusZ,
            up: FaceDirection::PlusY,
        };
        assert_eq!(portal.chunk(), chunk);
        portal
    }

    /// The portals of `owner`: blue in the chunk `blue`, orange in `orange`.
    fn pair(portals: &mut Portals, owner: u64, blue: IVec3, orange: IVec3) {
        portals.insert(portal(owner, PortalColor::Blue, blue));
        portals.insert(portal(owner, PortalColor::Orange, orange));
    }

    const FAR: IVec3 = IVec3::new(40, 0, 0);

    #[test]
    fn a_portal_opens_onto_the_chunk_in_front_of_its_opening() {
        // On the last cells of a chunk along z, facing +Z: what it opens onto is the next chunk
        let portal = Portal {
            owner: 1,
            color: PortalColor::Blue,
            anchor: IVec3::new(3, 2, CHUNK_SIZE - 1),
            face: FaceDirection::PlusZ,
            up: FaceDirection::PlusY,
        };
        assert_eq!(portal.chunk(), IVec3::new(0, 0, 1));
    }

    #[test]
    fn the_chunks_around_the_twin_of_an_open_portal_within_reach_are_sent() {
        let mut portals = Portals::default();
        pair(&mut portals, 1, IVec3::new(1, 0, 0), FAR);
        let reach = ChunkReach::new(IVec3::ZERO, DISTANCE, &portals);

        // Through the orange portal, far away, the blue one shows nothing within reach: only the
        // blue one, within reach, has its twin's chunks sent
        assert_eq!(reach.twins(), vec![FAR]);
        let around = reach.portal_chunks();
        let offsets = render_distance_offsets(PORTAL_VIEW_RADIUS);
        assert_eq!(around.len(), offsets.len());
        assert_eq!(around[0], FAR);
        assert!(around
            .iter()
            .all(|chunk| within_render_distance(*chunk, FAR, PORTAL_VIEW_RADIUS)));
    }

    #[test]
    fn a_lone_portal_or_one_out_of_reach_sends_nothing_more() {
        let mut portals = Portals::default();
        portals.insert(portal(1, PortalColor::Blue, IVec3::X));
        // Another player's pair, beyond the render distance
        pair(&mut portals, 2, IVec3::new(DISTANCE + 1, 0, 0), FAR);
        let reach = ChunkReach::new(IVec3::ZERO, DISTANCE, &portals);
        assert!(reach.twins().is_empty());
        assert!(reach.portal_chunks().is_empty());
    }

    #[test]
    fn the_chunks_around_twins_come_once_and_not_those_of_the_player() {
        let mut portals = Portals::default();
        // A twin right beside the player, and two next to each other far away
        pair(&mut portals, 1, IVec3::NEG_X, IVec3::new(3, 0, 0));
        pair(&mut portals, 2, IVec3::Z, FAR);
        pair(&mut portals, 3, IVec3::NEG_Z, FAR + IVec3::X);
        let reach = ChunkReach::new(IVec3::ZERO, DISTANCE, &portals);

        let around = reach.portal_chunks();
        let unique: std::collections::HashSet<IVec3> = around.iter().copied().collect();
        assert_eq!(unique.len(), around.len());
        assert!(around
            .iter()
            .all(|chunk| !within_render_distance(*chunk, IVec3::ZERO, DISTANCE)));
        let expected = |chunk: &IVec3| {
            !within_render_distance(*chunk, IVec3::ZERO, DISTANCE)
                && reach
                    .twins()
                    .iter()
                    .any(|twin| within_render_distance(*chunk, *twin, PORTAL_VIEW_RADIUS))
        };
        let span = FAR.x + PORTAL_VIEW_RADIUS + 2;
        let all = (-span..=span).flat_map(|x| {
            (-span..=span).flat_map(move |y| (-span..=span).map(move |z| IVec3::new(x, y, z)))
        });
        assert_eq!(all.filter(expected).count(), around.len());
    }

    #[test]
    fn the_chunks_around_a_twin_are_kept_with_the_margin_while_its_portal_is_within_reach_plus_it()
    {
        let mut portals = Portals::default();
        let portal = IVec3::new(DISTANCE + 1, 0, 0);
        pair(&mut portals, 1, portal, FAR);
        let near = ChunkReach::new(IVec3::ZERO, DISTANCE, &portals);
        let edge = FAR + IVec3::new(PORTAL_VIEW_RADIUS + 1, 0, 0);
        let beyond = FAR + IVec3::new(0, PORTAL_VIEW_RADIUS + 2, 0);

        // The portal is beyond the render distance, within the margin: its twin's chunks are not
        // sent, but kept
        assert!(near.portal_chunks().is_empty());
        assert!(near.keeps(FAR, 1));
        assert!(near.keeps(edge, 1));
        assert!(!near.keeps(edge, 0));
        assert!(!near.keeps(beyond, 1));
        // The player's own, as before
        assert!(near.keeps(IVec3::new(0, 0, -DISTANCE - 1), 1));
        assert!(near.within(IVec3::new(0, 0, -DISTANCE - 1), 1));
        assert!(!near.within(IVec3::new(0, 0, -DISTANCE - 1), 0));
        assert!(!near.within(FAR, 1));
        assert!(!near.keeps(IVec3::new(0, 0, -DISTANCE - 2), 1));

        // One chunk farther, the portal is beyond the margin too
        let far = ChunkReach::new(IVec3::NEG_X, DISTANCE, &portals);
        assert!(!far.keeps(FAR, 1));

        // Without its twin, the portal keeps nothing
        portals.remove(1, PortalColor::Orange);
        let closed = ChunkReach::new(IVec3::ZERO, DISTANCE, &portals);
        assert!(!closed.keeps(FAR, 1));
        assert_ne!(closed, near);
    }

    /// A render distance larger than `PORTAL_VIEW_RADIUS`, as players have.
    const WIDE: i32 = 6;

    #[test]
    fn an_approached_portal_has_the_render_distance_of_its_twin_sent() {
        let mut portals = Portals::default();
        pair(
            &mut portals,
            1,
            IVec3::new(PORTAL_APPROACH_RADIUS, 0, 0),
            FAR,
        );
        let reach = ChunkReach::new(IVec3::ZERO, WIDE, &portals);

        assert_eq!(
            reach.areas(),
            vec![TwinArea {
                twin: FAR,
                facing: IVec3::Z,
                radius: WIDE,
            }]
        );
        let around = reach.portal_chunks();
        assert_eq!(around.len(), render_distance_offsets(WIDE).len());
        assert_eq!(around[0], FAR);
        assert!(around
            .iter()
            .all(|chunk| within_render_distance(*chunk, FAR, WIDE)));
        // Vertically too
        assert!(around.contains(&(FAR + IVec3::new(0, -WIDE, 0))));
    }

    #[test]
    fn the_twins_approached_are_those_of_the_portals_within_the_approach() {
        let mut portals = Portals::default();
        pair(
            &mut portals,
            1,
            IVec3::new(0, 0, PORTAL_APPROACH_RADIUS),
            FAR,
        );
        pair(
            &mut portals,
            2,
            IVec3::new(0, 0, -PORTAL_APPROACH_RADIUS - 1),
            -FAR,
        );
        let reach = ChunkReach::new(IVec3::ZERO, WIDE, &portals);
        // The twins of the first pair face each other: each is a portal approached or not
        assert_eq!(reach.approached_twins(), vec![FAR]);
    }

    #[test]
    fn a_portal_beyond_the_approach_has_only_the_view_radius_of_its_twin_sent() {
        let mut portals = Portals::default();
        // Within the render distance, one chunk beyond the approach
        pair(
            &mut portals,
            1,
            IVec3::new(0, 0, PORTAL_APPROACH_RADIUS + 1),
            FAR,
        );
        let reach = ChunkReach::new(IVec3::ZERO, WIDE, &portals);

        assert_eq!(reach.areas()[0].radius, PORTAL_VIEW_RADIUS);
        assert_eq!(
            reach.portal_chunks().len(),
            render_distance_offsets(PORTAL_VIEW_RADIUS).len()
        );
        // Above or below by as much, too
        pair(
            &mut portals,
            1,
            IVec3::new(0, PORTAL_APPROACH_RADIUS + 1, 0),
            FAR,
        );
        let reach = ChunkReach::new(IVec3::ZERO, WIDE, &portals);
        assert_eq!(reach.areas()[0].radius, PORTAL_VIEW_RADIUS);
    }

    #[test]
    fn the_chunks_of_an_area_come_shell_after_shell_those_the_twin_faces_first() {
        let offsets = twin_area_offsets(WIDE, IVec3::Z);
        assert_eq!(offsets.len(), render_distance_offsets(WIDE).len());
        assert_eq!(offsets[0], IVec3::ZERO);
        let keys: Vec<(i32, i32)> = offsets
            .iter()
            .map(|offset| twin_area_key(*offset, IVec3::Z))
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));
        // Each shell in front of the twin before the next one, so that the boxes the client
        // counts are held in turn
        let position = |offset: IVec3| offsets.iter().position(|o| *o == offset).unwrap();
        let shell = |offset: IVec3| offset.abs().max_element();
        for (index, offset) in offsets.iter().enumerate() {
            if offset.z >= 0 {
                assert!(offsets[..index]
                    .iter()
                    .filter(|o| o.z >= 0)
                    .all(|o| shell(*o) <= shell(*offset)));
            }
        }
        // In front before behind, at the same distance
        assert!(position(IVec3::new(0, 0, 1)) < position(IVec3::new(0, 0, -1)));
        assert!(position(IVec3::new(1, 1, 1)) < position(IVec3::new(0, 0, -1)));
        assert!(position(IVec3::new(0, 0, -1)) < position(IVec3::new(0, 0, 3)));
        // The corner of a shell before the face of the next one
        assert!(position(IVec3::new(2, 2, 2)) < position(IVec3::new(0, 0, 3)));
    }

    #[test]
    fn the_areas_of_several_twins_are_merged_each_chunk_once() {
        let mut portals = Portals::default();
        // Their areas overlap, but neither holds the other twin
        let other = FAR + IVec3::new(0, 0, 8);
        pair(&mut portals, 1, IVec3::X, FAR);
        pair(&mut portals, 2, IVec3::new(0, 0, WIDE), other);
        let reach = ChunkReach::new(IVec3::ZERO, WIDE, &portals);

        let areas = reach.areas();
        assert_eq!(areas.len(), 2);
        let radius = |twin| areas.iter().find(|area| area.twin == twin).unwrap().radius;
        assert_eq!((radius(FAR), radius(other)), (WIDE, PORTAL_VIEW_RADIUS));
        let around = reach.portal_chunks();
        // Both twins first
        let first: std::collections::HashSet<IVec3> = around[..2].iter().copied().collect();
        assert_eq!(first, [FAR, other].into());
        let unique: std::collections::HashSet<IVec3> = around.iter().copied().collect();
        assert_eq!(unique.len(), around.len());
        let expected = (-20..=20)
            .flat_map(|x| {
                (-20..=20).flat_map(move |y| (-20..=20).map(move |z| IVec3::new(x, y, z)))
            })
            .map(|offset| FAR + offset)
            .filter(|chunk| areas.iter().any(|area| area.contains(*chunk, 0)))
            .count();
        assert_eq!(around.len(), expected);
    }

    #[test]
    fn the_area_of_an_approached_twin_is_kept_with_the_margin_then_shrinks() {
        let mut portals = Portals::default();
        pair(
            &mut portals,
            1,
            IVec3::new(PORTAL_APPROACH_RADIUS + 1, 0, 0),
            FAR,
        );
        let edge = FAR + IVec3::new(WIDE, 0, 0);
        let core = FAR + IVec3::new(PORTAL_VIEW_RADIUS, 0, 0);

        // One chunk beyond the approach: kept with the margin, not sent
        let near = ChunkReach::new(IVec3::ZERO, WIDE, &portals);
        assert!(!near.portal_chunks().contains(&edge));
        assert!(near.keeps(edge, 1));
        assert!(near.keeps(edge + IVec3::X, 1));
        assert!(!near.keeps(edge, 0));
        assert!(near.keeps(core, 0));

        // Past the widest approach and the margin: only the view radius and its margin
        let farther = ChunkReach::new(
            IVec3::new(
                PORTAL_APPROACH_RADIUS - MAX_PORTAL_APPROACH_RADIUS - 1,
                0,
                0,
            ),
            2 * WIDE,
            &portals,
        );
        assert!(!farther.keeps(edge, 1));
        assert!(farther.keeps(core + IVec3::X, 1));
    }

    #[test]
    fn the_approach_grows_with_the_players_speed_up_to_a_bound() {
        assert_eq!(approach_radius_at(0.0), PORTAL_APPROACH_RADIUS);
        // A sprint on foot: 2 chunks, 17 blocks at least, 2.6 s
        assert_eq!(approach_radius_at(6.5), PORTAL_APPROACH_RADIUS);
        // A sprint in flight, 40 blocks a second: two seconds ahead
        let flying = approach_radius_at(40.0);
        assert!(((flying - 1) * CHUNK_SIZE) as f32 >= 40.0 * APPROACH_LEAD_SECS);
        assert_eq!(flying, 6);
        assert_eq!(approach_radius_at(1000.0), MAX_PORTAL_APPROACH_RADIUS);

        // A wider approach reaches a portal 5 chunks away
        let mut portals = Portals::default();
        pair(&mut portals, 1, IVec3::new(5, 0, 0), FAR);
        let reach = ChunkReach::new(IVec3::ZERO, WIDE, &portals);
        assert_eq!(reach.areas()[0].radius, PORTAL_VIEW_RADIUS);
        let reach = reach.approaching(flying);
        assert_eq!(reach.areas()[0].radius, WIDE);
        assert_eq!(reach.approached_twins(), vec![FAR]);
    }

    #[test]
    fn the_approach_of_a_player_follows_how_fast_it_may_go() {
        use crate::players::Player;
        let mut player = Player::new(
            1,
            "PlayerOne".into(),
            bevy::math::Vec3::ZERO,
            Default::default(),
        );
        assert_eq!(approach_radius(&player), PORTAL_APPROACH_RADIUS);
        player.is_flying = true;
        player.is_sprinting = true;
        assert_eq!(approach_radius(&player), 6);
        // Falling fast
        player.is_flying = false;
        player.velocity = bevy::math::Vec3::new(0.0, -60.0, 0.0);
        assert_eq!(approach_radius(&player), MAX_PORTAL_APPROACH_RADIUS);
    }

    fn standing_player() -> crate::players::Player {
        crate::players::Player::new(
            1,
            "PlayerOne".into(),
            bevy::math::Vec3::splat(8.0),
            Default::default(),
        )
    }

    #[test]
    fn a_jump_or_a_short_fall_does_not_change_the_reach() {
        use crate::players::constants::JUMP_VELOCITY;
        use bevy::math::Vec3;
        let still = standing_player();
        let mut near = Portals::default();
        pair(&mut near, 1, IVec3::new(3, 0, 0), FAR);
        for portals in [Portals::default(), near] {
            let reach =
                |player: &crate::players::Player| ChunkReach::of_player(player, WIDE, &portals);
            // A bunny hop at a sprint: up, then down, every frame of it
            for velocity in [
                Vec3::new(5.0, JUMP_VELOCITY, 3.0),
                Vec3::new(5.0, 8.2, 3.0),
                Vec3::new(5.0, -JUMP_VELOCITY, 3.0),
                // A fall of two blocks
                Vec3::new(0.0, -10.9, 0.0),
            ] {
                let mut moving = still.clone();
                moving.velocity = velocity;
                assert_eq!(reach(&moving), reach(&still), "{velocity}");
            }
        }
        // A long fall, with no portal near, nor a sprint in flight: nothing to approach
        let mut portals = Portals::default();
        pair(
            &mut portals,
            1,
            IVec3::new(MAX_PORTAL_APPROACH_RADIUS + 1, 0, 0),
            FAR,
        );
        let mut falling = still.clone();
        falling.velocity = Vec3::new(0.0, -60.0, 0.0);
        let mut flying = still.clone();
        flying.is_flying = true;
        flying.is_sprinting = true;
        for moving in [falling, flying] {
            assert_eq!(
                ChunkReach::of_player(&moving, WIDE + 4, &portals),
                ChunkReach::of_player(&still, WIDE + 4, &portals)
            );
        }
    }

    #[test]
    fn releasing_a_sprint_or_landing_keeps_the_area_sent() {
        use crate::constants::UNLOAD_MARGIN;
        let mut portals = Portals::default();
        pair(&mut portals, 1, IVec3::new(6, 0, 0), FAR);
        let mut player = standing_player();
        player.is_flying = true;
        player.is_sprinting = true;
        let sprint = ChunkReach::of_player(&player, WIDE + 4, &portals);
        let sent = sprint.areas();
        assert_eq!(sent[0].radius, WIDE + 4);
        // Flying, then on foot: the portal is no longer approached, but what was sent stays
        player.is_sprinting = false;
        let flying = ChunkReach::of_player(&player, WIDE + 4, &portals);
        player.is_flying = false;
        let walking = ChunkReach::of_player(&player, WIDE + 4, &portals);
        for reach in [flying, walking] {
            assert_eq!(reach.areas()[0].radius, PORTAL_VIEW_RADIUS);
            let kept = reach.kept(UNLOAD_MARGIN);
            assert!(sent[0].chunks().all(|chunk| kept.contains(chunk)));
        }
        // Past the widest approach and the margin, it goes
        let away = ChunkReach::new(
            IVec3::new(6 - MAX_PORTAL_APPROACH_RADIUS - UNLOAD_MARGIN - 1, 0, 0),
            WIDE + 4,
            &portals,
        );
        let kept = away.kept(UNLOAD_MARGIN);
        assert!(!sent[0].chunks().all(|chunk| kept.contains(chunk)));
    }

    #[test]
    fn the_capped_radius_of_a_near_twin_seldom_changes_as_the_player_moves() {
        let mut portals = Portals::default();
        pair(&mut portals, 1, IVec3::ZERO, IVec3::new(21, 0, 0));
        let radii: Vec<i32> = (-6..=6)
            .map(|x| {
                ChunkReach::new(IVec3::new(x, 0, 0), 32, &portals)
                    .approaching(MAX_PORTAL_APPROACH_RADIUS)
                    .areas()[0]
                    .radius
            })
            .collect();
        for radius in &radii {
            assert!(
                *radius == 32 || radius % CAPPED_RADIUS_STEP == 0,
                "{radii:?}"
            );
        }
        let changes = radii.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert!(changes <= 4, "{radii:?}");
    }

    #[test]
    fn stepping_to_and_fro_by_a_portal_unloads_nothing_of_its_twin() {
        use crate::constants::UNLOAD_MARGIN;
        for (distance, twin) in [(32, 21), (32, 40), (64, 50)] {
            let mut portals = Portals::default();
            pair(&mut portals, 1, IVec3::ZERO, IVec3::new(twin, 0, 0));
            let mut sent: std::collections::HashSet<IVec3> = Default::default();
            for (step, x) in [0, 1, 0, 1, 0, -1, 0, 2, 1, 0].into_iter().enumerate() {
                let reach = ChunkReach::new(IVec3::new(x, 0, 0), distance, &portals);
                let kept = reach.kept(UNLOAD_MARGIN);
                let unloaded = sent.iter().filter(|chunk| !kept.contains(**chunk)).count();
                assert_eq!(unloaded, 0, "{distance}, {twin}: step {step}, at {x}");
                sent.extend(reach.portal_chunks());
            }
        }
    }

    #[test]
    fn the_offsets_least_used_go_first() {
        let mut cache = OffsetsCache::default();
        let players = cache.get(64, IVec3::Z);
        // Many radii come and go, the player's area asked for in between
        for radius in 5..40 {
            cache.get(radius, IVec3::X);
            assert!(Arc::ptr_eq(&cache.get(64, IVec3::Z), &players));
        }
        assert!(cache.entries.len() <= OFFSETS_KEPT);
    }

    /// The chunks of `area` not within the render distance of `reach`.
    fn extra(reach: &ChunkReach, area: &TwinArea) -> usize {
        area.chunks()
            .filter(|chunk| !reach.within(*chunk, 0))
            .count()
    }

    #[test]
    fn the_area_of_an_approached_twin_adds_at_most_a_capped_count_of_chunks() {
        let mut portals = Portals::default();
        let far = IVec3::new(500, 0, 0);
        pair(&mut portals, 1, IVec3::X, far);
        // At render distances up to 12, the whole render distance
        for distance in [8, 12] {
            let reach = ChunkReach::new(IVec3::ZERO, distance, &portals);
            assert_eq!(reach.areas()[0].radius, distance, "{distance}");
        }
        // At 64, as many shells as the cap allows
        let reach = ChunkReach::new(IVec3::ZERO, 64, &portals);
        let area = reach.areas()[0];
        assert!(area.radius < 64 && area.radius >= 12, "{}", area.radius);
        assert!(extra(&reach, &area) <= MAX_APPROACHED_EXTRA_CHUNKS);
        let wider = TwinArea {
            radius: area.radius + 1,
            ..area
        };
        assert!(extra(&reach, &wider) > MAX_APPROACHED_EXTRA_CHUNKS);
        assert_eq!(reach.portal_chunks().len(), extra(&reach, &area));

        // A twin within the player's own render distance adds only what lies past it: wider
        let mut portals = Portals::default();
        pair(&mut portals, 1, IVec3::X, IVec3::new(21, 0, 0));
        let near = ChunkReach::new(IVec3::ZERO, 64, &portals);
        let near_area = near.areas()[0];
        assert!(near_area.radius > area.radius + 20, "{}", near_area.radius);
        assert!(extra(&near, &near_area) <= MAX_APPROACHED_EXTRA_CHUNKS);
        // And the whole render distance of a twin as close as the player's area allows
        let mut portals = Portals::default();
        pair(&mut portals, 1, IVec3::X, IVec3::new(0, 0, 1));
        let reach = ChunkReach::new(IVec3::ZERO, 64, &portals);
        assert_eq!(reach.areas()[0].radius, 64);
    }

    #[test]
    fn the_cap_holds_for_every_portal_approached_together() {
        let mut portals = Portals::default();
        for owner in 0..4 {
            let at = IVec3::new(owner as i32 % 2, 0, owner as i32 / 2);
            pair(
                &mut portals,
                owner,
                at,
                IVec3::new(300 + 100 * owner as i32, 0, 0),
            );
        }
        let reach = ChunkReach::new(IVec3::ZERO, 32, &portals);
        let areas = reach.areas();
        assert_eq!(areas.len(), 4);
        let added: usize = areas.iter().map(|area| extra(&reach, area)).sum();
        assert!(added <= MAX_APPROACHED_EXTRA_CHUNKS + 3 * 729, "{added}");
        // The nearest gets the most
        assert!(areas.iter().any(|area| area.radius > PORTAL_VIEW_RADIUS));
        // Kept with a margin, each keeps at least the radius it is sent
        let kept = reach.areas_within(3);
        for area in &areas {
            assert!(
                kept.iter().any(|kept| kept.twin == area.twin
                    && kept.facing == area.facing
                    && kept.radius >= area.radius),
                "{area:?}"
            );
        }
    }
}
