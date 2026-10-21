//! The portals of the world: openings 2 blocks wide and 3 high on any face of a block, each
//! player owning a blue one and an orange one. What lies in front of a portal is seen, and later reached, in
//! front of its twin, the portal of the other colour of the same player. They live in the
//! `ChunkMap`, so that the simulation of both sides reads them.

use bevy::math::{Affine3A, IVec3, Mat3, Vec2, Vec3};
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;
use crate::world::{
    world_position_to_chunk_position, BlockHitbox, BlockTransparency, ChunkMap, FaceDirection,
    FaceDirectionExt,
};

/// How many wall cells wide a portal is.
pub const PORTAL_WIDTH: i32 = 2;
/// How many wall cells high a portal is.
pub const PORTAL_HEIGHT: i32 = 3;
/// How far behind the plane of a portal a point is still in front of it, in blocks. A player
/// that comes out of a portal is in front of it by more than that, so that no rounding leaves it
/// behind, in the wall. Less than the inset of the portal's box (2 mm) on the client, so that
/// an eye in front never is behind the front face of that box.
pub const PORTAL_SLACK: f32 = 1e-3;

/// Append only, like `ItemId`: network messages carry the position of the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PortalColor {
    Blue,
    Orange,
}

impl PortalColor {
    /// The colour of the twin.
    pub fn other(self) -> Self {
        match self {
            PortalColor::Blue => PortalColor::Orange,
            PortalColor::Orange => PortalColor::Blue,
        }
    }
}

/// A portal on the face `face` of six wall cells, its long side along `up`. `anchor` is the
/// lower-left wall cell, seen from in front of the wall, its up at the top; the others are to
/// its right and above it. A wall, a floor or a ceiling: a portal's wall cells are those it lies
/// on, whichever way they face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Portal {
    pub owner: PlayerId,
    pub color: PortalColor,
    pub anchor: IVec3,
    pub face: FaceDirection,
    /// The direction of the long side of the opening, its up: `PlusY` on a wall, one of the four
    /// horizontal directions on a floor or a ceiling (`opening_up`).
    pub up: FaceDirection,
}

impl Portal {
    /// The direction the portal faces, out of the wall.
    pub fn normal(&self) -> Vec3 {
        self.face.to_ivec3().as_vec3()
    }

    /// The up of the portal, along its long side.
    pub fn up(&self) -> Vec3 {
        self.up.to_ivec3().as_vec3()
    }

    /// The right of a viewer facing the portal, its head toward the portal's up.
    pub fn right(&self) -> Vec3 {
        self.up().cross(self.normal())
    }

    /// The six wall cells the portal lies on.
    pub fn cells(&self) -> impl Iterator<Item = IVec3> + '_ {
        let (right, up) = (self.right().as_ivec3(), self.up.to_ivec3());
        (0..PORTAL_HEIGHT).flat_map(move |high| {
            (0..PORTAL_WIDTH).map(move |across| self.anchor + right * across + up * high)
        })
    }

    /// The six cells in front of the wall cells, which the portal opens onto.
    pub fn front_cells(&self) -> impl Iterator<Item = IVec3> + '_ {
        let normal = self.face.to_ivec3();
        self.cells().map(move |cell| cell + normal)
    }

    /// Whether the portal may stand where it is in `world`: its wall cells hold solid blocks
    /// with a full hitbox (not glass or leaves, which the box of the portal, inside the wall,
    /// would show through), and nothing collides in front of them (plants and liquids do not).
    pub fn fits(&self, world: &ChunkMap) -> bool {
        let wall = self.cells().all(|cell| {
            world.get_block(cell).is_some_and(|block| {
                matches!(block.id.get_hitbox(), BlockHitbox::FullBlock)
                    && block.id.get_visibility() == BlockTransparency::Solid
            })
        });
        wall && self.front_cells().all(|cell| {
            world
                .get_block(cell)
                .is_none_or(|block| matches!(block.id.get_hitbox(), BlockHitbox::None))
        })
    }

    /// The frame of the portal in world space: its origin is the centre of the opening, on the
    /// face of the wall, and its axes are the right, the up and the normal of the portal.
    pub fn frame(&self) -> Affine3A {
        let (right, up, normal) = (self.right(), self.up(), self.normal());
        let center = self.anchor.as_vec3()
            + Vec3::splat(0.5)
            + right * (PORTAL_WIDTH - 1) as f32 / 2.
            + up * (PORTAL_HEIGHT - 1) as f32 / 2.
            + normal * 0.5;
        Affine3A::from_mat3_translation(Mat3::from_cols(right, up, normal), center)
    }

    /// The chunk of what the portal opens onto: the chunk of the middle of its front cells.
    pub fn chunk(&self) -> IVec3 {
        let front = Vec3::from(self.frame().translation) + self.normal() * 0.5;
        world_position_to_chunk_position(front)
    }

    /// How far `point` is in front of the plane of the portal: negative behind it.
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal()
            .dot(point - Vec3::from(self.frame().translation))
    }

    /// `point` in the frame of the portal (`frame`): across to the right, up, and out of the
    /// wall, from the centre of the opening.
    pub fn local(&self, point: Vec3) -> Vec3 {
        let offset = point - Vec3::from(self.frame().translation);
        Vec3::new(
            self.right().dot(offset),
            self.up().dot(offset),
            self.normal().dot(offset),
        )
    }
}

/// The portals of the world, one of each colour per player at most, and whether they changed
/// since the last `take_changed`.
#[derive(Default, Clone, Debug)]
pub struct Portals {
    portals: HashMap<(PlayerId, PortalColor), Portal>,
    changed: bool,
}

impl Portals {
    /// Adds `portal`, replacing the one of its owner and colour: placing a colour again moves
    /// that portal.
    pub fn insert(&mut self, portal: Portal) {
        self.portals.insert((portal.owner, portal.color), portal);
        self.changed = true;
    }

    /// Removes the portal of `owner` of colour `color`, and returns it.
    pub fn remove(&mut self, owner: PlayerId, color: PortalColor) -> Option<Portal> {
        let removed = self.portals.remove(&(owner, color));
        self.changed |= removed.is_some();
        removed
    }

    /// Replaces every portal by `portals`, as a client does with those the server sends.
    pub fn replace(&mut self, portals: Vec<Portal>) {
        self.portals = portals
            .into_iter()
            .map(|portal| ((portal.owner, portal.color), portal))
            .collect();
        self.changed = true;
    }

    /// Records a change without changing anything, so that the portals are sent again. The
    /// server uses it when it refuses a shot that a client may have predicted.
    pub fn touch(&mut self) {
        self.changed = true;
    }

    /// Whether the portals changed since the previous call.
    pub fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn get(&self, owner: PlayerId, color: PortalColor) -> Option<&Portal> {
        self.portals.get(&(owner, color))
    }

    /// The portals, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &Portal> + '_ {
        self.portals.values()
    }

    pub fn is_empty(&self) -> bool {
        self.portals.is_empty()
    }

    /// The portal of the other colour of the owner of `portal`: a portal is open when it has
    /// one.
    pub fn twin(&self, portal: &Portal) -> Option<&Portal> {
        self.get(portal.owner, portal.color.other())
    }

    /// The transform from the space in front of `portal` to the space in front of its twin,
    /// `None` when it has none. A point going into `portal`, from its front to its back, comes
    /// out of the twin, from its back to its front, turned as the twin is: in the frames of the
    /// portals, it is turned half round their up. A rigid transform, whatever the faces: from a
    /// floor to a wall, what falls into the floor comes out of the wall.
    pub fn through(&self, portal: &Portal) -> Option<Affine3A> {
        let twin = self.twin(portal)?;
        Some(
            twin.frame()
                * Affine3A::from_rotation_y(std::f32::consts::PI)
                * portal.frame().inverse(),
        )
    }

    /// The portals that have a twin, which let players through.
    fn open(&self) -> impl Iterator<Item = &Portal> + '_ {
        self.iter().filter(|portal| self.twin(portal).is_some())
    }

    /// The open portals whose wall lets in a player centred on `center`: it is in front of them,
    /// within the slack, and in front of their opening shrunk by `margin` on every side, half
    /// the width of the player, so that its box fits in the opening.
    pub fn letting_through(&self, center: Vec3, margin: f32) -> impl Iterator<Item = &Portal> {
        let half = Vec3::new(PORTAL_WIDTH as f32, PORTAL_HEIGHT as f32, 0.0) / 2.0 - margin;
        self.open().filter(move |portal| {
            let local = portal.local(center);
            local.z >= -PORTAL_SLACK && local.x.abs() <= half.x && local.y.abs() <= half.y
        })
    }

    /// The open portal a point going from `from` to `to` goes into: the segment goes from in
    /// front of its plane to behind it (`PORTAL_SLACK`), through its opening. The first one
    /// along the segment, if several.
    pub fn crossed(&self, from: Vec3, to: Vec3) -> Option<&Portal> {
        let half = Vec2::new(PORTAL_WIDTH as f32, PORTAL_HEIGHT as f32) / 2.0;
        self.open()
            .filter_map(|portal| {
                let (from, to) = (portal.local(from), portal.local(to));
                if from.z < -PORTAL_SLACK || to.z >= -PORTAL_SLACK {
                    return None;
                }
                // Where the segment meets the plane
                let along = (from.z / (from.z - to.z)).clamp(0.0, 1.0);
                let hit = from.lerp(to, along);
                (hit.x.abs() <= half.x && hit.y.abs() <= half.y).then_some((along, portal))
            })
            .min_by(|(a, _), (b, _)| a.total_cmp(b))
            .map(|(_, portal)| portal)
    }

    /// The wall cells of the open portals that let in an entity centred on `center`, of half
    /// width `margin` (`letting_through`): they do not collide with it, so that it goes into
    /// them, and through the portal.
    pub fn passable_cells(&self, center: Vec3, margin: f32) -> Vec<IVec3> {
        self.letting_through(center, margin)
            .flat_map(Portal::cells)
            .collect()
    }

    /// Whether the centre of an entity of half width `margin`, moving from `from` to `to`,
    /// goes into an open portal that lets it through (`letting_through`): the move is then made
    /// whatever lies behind the wall cells, as a floor thicker than a block does under a floor
    /// portal, and is the crossing.
    pub fn goes_into(&self, from: Vec3, to: Vec3, margin: f32) -> bool {
        self.crossed(from, to).is_some_and(|crossed| {
            self.letting_through(from, margin)
                .any(|open| open == crossed)
        })
    }

    /// Takes through the open portal it went into (`crossed`) a centre that moved from `from` to
    /// `to`, of a box of half extents `extents`: the transform of the pair (`through`) and where
    /// the centre comes out. Out of the twin, the centre is kept within its opening shrunk by
    /// the half extents of the box, so that the box fits the opening: a box falling into a
    /// floor portal near one end comes out of a wall portal within the opening, not in the wall
    /// under it. A box that went into a portal fitted its opening, which the same shape on a
    /// wall keeps it doing.
    pub fn pass(&self, from: Vec3, to: Vec3, extents: Vec3) -> Option<Passage> {
        let portal = self.crossed(from, to)?;
        let (through, twin) = (self.through(portal)?, self.twin(portal)?);
        let room =
            |side: Vec3, length: i32| (length as f32 / 2.0 - side.abs().dot(extents)).max(0.0);
        let (across, high) = (
            room(twin.right(), PORTAL_WIDTH),
            room(twin.up(), PORTAL_HEIGHT),
        );
        let out = twin.local(through.transform_point3(to));
        let out = Vec3::new(
            out.x.clamp(-across, across),
            out.y.clamp(-high, high),
            out.z,
        );
        Some(Passage {
            through,
            position: twin.frame().transform_point3(out),
        })
    }

    /// Whether an open portal lies on the face `face` of the block at `cell`: there the view
    /// through it is seen, not the block.
    pub fn opens(&self, cell: IVec3, face: FaceDirection) -> bool {
        self.open()
            .any(|portal| portal.face == face && portal.cells().any(|wall| wall == cell))
    }

    /// Whether `portal` shares a wall cell with another portal, but the one of its owner and
    /// colour, which it would replace, and one on the opposite face of the cell: a portal's box
    /// fills the half of its cells behind its face (`client/src/portal/`), so two portals
    /// through a wall one block thick keep apart, but two on faces at a corner would cut into
    /// each other.
    pub fn overlaps(&self, portal: &Portal) -> bool {
        self.portals.values().any(|other| {
            (other.owner, other.color) != (portal.owner, portal.color)
                && other.normal() != -portal.normal()
                && other
                    .cells()
                    .any(|cell| portal.cells().any(|mine| mine == cell))
        })
    }

    /// Whether a portal lies on the face `face` of the block at `cell`: that face is not drawn,
    /// as the portal shows there.
    pub fn covers(&self, cell: IVec3, face: FaceDirection) -> bool {
        self.portals
            .values()
            .any(|portal| portal.face == face && portal.cells().any(|wall| wall == cell))
    }
}

/// An entity's centre gone through a portal (`Portals::pass`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Passage {
    /// The transform of the pair, for what else goes through with the centre: its velocity,
    /// where it looks.
    pub through: Affine3A,
    /// Where the centre comes out, in front of the twin.
    pub position: Vec3,
}

/// Which way the long side of a portal shot along `direction` onto the face `face` goes: up on a
/// wall; on a floor or a ceiling, away from the shooter, along the horizontal axis the shot goes
/// most along, as a wall tipped over forwards.
pub fn opening_up(face: FaceDirection, direction: Vec3) -> FaceDirection {
    if !matches!(face, FaceDirection::PlusY | FaceDirection::MinusY) {
        return FaceDirection::PlusY;
    }
    match (direction.x.abs() > direction.z.abs(), direction) {
        (true, d) if d.x > 0.0 => FaceDirection::PlusX,
        (true, _) => FaceDirection::MinusX,
        (false, d) if d.z > 0.0 => FaceDirection::PlusZ,
        (false, _) => FaceDirection::MinusZ,
    }
}

/// The portal of `owner` and `color` that a shot along `direction` hitting the face `face` of
/// the cell `aimed` at the point `hit` places, if any, its long side along `opening_up`. Of the
/// six placements on that face that hold the aimed cell, it is the first that fits
/// (`Portal::fits`) and overlaps no other portal, by the distance of the centre of its opening
/// to `hit`.
#[allow(clippy::too_many_arguments)]
pub fn fit_portal(
    world: &ChunkMap,
    portals: &Portals,
    owner: PlayerId,
    color: PortalColor,
    aimed: IVec3,
    face: FaceDirection,
    hit: Vec3,
    direction: Vec3,
) -> Option<Portal> {
    let placement = |across: i32, high: i32| {
        let mut portal = Portal {
            owner,
            color,
            anchor: aimed,
            face,
            up: opening_up(face, direction),
        };
        portal.anchor -= portal.right().as_ivec3() * across + portal.up.to_ivec3() * high;
        portal
    };
    let mut placements: Vec<Portal> = (0..PORTAL_WIDTH)
        .flat_map(|across| (0..PORTAL_HEIGHT).map(move |high| placement(across, high)))
        .collect();
    let distance = |portal: &Portal| hit.distance_squared(portal.frame().translation.into());
    // A stable sort: the order of the anchors breaks ties
    placements.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    placements
        .into_iter()
        .find(|portal| portal.fits(world) && !portals.overlaps(portal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::{BlockData, BlockDirection, BlockId};

    fn portal(color: PortalColor, anchor: IVec3, face: FaceDirection) -> Portal {
        Portal {
            owner: 7,
            color,
            anchor,
            face,
            up: FaceDirection::PlusY,
        }
    }

    fn assert_near(a: Vec3, b: Vec3) {
        assert!(a.distance(b) < 1e-4, "{a} is not {b}");
    }

    /// A blue portal on a wall facing +Z, and an orange one on a wall facing -X.
    fn pair() -> Portals {
        let mut portals = Portals::default();
        portals.insert(portal(
            PortalColor::Blue,
            IVec3::new(0, 10, 0),
            FaceDirection::PlusZ,
        ));
        portals.insert(portal(
            PortalColor::Orange,
            IVec3::new(20, 10, 5),
            FaceDirection::MinusX,
        ));
        portals
    }

    #[test]
    fn the_wall_cells_go_right_and_up_from_the_anchor() {
        let facing_z = portal(PortalColor::Blue, IVec3::ZERO, FaceDirection::PlusZ);
        let cells: Vec<_> = facing_z.cells().collect();
        assert_eq!(cells.len(), 6);
        assert!(cells.contains(&IVec3::new(1, 2, 0)));
        // A viewer facing a wall that faces -X has +Z on its right
        let facing_minus_x = portal(PortalColor::Blue, IVec3::ZERO, FaceDirection::MinusX);
        assert!(facing_minus_x
            .cells()
            .any(|cell| cell == IVec3::new(0, 2, 1)));
        let facing_x = portal(PortalColor::Blue, IVec3::ZERO, FaceDirection::PlusX);
        assert!(facing_x.cells().any(|cell| cell == IVec3::new(0, 2, -1)));
    }

    #[test]
    fn the_frame_is_at_the_centre_of_the_opening_facing_out() {
        let frame = portal(
            PortalColor::Blue,
            IVec3::new(0, 10, 0),
            FaceDirection::PlusZ,
        )
        .frame();
        assert_near(frame.translation.into(), Vec3::new(1.0, 11.5, 1.0));
        assert_near(frame.transform_vector3(Vec3::Z), Vec3::Z);
        assert_near(frame.transform_vector3(Vec3::X), Vec3::X);

        let frame = portal(
            PortalColor::Blue,
            IVec3::new(20, 10, 5),
            FaceDirection::MinusX,
        )
        .frame();
        assert_near(frame.translation.into(), Vec3::new(20.0, 11.5, 6.0));
        assert_near(frame.transform_vector3(Vec3::Z), Vec3::NEG_X);
    }

    #[test]
    fn the_twin_is_the_other_colour_of_the_same_owner() {
        let portals = pair();
        let blue = portals.get(7, PortalColor::Blue).unwrap();
        assert_eq!(portals.twin(blue).unwrap().color, PortalColor::Orange);
        // The orange portal of another player is not a twin
        let mut portals = Portals::default();
        portals.insert(*blue);
        portals.insert(Portal {
            owner: 8,
            ..portal(PortalColor::Orange, IVec3::ZERO, FaceDirection::PlusX)
        });
        assert!(portals.twin(blue).is_none());
    }

    #[test]
    fn a_lone_portal_has_no_twin() {
        let mut portals = pair();
        portals.remove(7, PortalColor::Orange).unwrap();
        let blue = portals.get(7, PortalColor::Blue).unwrap();
        assert!(portals.twin(blue).is_none());
        assert!(portals.through(blue).is_none());
    }

    #[test]
    fn placing_a_colour_again_moves_it() {
        let mut portals = pair();
        portals.insert(portal(
            PortalColor::Blue,
            IVec3::new(3, 4, 5),
            FaceDirection::PlusX,
        ));
        assert_eq!(portals.iter().count(), 2);
        assert_eq!(
            portals.get(7, PortalColor::Blue).unwrap().anchor,
            IVec3::new(3, 4, 5)
        );
    }

    #[test]
    fn through_takes_the_front_of_a_portal_to_the_back_of_its_twin() {
        let portals = pair();
        let blue = portals.get(7, PortalColor::Blue).unwrap();
        let through = portals.through(blue).unwrap();
        // The centres meet
        assert_near(
            through.transform_point3(Vec3::new(1.0, 11.5, 1.0)),
            Vec3::new(20.0, 11.5, 6.0),
        );
        // Two blocks in front of the blue portal, a block to its right: behind the orange one,
        // two blocks into its wall, a block to its left
        assert_near(
            through.transform_point3(Vec3::new(2.0, 11.5, 3.0)),
            Vec3::new(22.0, 11.5, 5.0),
        );
    }

    #[test]
    fn through_turns_a_direction_into_the_portal_into_one_out_of_the_twin() {
        let portals = pair();
        let through = portals
            .through(portals.get(7, PortalColor::Blue).unwrap())
            .unwrap();
        // Into the blue portal is -Z, out of the orange one is -X
        assert_near(through.transform_vector3(Vec3::NEG_Z), Vec3::NEG_X);
        assert_near(through.transform_vector3(Vec3::Y), Vec3::Y);
    }

    #[test]
    fn through_and_back_is_the_identity() {
        let portals = pair();
        let there = portals
            .through(portals.get(7, PortalColor::Blue).unwrap())
            .unwrap();
        let back = portals
            .through(portals.get(7, PortalColor::Orange).unwrap())
            .unwrap();
        let point = Vec3::new(-3.0, 12.25, 4.5);
        assert_near((back * there).transform_point3(point), point);
    }

    #[test]
    fn a_portal_covers_the_front_faces_of_its_wall_cells_only() {
        let portals = pair();
        assert!(portals.covers(IVec3::new(1, 12, 0), FaceDirection::PlusZ));
        assert!(!portals.covers(IVec3::new(1, 12, 0), FaceDirection::MinusZ));
        assert!(!portals.covers(IVec3::new(2, 12, 0), FaceDirection::PlusZ));
        assert!(!portals.covers(IVec3::new(1, 13, 0), FaceDirection::PlusZ));
    }

    #[test]
    fn a_point_in_front_of_the_frame_is_let_through_only_within_it_shrunk() {
        let portals = pair();
        let letting = |point: Vec3| {
            portals
                .letting_through(point, 0.4)
                .map(|portal| portal.color)
                .collect::<Vec<_>>()
        };
        // The blue portal's opening is x in 0..2, y in 10..13, its plane z = 1
        assert_eq!(letting(Vec3::new(1.5, 10.9, 1.3)), [PortalColor::Blue]);
        assert_eq!(letting(Vec3::new(1.55, 12.55, 4.0)), [PortalColor::Blue]);
        // A hair behind the plane is still in front
        assert_eq!(letting(Vec3::new(1.0, 11.5, 0.9995)), [PortalColor::Blue]);
        // Too near an edge for the box of the player to fit in the opening
        assert!(letting(Vec3::new(1.7, 11.5, 1.3)).is_empty());
        assert!(letting(Vec3::new(1.0, 10.3, 1.3)).is_empty());
        assert!(letting(Vec3::new(1.0, 12.7, 1.3)).is_empty());
        // Behind the plane
        assert!(letting(Vec3::new(1.0, 11.5, 0.9)).is_empty());
        // The orange one, facing -X
        assert_eq!(letting(Vec3::new(19.5, 11.0, 6.5)), [PortalColor::Orange]);
    }

    #[test]
    fn the_wall_cells_of_a_portal_that_lets_an_entity_through_are_passable() {
        let portals = pair();
        let blue = *portals.get(7, PortalColor::Blue).unwrap();
        let mut cells = portals.passable_cells(Vec3::new(1.0, 11.0, 1.2), 0.3);
        cells.sort_by_key(|cell| cell.to_array());
        let mut wall: Vec<IVec3> = blue.cells().collect();
        wall.sort_by_key(|cell| cell.to_array());
        assert_eq!(cells, wall);
        // Too wide for where it is, behind the plane, or the twin gone: nothing
        assert!(portals
            .passable_cells(Vec3::new(0.3, 11.0, 1.2), 0.9)
            .is_empty());
        assert!(portals
            .passable_cells(Vec3::new(1.0, 11.0, 0.5), 0.3)
            .is_empty());
        let mut closed = portals.clone();
        closed.remove(7, PortalColor::Orange);
        assert!(closed
            .passable_cells(Vec3::new(1.0, 11.0, 1.2), 0.3)
            .is_empty());
    }

    #[test]
    fn a_centre_goes_into_a_portal_that_lets_its_width_through() {
        let portals = pair();
        let (from, to) = (Vec3::new(1.0, 11.0, 1.1), Vec3::new(1.0, 11.0, 0.9));
        assert!(portals.goes_into(from, to, 0.3));
        // Into the opening, but too wide for where it crosses; narrower, it goes in
        let (from, to) = (Vec3::new(0.2, 11.0, 1.1), Vec3::new(0.2, 11.0, 0.9));
        assert!(!portals.goes_into(from, to, 0.3));
        assert!(portals.goes_into(from, to, 0.1));
        // Staying in front
        assert!(!portals.goes_into(Vec3::new(1.0, 11.0, 1.5), Vec3::new(1.0, 11.0, 1.1), 0.1));
    }

    #[test]
    fn a_centre_that_crossed_passes_out_of_the_twin_within_its_opening() {
        let portals = pair();
        let blue = portals.get(7, PortalColor::Blue).unwrap();
        let through = portals.through(blue).unwrap();
        let extents = Vec3::new(0.3, 0.9, 0.3);
        let (from, to) = (Vec3::new(1.2, 11.0, 1.1), Vec3::new(1.2, 11.0, 0.9));
        let passage = portals.pass(from, to, extents).unwrap();
        assert_eq!(passage.through, through);
        assert_near(passage.position, through.transform_point3(to));
        assert!(portals
            .pass(from, Vec3::new(1.2, 11.0, 1.05), extents)
            .is_none());

        // Into a floor near its north end, out of the wall's opening with the box in it: its
        // centre no higher than 1.5 - 0.9 above the middle of the opening
        let portals = floor_and_wall();
        let (from, to) = (Vec3::new(1.0, 11.1, -1.9), Vec3::new(1.0, 10.9, -1.9));
        let passage = portals.pass(from, to, extents).unwrap();
        assert_near(passage.position, Vec3::new(21.0, 12.1, 5.9 + 0.2));
    }

    #[test]
    fn a_closed_portal_lets_nothing_through() {
        let mut portals = pair();
        portals.remove(7, PortalColor::Orange);
        assert_eq!(
            portals
                .letting_through(Vec3::new(1.0, 11.5, 1.3), 0.4)
                .count(),
            0
        );
        assert!(portals
            .crossed(Vec3::new(1.0, 11.5, 1.1), Vec3::new(1.0, 11.5, 0.9))
            .is_none());
    }

    #[test]
    fn a_segment_into_the_frame_crosses_the_portal() {
        let portals = pair();
        let crossed = portals.crossed(Vec3::new(1.5, 11.0, 1.1), Vec3::new(1.4, 11.0, 0.9));
        assert_eq!(crossed.map(|portal| portal.color), Some(PortalColor::Blue));
        // From just in front, as a player that came out of the twin is, to just behind
        let crossed = portals.crossed(Vec3::new(1.0, 11.5, 1.0), Vec3::new(1.0, 11.5, 0.998));
        assert_eq!(crossed.map(|portal| portal.color), Some(PortalColor::Blue));
        // Into the corner of the opening, the ends of the segment outside it
        let crossed = portals.crossed(Vec3::new(2.1, 10.0, 1.2), Vec3::new(1.7, 10.2, 0.8));
        assert_eq!(crossed.map(|portal| portal.color), Some(PortalColor::Blue));
        let crossed = portals.crossed(Vec3::new(19.9, 11.0, 5.5), Vec3::new(20.1, 11.0, 5.5));
        assert_eq!(
            crossed.map(|portal| portal.color),
            Some(PortalColor::Orange)
        );
    }

    #[test]
    fn a_segment_outside_the_frame_from_behind_or_staying_in_front_crosses_nothing() {
        let portals = pair();
        for (from, to) in [
            // Beside the opening, above it
            (Vec3::new(2.2, 11.0, 1.1), Vec3::new(2.2, 11.0, 0.9)),
            (Vec3::new(1.0, 13.1, 1.1), Vec3::new(1.0, 13.1, 0.9)),
            // From behind
            (Vec3::new(1.0, 11.5, 0.9), Vec3::new(1.0, 11.5, 1.1)),
            // In front all along, the second end within the slack
            (Vec3::new(1.0, 11.5, 1.5), Vec3::new(1.0, 11.5, 1.2)),
            (Vec3::new(1.0, 11.5, 1.1), Vec3::new(1.0, 11.5, 0.9995)),
            // Behind all along
            (Vec3::new(1.0, 11.5, 0.9), Vec3::new(1.0, 11.5, 0.5)),
        ] {
            assert!(portals.crossed(from, to).is_none(), "{from} to {to}");
        }
    }

    #[test]
    fn an_open_portal_opens_its_face_only() {
        let mut portals = pair();
        assert!(portals.opens(IVec3::new(1, 12, 0), FaceDirection::PlusZ));
        assert!(!portals.opens(IVec3::new(1, 12, 0), FaceDirection::MinusZ));
        assert!(!portals.opens(IVec3::new(2, 12, 0), FaceDirection::PlusZ));
        portals.remove(7, PortalColor::Orange);
        assert!(!portals.opens(IVec3::new(1, 12, 0), FaceDirection::PlusZ));
    }

    #[test]
    fn inserting_removing_or_touching_the_portals_is_a_change() {
        let mut portals = Portals::default();
        assert!(!portals.take_changed());
        portals.insert(portal(PortalColor::Blue, IVec3::ZERO, FaceDirection::PlusX));
        assert!(portals.take_changed());
        assert!(!portals.take_changed());
        // Removing what is not there changes nothing
        portals.remove(7, PortalColor::Orange);
        assert!(!portals.take_changed());
        portals.remove(7, PortalColor::Blue);
        assert!(portals.take_changed());
        portals.touch();
        assert!(portals.take_changed());
    }

    #[test]
    fn replacing_the_portals_keeps_only_the_new_ones() {
        let mut portals = pair();
        portals.take_changed();
        let moved = portal(PortalColor::Blue, IVec3::new(3, 4, 5), FaceDirection::PlusX);
        portals.replace(vec![moved]);
        assert_eq!(portals.iter().collect::<Vec<_>>(), [&moved]);
        assert!(portals.take_changed());
    }

    #[test]
    fn the_distance_to_a_portal_is_negative_behind_it() {
        let blue = portal(
            PortalColor::Blue,
            IVec3::new(0, 10, 0),
            FaceDirection::PlusZ,
        );
        assert!((blue.signed_distance(Vec3::new(5.0, 0.0, 3.0)) - 2.0).abs() < 1e-5);
        assert!(blue.signed_distance(Vec3::new(1.0, 11.0, 0.5)) < 0.0);
    }

    /// A stone wall 4 wide and 4 high, facing +Z: x in 0..=3, y in 10..=13, z = 0.
    fn wall() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-4, 0, -4), IVec3::new(8, 20, 8));
        world.fill_box(IVec3::new(0, 10, 0), IVec3::new(3, 13, 0), BlockId::Stone);
        world
    }

    /// The anchor of the blue portal of player 7 fitted on the wall at `aimed`, facing +Z, the
    /// ray hitting the face at `(x, y)`.
    fn fitted(world: &ChunkMap, portals: &Portals, aimed: IVec3, x: f32, y: f32) -> Option<IVec3> {
        let hit = Vec3::new(x, y, 1.0);
        fit_portal(
            world,
            portals,
            7,
            PortalColor::Blue,
            aimed,
            FaceDirection::PlusZ,
            hit,
            Vec3::NEG_Z,
        )
        .map(|portal| portal.anchor)
    }

    #[test]
    fn the_fit_is_the_placement_nearest_to_the_hit_point() {
        let world = wall();
        let none = Portals::default();
        let aimed = IVec3::new(1, 11, 0);
        assert_eq!(
            fitted(&world, &none, aimed, 1.9, 11.9),
            Some(IVec3::new(1, 10, 0))
        );
        assert_eq!(
            fitted(&world, &none, aimed, 1.1, 11.1),
            Some(IVec3::new(0, 10, 0))
        );
        assert_eq!(
            fitted(&world, &none, aimed, 1.9, 11.95),
            Some(IVec3::new(1, 10, 0))
        );
        // At the top of the wall, only the placements going down from the aimed cell fit
        let top = IVec3::new(2, 13, 0);
        assert_eq!(
            fitted(&world, &none, top, 2.6, 13.9),
            Some(IVec3::new(2, 11, 0))
        );
        let portal = fit_portal(
            &world,
            &none,
            7,
            PortalColor::Orange,
            aimed,
            FaceDirection::PlusZ,
            Vec3::new(1.9, 11.9, 1.0),
            Vec3::NEG_Z,
        )
        .unwrap();
        assert_eq!((portal.owner, portal.color), (7, PortalColor::Orange));
        assert_eq!(portal.face, FaceDirection::PlusZ);
    }

    #[test]
    fn a_placement_whose_wall_is_not_solid_full_blocks_gives_way_to_the_next() {
        let none = Portals::default();
        let aimed = IVec3::new(1, 11, 0);
        for hole in [
            None,
            Some(BlockId::Glass),
            Some(BlockId::OakLeaves),
            Some(BlockId::Poppy),
        ] {
            let mut world = wall();
            let cell = IVec3::new(2, 12, 0);
            match hole {
                Some(id) => world.fill_box(cell, cell, id),
                None => {
                    world.remove_block(cell);
                }
            }
            // The two nearest placements hold (2, 12, 0), the third does not
            assert_eq!(
                fitted(&world, &none, aimed, 1.9, 11.9),
                Some(IVec3::new(0, 10, 0)),
                "{hole:?}"
            );
        }
    }

    #[test]
    fn a_placement_whose_front_collides_gives_way_to_the_next() {
        let none = Portals::default();
        let aimed = IVec3::new(1, 11, 0);
        let front = IVec3::new(2, 12, 1);
        let mut world = wall();
        world.fill_box(front, front, BlockId::Dirt);
        assert_eq!(
            fitted(&world, &none, aimed, 1.9, 11.9),
            Some(IVec3::new(0, 10, 0))
        );
        // Plants and water let the portal open onto them
        for free in [BlockId::TallGrass, BlockId::Water] {
            let mut world = wall();
            world.fill_box(front, front, free);
            assert_eq!(
                fitted(&world, &none, aimed, 1.9, 11.9),
                Some(IVec3::new(1, 10, 0)),
                "{free:?}"
            );
        }
    }

    #[test]
    fn the_jungle_wood_holds_a_portal_but_not_its_leaves_vines_ferns_or_pods() {
        let portal = portal(
            PortalColor::Blue,
            IVec3::new(1, 10, 0),
            FaceDirection::PlusZ,
        );
        let cell = IVec3::new(2, 12, 0);
        for (id, holds) in [
            (BlockId::JungleLog, true),
            (BlockId::JunglePlanks, true),
            (BlockId::JungleLeaves, false),
            (BlockId::Vine, false),
            (BlockId::Fern, false),
            (BlockId::Cocoa, false),
        ] {
            let mut world = wall();
            world.fill_box(cell, cell, id);
            assert_eq!(portal.fits(&world), holds, "{id:?}");
        }
    }

    #[test]
    fn a_vine_or_a_fern_in_front_lets_a_portal_open_but_a_cocoa_pod_does_not() {
        let portal = portal(
            PortalColor::Blue,
            IVec3::new(1, 10, 0),
            FaceDirection::PlusZ,
        );
        let front = IVec3::new(2, 12, 1);
        // Each hangs on the wall behind it, -Z of its cell, as on a jungle tree
        for (id, free) in [
            (BlockId::Vine, true),
            (BlockId::Fern, true),
            (BlockId::Cocoa, false),
        ] {
            let mut world = wall();
            world.set_block(front, BlockData::new(id, BlockDirection::Front));
            assert_eq!(portal.fits(&world), free, "{id:?}");
        }
    }

    #[test]
    fn a_furnace_a_chest_and_the_new_ores_and_metal_blocks_hold_a_portal() {
        let portal = portal(
            PortalColor::Blue,
            IVec3::new(1, 10, 0),
            FaceDirection::PlusZ,
        );
        let cell = IVec3::new(2, 12, 0);
        // Solid full blocks, whichever way a furnace faces: the fit rule takes them
        for id in [
            BlockId::Furnace,
            BlockId::LitFurnace,
            BlockId::Chest,
            BlockId::DiamondOre,
            BlockId::IronBlock,
            BlockId::GoldBlock,
            BlockId::DiamondBlock,
        ] {
            for direction in [
                BlockDirection::Front,
                BlockDirection::Back,
                BlockDirection::Left,
            ] {
                let mut world = wall();
                world.set_block(cell, BlockData::new(id, direction));
                assert!(portal.fits(&world), "{id:?} {direction:?}");
            }
        }
    }

    #[test]
    fn a_furnace_or_a_chest_in_front_keeps_a_portal_closed() {
        let portal = portal(
            PortalColor::Blue,
            IVec3::new(1, 10, 0),
            FaceDirection::PlusZ,
        );
        let front = IVec3::new(2, 12, 1);
        for id in [BlockId::Furnace, BlockId::LitFurnace, BlockId::Chest] {
            let mut world = wall();
            world.set_block(front, BlockData::new(id, BlockDirection::Back));
            assert!(!portal.fits(&world), "{id:?}");
        }
    }

    #[test]
    fn a_portal_sharing_a_wall_cell_on_another_face_overlaps_unless_on_the_opposite_face() {
        // Orange of player 8 on the +Z face of the cells x 2 and 3 of a wall one block thick
        let mut portals = Portals::default();
        portals.insert(Portal {
            owner: 8,
            color: PortalColor::Orange,
            anchor: IVec3::new(2, 10, 0),
            face: FaceDirection::PlusZ,
            up: FaceDirection::PlusY,
        });
        let blue = |anchor, face| Portal {
            owner: 7,
            color: PortalColor::Blue,
            anchor,
            face,
            up: FaceDirection::PlusY,
        };
        // On the -Z face of the same cells, through the wall: its box keeps to the back half
        assert!(!portals.overlaps(&blue(IVec3::new(3, 10, 0), FaceDirection::MinusZ)));
        // On the +X face of the corner cell (3, 10, 0): its box would cut into orange's
        let corner = blue(IVec3::new(3, 10, 1), FaceDirection::PlusX);
        assert!(corner.cells().any(|cell| cell == IVec3::new(3, 10, 0)));
        assert!(portals.overlaps(&corner));
        // On the +X face of the cells beside, none of orange's
        assert!(!portals.overlaps(&blue(IVec3::new(4, 10, 1), FaceDirection::PlusX)));
    }

    #[test]
    fn a_portal_of_another_colour_or_player_on_the_face_is_not_overlapped() {
        let world = wall();
        let aimed = IVec3::new(1, 11, 0);
        let in_the_way = |owner, color| {
            let mut portals = Portals::default();
            portals.insert(Portal {
                owner,
                color,
                anchor: IVec3::new(2, 10, 0),
                face: FaceDirection::PlusZ,
                up: FaceDirection::PlusY,
            });
            portals
        };
        // Their own orange portal, the blue portal of another player: the nearest placement
        // left of them is the only one
        for portals in [
            in_the_way(7, PortalColor::Orange),
            in_the_way(8, PortalColor::Blue),
        ] {
            assert_eq!(
                fitted(&world, &portals, aimed, 1.9, 11.9),
                Some(IVec3::new(0, 10, 0))
            );
        }
        // Their own blue portal is moved, wherever it was
        let own = in_the_way(7, PortalColor::Blue);
        assert_eq!(
            fitted(&world, &own, aimed, 1.9, 11.9),
            Some(IVec3::new(1, 10, 0))
        );
        // A portal on the other face of the same cells is no overlap
        let mut behind = Portals::default();
        behind.insert(Portal {
            owner: 8,
            color: PortalColor::Blue,
            anchor: IVec3::new(2, 10, 0),
            face: FaceDirection::MinusZ,
            up: FaceDirection::PlusY,
        });
        assert_eq!(
            fitted(&world, &behind, aimed, 1.9, 11.9),
            Some(IVec3::new(1, 10, 0))
        );
    }

    #[test]
    fn nothing_fits_a_wall_too_narrow_or_on_top_of_a_thin_one() {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-4, 0, -4), IVec3::new(8, 20, 8));
        world.fill_box(IVec3::new(0, 10, 0), IVec3::new(0, 13, 0), BlockId::Stone);
        let none = Portals::default();
        assert_eq!(fitted(&world, &none, IVec3::new(0, 11, 0), 0.5, 11.5), None);
        let world = wall();
        let floor = fit_portal(
            &world,
            &none,
            7,
            PortalColor::Blue,
            IVec3::new(1, 13, 0),
            FaceDirection::PlusY,
            Vec3::new(1.5, 14.0, 0.5),
            Vec3::new(1.0, -1.0, 0.0),
        );
        // Three cells long along the wall, but one across
        assert_eq!(floor, None);
    }

    /// A floor portal of player 7, of colour `color`, at `anchor`, its long side along `up`.
    fn on_floor(color: PortalColor, anchor: IVec3, up: FaceDirection) -> Portal {
        Portal {
            up,
            ..portal(color, anchor, FaceDirection::PlusY)
        }
    }

    #[test]
    fn the_opening_of_a_floor_or_ceiling_portal_lies_along_the_shot() {
        use FaceDirection::*;
        // Walls stand upright, whatever the shot
        for face in [PlusX, MinusX, PlusZ, MinusZ] {
            assert_eq!(opening_up(face, Vec3::new(0.3, -0.9, -0.2)), PlusY);
        }
        // A floor or a ceiling: the horizontal direction the shot goes most along
        for face in [PlusY, MinusY] {
            for (direction, up) in [
                (Vec3::new(0.2, -0.9, -0.4), MinusZ),
                (Vec3::new(-0.2, 0.9, 0.4), PlusZ),
                (Vec3::new(0.5, -0.9, -0.4), PlusX),
                (Vec3::new(-0.5, 0.1, 0.4), MinusX),
            ] {
                assert_eq!(opening_up(face, direction), up, "{face:?} {direction}");
            }
        }
    }

    #[test]
    fn the_cells_of_a_floor_portal_go_right_and_along_its_up() {
        // Up north: a viewer looking north and down at it has east on its right
        let north = on_floor(PortalColor::Blue, IVec3::ZERO, FaceDirection::MinusZ);
        assert_near(north.right(), Vec3::X);
        let cells: Vec<_> = north.cells().collect();
        assert_eq!(cells.len(), 6);
        for cell in [
            IVec3::new(1, 0, 0),
            IVec3::new(0, 0, -2),
            IVec3::new(1, 0, -2),
        ] {
            assert!(cells.contains(&cell), "{cell}");
        }
        assert!(north.front_cells().all(|cell| cell.y == 1));
        // Its frame: centred on the top of the six cells, up north, facing up
        let frame = north.frame();
        assert_near(frame.translation.into(), Vec3::new(1.0, 1.0, -0.5));
        assert_near(frame.transform_vector3(Vec3::Y), Vec3::NEG_Z);
        assert_near(frame.transform_vector3(Vec3::Z), Vec3::Y);
        assert_near(frame.transform_vector3(Vec3::X), Vec3::X);
        // On a ceiling, up east: facing down, its right is north
        let ceiling = Portal {
            up: FaceDirection::PlusX,
            ..portal(PortalColor::Blue, IVec3::ZERO, FaceDirection::MinusY)
        };
        assert_near(ceiling.right(), Vec3::X.cross(Vec3::NEG_Y));
        assert!(ceiling.cells().any(|cell| cell == IVec3::new(2, 0, -1)));
        assert!(ceiling.front_cells().all(|cell| cell.y == -1));
        assert_near(ceiling.frame().translation.into(), Vec3::new(1.5, 0.0, 0.0));
    }

    /// A blue portal on a floor, its up north, and an orange one on a wall facing +Z.
    fn floor_and_wall() -> Portals {
        let mut portals = Portals::default();
        portals.insert(on_floor(
            PortalColor::Blue,
            IVec3::new(0, 10, 0),
            FaceDirection::MinusZ,
        ));
        portals.insert(portal(
            PortalColor::Orange,
            IVec3::new(20, 10, 5),
            FaceDirection::PlusZ,
        ));
        portals
    }

    #[test]
    fn falling_into_a_floor_portal_comes_out_of_a_wall_upright() {
        let portals = floor_and_wall();
        let blue = portals.get(7, PortalColor::Blue).unwrap();
        let through = portals.through(blue).unwrap();
        // Down into the floor is out of the wall; the floor's up, north, is the wall's up
        assert_near(through.transform_vector3(Vec3::NEG_Y), Vec3::Z);
        assert_near(through.transform_vector3(Vec3::NEG_Z), Vec3::Y);
        // The centres meet: the floor's (1, 11, -0.5), the wall's (21, 11.5, 6)
        assert_near(
            through.transform_point3(Vec3::new(1.0, 11.0, -0.5)),
            Vec3::new(21.0, 11.5, 6.0),
        );
        // Rigid, and not mirrored
        assert!((through.matrix3.determinant() - 1.0).abs() < 1e-5);
        let back = portals
            .through(portals.get(7, PortalColor::Orange).unwrap())
            .unwrap();
        let point = Vec3::new(3.0, 12.5, -1.0);
        assert_near((back * through).transform_point3(point), point);
    }

    #[test]
    fn walking_into_a_wall_portal_comes_up_out_of_a_floor() {
        let portals = floor_and_wall();
        let orange = portals.get(7, PortalColor::Orange).unwrap();
        let through = portals.through(orange).unwrap();
        // Into the wall, south to north, is up out of the floor
        assert_near(through.transform_vector3(Vec3::NEG_Z), Vec3::Y);
        // The wall's up is the floor's, north
        assert_near(through.transform_vector3(Vec3::Y), Vec3::NEG_Z);
    }

    #[test]
    fn falling_into_a_floor_portal_comes_out_of_a_ceiling_still_falling() {
        let mut portals = Portals::default();
        portals.insert(on_floor(
            PortalColor::Blue,
            IVec3::new(0, 10, 0),
            FaceDirection::MinusZ,
        ));
        portals.insert(Portal {
            up: FaceDirection::PlusX,
            ..portal(
                PortalColor::Orange,
                IVec3::new(20, 30, 5),
                FaceDirection::MinusY,
            )
        });
        let through = portals
            .through(portals.get(7, PortalColor::Blue).unwrap())
            .unwrap();
        assert_near(through.transform_vector3(Vec3::NEG_Y), Vec3::NEG_Y);
        // Up north on the floor, up east on the ceiling
        assert_near(through.transform_vector3(Vec3::NEG_Z), Vec3::X);
        // A point a block above the floor's centre is a block below the ceiling's, behind it
        let ceiling_centre = portals
            .get(7, PortalColor::Orange)
            .unwrap()
            .frame()
            .translation;
        assert_near(
            through.transform_point3(Vec3::new(1.0, 12.0, -0.5)),
            Vec3::from(ceiling_centre) + Vec3::Y,
        );
    }

    #[test]
    fn a_floor_portal_lets_through_and_is_crossed_from_above_within_its_opening() {
        let portals = floor_and_wall();
        let letting = |point: Vec3| {
            portals
                .letting_through(point, 0.3)
                .map(|portal| portal.color)
                .collect::<Vec<_>>()
        };
        // The opening is x in 0..2, z in -2..1, its plane y = 11
        assert_eq!(letting(Vec3::new(1.0, 11.9, -0.5)), [PortalColor::Blue]);
        assert_eq!(letting(Vec3::new(1.6, 11.9, 0.6)), [PortalColor::Blue]);
        // Too near an edge for the box to fit, and below the plane
        assert!(letting(Vec3::new(1.8, 11.9, -0.5)).is_empty());
        assert!(letting(Vec3::new(1.0, 11.9, 0.8)).is_empty());
        assert!(letting(Vec3::new(1.0, 10.9, -0.5)).is_empty());

        let crossed = portals.crossed(Vec3::new(1.0, 11.2, -0.5), Vec3::new(1.0, 10.8, -0.5));
        assert_eq!(crossed.map(|portal| portal.color), Some(PortalColor::Blue));
        // Beside the opening, or from below
        assert!(portals
            .crossed(Vec3::new(2.2, 11.2, -0.5), Vec3::new(2.2, 10.8, -0.5))
            .is_none());
        assert!(portals
            .crossed(Vec3::new(1.0, 10.8, -0.5), Vec3::new(1.0, 11.2, -0.5))
            .is_none());
    }

    #[test]
    fn a_floor_portal_covers_and_opens_the_top_of_its_cells() {
        let portals = floor_and_wall();
        assert!(portals.covers(IVec3::new(1, 10, -2), FaceDirection::PlusY));
        assert!(portals.opens(IVec3::new(0, 10, 0), FaceDirection::PlusY));
        assert!(!portals.covers(IVec3::new(1, 10, -2), FaceDirection::MinusY));
        assert!(!portals.covers(IVec3::new(1, 10, 1), FaceDirection::PlusY));
    }

    #[test]
    fn a_floor_portal_overlaps_one_on_a_wall_at_its_edge_but_not_one_under_its_slab() {
        let mut portals = Portals::default();
        portals.insert(Portal {
            owner: 8,
            ..on_floor(
                PortalColor::Orange,
                IVec3::new(0, 10, 0),
                FaceDirection::MinusZ,
            )
        });
        // On the south face of its cells (0..2, 10, 0): a box in each from two sides
        let wall = portal(PortalColor::Blue, IVec3::new(0, 8, 0), FaceDirection::PlusZ);
        assert!(wall.cells().any(|cell| cell == IVec3::new(0, 10, 0)));
        assert!(portals.overlaps(&wall));
        // On the ceiling under the same cells: the opposite face
        let under = Portal {
            up: FaceDirection::PlusZ,
            ..portal(
                PortalColor::Blue,
                IVec3::new(1, 10, -2),
                FaceDirection::MinusY,
            )
        };
        assert!(under.cells().all(|cell| cell.y == 10));
        assert!(!portals.overlaps(&under));
    }

    /// A stone slab 9 × 9 at y = 10, x and z in -4..=4, and another at y = 20 above it.
    fn slabs() -> ChunkMap {
        let mut world = ChunkMap::with_loaded_region(IVec3::new(-8, 0, -8), IVec3::new(8, 31, 8));
        world.fill_box(IVec3::new(-4, 10, -4), IVec3::new(4, 10, 4), BlockId::Stone);
        world.fill_box(IVec3::new(-4, 20, -4), IVec3::new(4, 20, 4), BlockId::Stone);
        world
    }

    #[test]
    fn a_portal_fits_on_a_floor_and_a_ceiling_along_each_facing() {
        let world = slabs();
        let none = Portals::default();
        let horizontal = [
            (Vec3::new(0.1, 0.0, -1.0), FaceDirection::MinusZ),
            (Vec3::new(0.1, 0.0, 1.0), FaceDirection::PlusZ),
            (Vec3::new(1.0, 0.0, 0.1), FaceDirection::PlusX),
            (Vec3::new(-1.0, 0.0, 0.1), FaceDirection::MinusX),
        ];
        for (face, cell_y, plane_y, down) in [
            (FaceDirection::PlusY, 10, 11.0, -1.0),
            (FaceDirection::MinusY, 20, 20.0, 1.0),
        ] {
            for (facing, up) in horizontal {
                let direction = (facing + Vec3::Y * down).normalize();
                let hit = Vec3::new(0.3, plane_y, 0.6);
                let aimed = IVec3::new(0, cell_y, 0);
                let fitted = fit_portal(
                    &world,
                    &none,
                    7,
                    PortalColor::Blue,
                    aimed,
                    face,
                    hit,
                    direction,
                )
                .unwrap_or_else(|| panic!("{face:?} {up:?}"));
                assert_eq!((fitted.face, fitted.up), (face, up));
                let cells: Vec<_> = fitted.cells().collect();
                assert!(cells.contains(&aimed), "{face:?} {up:?}");
                assert!(cells.iter().all(|cell| cell.y == cell_y));
                // Three cells long along the facing, two across
                let up_axis = up.to_ivec3().abs();
                let span = |axis: IVec3| {
                    let along: Vec<i32> = cells.iter().map(|cell| cell.dot(axis)).collect();
                    along.iter().max().unwrap() - along.iter().min().unwrap() + 1
                };
                assert_eq!(span(up_axis), 3, "{face:?} {up:?}");
                assert_eq!(span(IVec3::new(1, 0, 1) - up_axis), 2, "{face:?} {up:?}");
                // The nearest of the placements to the hit point
                let centre = Vec3::from(fitted.frame().translation);
                assert!(centre.distance(hit) < 1.2, "{face:?} {up:?}: {centre}");
            }
        }
        // Looking north and down at the floor, hit at (0.3, 11, 0.6): west and south of the
        // aimed cell's middle, the opening reaches west of it, and north and south of it
        let north = fit_portal(
            &world,
            &none,
            7,
            PortalColor::Blue,
            IVec3::ZERO.with_y(10),
            FaceDirection::PlusY,
            Vec3::new(0.3, 11.0, 0.6),
            Vec3::new(0.0, -1.0, -1.0).normalize(),
        )
        .unwrap();
        assert_eq!(north.anchor, IVec3::new(-1, 10, 1));
    }

    #[test]
    fn a_floor_portal_needs_free_room_above_its_six_cells() {
        let mut world = slabs();
        let none = Portals::default();
        let down_north = Vec3::new(0.0, -1.0, -1.0).normalize();
        let fit = |world: &ChunkMap| {
            fit_portal(
                world,
                &none,
                7,
                PortalColor::Blue,
                IVec3::new(0, 10, 0),
                FaceDirection::PlusY,
                Vec3::new(0.5, 11.0, 0.5),
                down_north,
            )
        };
        let free = fit(&world).unwrap();
        // A block on one of the cells above it: the next placement, away from the block
        let blocked = free.front_cells().next().unwrap();
        world.fill_box(blocked, blocked, BlockId::Dirt);
        let moved = fit(&world).unwrap();
        assert!(moved.front_cells().all(|cell| cell != blocked));
        // At the edge of the slab, a placement that would reach past it does not fit
        let world = slabs();
        let edge = fit_portal(
            &world,
            &none,
            7,
            PortalColor::Blue,
            IVec3::new(4, 10, -4),
            FaceDirection::PlusY,
            Vec3::new(4.9, 11.0, -3.9),
            down_north,
        )
        .unwrap();
        assert!(edge.cells().all(|cell| cell.x <= 4 && cell.z >= -4));
    }

    #[test]
    fn a_portal_fits_while_its_wall_stands_and_its_front_is_free() {
        let mut world = wall();
        let portal = portal(
            PortalColor::Blue,
            IVec3::new(1, 10, 0),
            FaceDirection::PlusZ,
        );
        assert_eq!(portal.front_cells().count(), 6);
        assert!(portal.front_cells().all(|cell| cell.z == 1));
        assert!(portal.fits(&world));
        world.fill_box(IVec3::new(1, 12, 1), IVec3::new(1, 12, 1), BlockId::Sand);
        assert!(!portal.fits(&world));
    }
}
