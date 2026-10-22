//! The portals of the world: openings 2 blocks wide and 3 high on a wall, each player owning a
//! blue one and an orange one. What lies in front of a portal is seen, and later reached, in
//! front of its twin, the portal of the other colour of the same player. They live in the
//! `ChunkMap`, so that the simulation of both sides reads them.

use bevy::math::{Affine3A, IVec3, Mat3, Vec3};
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::messages::PlayerId;
use crate::world::{BlockHitbox, BlockTransparency, ChunkMap, FaceDirection, FaceDirectionExt};

/// How many wall cells wide a portal is.
pub const PORTAL_WIDTH: i32 = 2;
/// How many wall cells high a portal is.
pub const PORTAL_HEIGHT: i32 = 3;

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

/// A portal on the face `face` of six wall cells. `anchor` is the lower-left wall cell, seen
/// from in front of the wall; the others are to its right and above it. The face is one of the
/// four vertical ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Portal {
    pub owner: PlayerId,
    pub color: PortalColor,
    pub anchor: IVec3,
    pub face: FaceDirection,
}

impl Portal {
    /// The direction the portal faces, out of the wall.
    pub fn normal(&self) -> Vec3 {
        self.face.to_ivec3().as_vec3()
    }

    /// The right of a viewer facing the portal.
    pub fn right(&self) -> Vec3 {
        Vec3::Y.cross(self.normal())
    }

    /// The six wall cells the portal lies on.
    pub fn cells(&self) -> impl Iterator<Item = IVec3> + '_ {
        let right = self.right().as_ivec3();
        (0..PORTAL_HEIGHT).flat_map(move |up| {
            (0..PORTAL_WIDTH).map(move |across| self.anchor + right * across + IVec3::Y * up)
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
        let (right, normal) = (self.right(), self.normal());
        let center = self.anchor.as_vec3()
            + Vec3::splat(0.5)
            + right * (PORTAL_WIDTH - 1) as f32 / 2.
            + Vec3::Y * (PORTAL_HEIGHT - 1) as f32 / 2.
            + normal * 0.5;
        Affine3A::from_mat3_translation(Mat3::from_cols(right, Vec3::Y, normal), center)
    }

    /// How far `point` is in front of the plane of the portal: negative behind it.
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal()
            .dot(point - Vec3::from(self.frame().translation))
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
    /// portals, it is turned half round the vertical.
    pub fn through(&self, portal: &Portal) -> Option<Affine3A> {
        let twin = self.twin(portal)?;
        Some(
            twin.frame()
                * Affine3A::from_rotation_y(std::f32::consts::PI)
                * portal.frame().inverse(),
        )
    }

    /// Whether `portal` shares a wall cell with a portal on the same face, but the one of its
    /// owner and colour, which it would replace.
    pub fn overlaps(&self, portal: &Portal) -> bool {
        self.portals.values().any(|other| {
            (other.owner, other.color) != (portal.owner, portal.color)
                && other.face == portal.face
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

/// The portal of `owner` and `color` that a shot hitting the face `face` of the cell `aimed`
/// at the point `hit` places, if any. Of the six placements on that face that hold the aimed
/// cell, it is the first that fits (`Portal::fits`) and overlaps no other portal, by the
/// distance of the centre of its opening to `hit`. A horizontal face takes none.
pub fn fit_portal(
    world: &ChunkMap,
    portals: &Portals,
    owner: PlayerId,
    color: PortalColor,
    aimed: IVec3,
    face: FaceDirection,
    hit: Vec3,
) -> Option<Portal> {
    if matches!(face, FaceDirection::PlusY | FaceDirection::MinusY) {
        return None;
    }
    let placement = |across: i32, up: i32| {
        let mut portal = Portal {
            owner,
            color,
            anchor: aimed,
            face,
        };
        portal.anchor -= portal.right().as_ivec3() * across + IVec3::Y * up;
        portal
    };
    let mut placements: Vec<Portal> = (0..PORTAL_WIDTH)
        .flat_map(|across| (0..PORTAL_HEIGHT).map(move |up| placement(across, up)))
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
    use crate::world::BlockId;

    fn portal(color: PortalColor, anchor: IVec3, face: FaceDirection) -> Portal {
        Portal {
            owner: 7,
            color,
            anchor,
            face,
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
        });
        assert_eq!(
            fitted(&world, &behind, aimed, 1.9, 11.9),
            Some(IVec3::new(1, 10, 0))
        );
    }

    #[test]
    fn nothing_fits_a_wall_too_narrow_or_a_horizontal_face() {
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
        );
        assert_eq!(floor, None);
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
