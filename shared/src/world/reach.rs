//! Which chunks the client of a player is sent and keeps: those within its render distance, and
//! those around the twin of every open portal within it, which the portal shows. The server
//! sends, unloads, loads and forgets chunks by this one rule; the client keeps what it is sent
//! until the server tells it to forget it.

use bevy::math::IVec3;

use crate::world::{
    chunk_in_reach, render_distance_offsets, vertical_render_distance, within_render_distance,
    Portals,
};

/// How far around the twin of an open portal the chunks are sent to the players that see the
/// portal, in chunks: as far as what the portal shows reaches (its camera's fog).
pub const PORTAL_VIEW_RADIUS: i32 = 4;

/// The reach of a player in the chunk `center` with the render distance `distance`, among the
/// portals of the world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkReach {
    pub center: IVec3,
    pub distance: i32,
    /// The chunk of each open portal (`Portal::chunk`) with the chunk of its twin, sorted.
    views: Vec<(IVec3, IVec3)>,
}

impl ChunkReach {
    pub fn new(center: IVec3, distance: i32, portals: &Portals) -> Self {
        let mut views: Vec<(IVec3, IVec3)> = portals
            .iter()
            .filter_map(|portal| Some((portal.chunk(), portals.twin(portal)?.chunk())))
            .collect();
        views.sort_by_key(|(portal, twin)| (portal.to_array(), twin.to_array()));
        Self {
            center,
            distance,
            views,
        }
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

    /// The chunks of the twins of the open portals within the render distance plus `margin`,
    /// each once.
    fn twins_within(&self, margin: i32) -> Vec<IVec3> {
        let mut twins: Vec<IVec3> = Vec::new();
        for &(portal, twin) in &self.views {
            if self.within(portal, margin) && !twins.contains(&twin) {
                twins.push(twin);
            }
        }
        twins
    }

    /// The chunks of the twins of the open portals within the render distance, each once.
    pub fn twins(&self) -> Vec<IVec3> {
        self.twins_within(0)
    }

    /// The chunks within `PORTAL_VIEW_RADIUS` of the twins (`twins`) that are not within the
    /// render distance, each once: twin after twin, nearest to it first.
    pub fn portal_chunks(&self) -> Vec<IVec3> {
        let twins = self.twins();
        let offsets = render_distance_offsets(PORTAL_VIEW_RADIUS);
        let mut chunks = Vec::new();
        for (index, &twin) in twins.iter().enumerate() {
            chunks.extend(
                offsets
                    .iter()
                    .map(|&offset| twin + offset)
                    .filter(|&chunk| {
                        !self.within(chunk, 0)
                            && !twins[..index].iter().any(|&before| {
                                within_render_distance(chunk, before, PORTAL_VIEW_RADIUS)
                            })
                    }),
            );
        }
        chunks
    }

    /// Whether a client keeps `chunk`, `margin` chunks beyond the reach: it is within the render
    /// distance plus `margin`, or within `PORTAL_VIEW_RADIUS` plus `margin` of the twin of an
    /// open portal within the render distance plus `margin`.
    pub fn keeps(&self, chunk: IVec3, margin: i32) -> bool {
        self.within(chunk, margin)
            || self.twins_within(margin).into_iter().any(|twin| {
                chunk_in_reach(
                    chunk,
                    twin,
                    PORTAL_VIEW_RADIUS + margin,
                    vertical_render_distance(PORTAL_VIEW_RADIUS) + margin,
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{FaceDirection, Portal, PortalColor};
    use crate::CHUNK_SIZE;

    const DISTANCE: i32 = 2;

    /// A portal of `owner` and `color` facing +Z, opening onto the chunk `chunk`.
    fn portal(owner: u64, color: PortalColor, chunk: IVec3) -> Portal {
        let portal = Portal {
            owner,
            color,
            anchor: chunk * CHUNK_SIZE + IVec3::new(4, 4, 4),
            face: FaceDirection::PlusZ,
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
}
