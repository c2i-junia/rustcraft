//! How much of the area around each twin the client really holds and shows: the chunks the
//! server sends around the twin of an open portal (`ChunkReach::areas`) come over several
//! broadcasts and are meshed after, and a portal camera's fog ends where they end
//! (`camera::held_reach`), so that a view streaming in shows fog, not holes.
//!
//! An area is measured in shells, the boxes of chunks within 0, 1, 2… chunks of the twin's
//! (`vertical_render_distance` of it along y), in front of the twin's plane only: behind it
//! nothing is drawn. The server sends them in that order (`twin_area_offsets`), so each frame
//! goes on from where the last one stopped; a chunk of the area that goes measures it again.

use std::collections::HashMap;

use bevy::prelude::*;
use shared::world::{
    twin_area_key, twin_area_offsets, vertical_render_distance, PORTAL_VIEW_RADIUS,
};

use crate::world::{
    granted_render_distance, group_of, ChunkLoadEvent, ClientWorldMap, RenderDistance,
    ServerRenderDistanceLimit,
};

/// Whether the client shows the chunk `chunk`: it holds it, and a mesh made after it came is
/// shown, or its group needs none (`ChunkRender::meshed`).
pub(crate) fn shows(world_map: &ClientWorldMap, chunk: IVec3) -> bool {
    world_map.blocks.is_chunk_loaded(chunk)
        && world_map
            .render
            .get(&group_of(chunk))
            .is_none_or(|render| render.meshed)
}

/// What the client shows of the area around a twin.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HeldArea {
    twin: IVec3,
    facing: IVec3,
    /// The most chunks around the twin it may hold: the player's render distance, or the view
    /// radius, whichever is larger.
    radius: i32,
    /// How many of the chunks of the area, in the order they are sent, are shown or behind.
    checked: usize,
}

impl HeldArea {
    pub(crate) fn new(twin: IVec3, facing: IVec3, radius: i32) -> Self {
        Self {
            twin,
            facing,
            radius,
            checked: 0,
        }
    }

    /// Goes on through the chunks of the area in front of the twin, in the order they are sent,
    /// as far as they are shown.
    pub(crate) fn update(&mut self, world_map: &ClientWorldMap) {
        let offsets = twin_area_offsets(self.radius, self.facing);
        while let Some(offset) = offsets.get(self.checked) {
            if offset.dot(self.facing) >= 0 && !shows(world_map, self.twin + *offset) {
                break;
            }
            self.checked += 1;
        }
    }

    /// Measures the area again from where the chunk `chunk`, gone, comes in it, if it counted:
    /// in front of the twin's plane, within the area, among the chunks checked. One behind the
    /// twin, above or below the area or past the chunks checked changes nothing.
    pub(crate) fn forget(&mut self, chunk: IVec3) {
        let offset = chunk - self.twin;
        if offset.dot(self.facing) < 0
            || offset.xz().abs().max_element() > self.radius
            || offset.y.abs() > vertical_render_distance(self.radius)
        {
            return;
        }
        // The first chunk of its place in the order: those before it are still shown
        let offsets = twin_area_offsets(self.radius, self.facing);
        let key = twin_area_key(offset, self.facing);
        let at = offsets.partition_point(|other| twin_area_key(*other, self.facing) < key);
        self.checked = self.checked.min(at);
    }

    /// How many shells around the twin are shown whole, in front of its plane: `None` when not
    /// even the twin's chunk is.
    pub(crate) fn shells(&self) -> Option<i32> {
        let offsets = twin_area_offsets(self.radius, self.facing);
        // The shell of the first chunk in front not shown: those before it are whole
        let next = offsets[self.checked.min(offsets.len())..]
            .iter()
            .find(|offset| offset.dot(self.facing) >= 0)
            .map(|offset| offset.abs().max_element());
        match next {
            Some(0) => None,
            Some(shell) => Some(shell - 1),
            None => Some(self.radius),
        }
    }
}

/// What the client shows of the area around each twin of an open portal, by the twin's chunk
/// and facing.
#[derive(Resource, Default)]
pub struct HeldAreas(HashMap<(IVec3, IVec3), HeldArea>);

impl HeldAreas {
    /// How many shells around the twin in the chunk `twin`, facing `facing`, are shown whole
    /// (`HeldArea::shells`).
    pub(crate) fn shells(&self, twin: IVec3, facing: IVec3) -> Option<i32> {
        self.0.get(&(twin, facing)).and_then(HeldArea::shells)
    }
}

/// Follows what the client shows around the twin of every open portal, as chunks come, are
/// meshed and go.
pub(super) fn track_held_areas(
    world_map: Res<ClientWorldMap>,
    render_distance: Option<Res<RenderDistance>>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    mut loads: MessageReader<ChunkLoadEvent>,
    mut held: ResMut<HeldAreas>,
) {
    let distance =
        granted_render_distance(render_distance.as_deref(), server_limit.as_deref()) as i32;
    let radius = distance.max(PORTAL_VIEW_RADIUS);
    let portals = world_map.blocks.portals();
    let twins: Vec<(IVec3, IVec3)> = portals
        .iter()
        .filter_map(|portal| portals.twin(portal))
        .map(|twin| (twin.chunk(), twin.normal().round().as_ivec3()))
        .collect();
    held.0
        .retain(|key, area| twins.contains(key) && area.radius == radius);
    for &(twin, facing) in &twins {
        held.0
            .entry((twin, facing))
            .or_insert_with(|| HeldArea::new(twin, facing, radius));
    }
    for load in loads.read() {
        if let ChunkLoadEvent::Unloaded(chunk) = load {
            for area in held.0.values_mut() {
                area.forget(*chunk);
            }
        }
    }
    for area in held.0.values_mut() {
        area.update(&world_map);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::ChunkRender;
    use shared::world::Chunk;

    const TWIN: IVec3 = IVec3::new(10, 4, 0);

    /// Holds the chunks within `radius` of `TWIN`, its render distance's height, meshed.
    fn hold(world_map: &mut ClientWorldMap, radius: i32) {
        let vertical = vertical_render_distance(radius);
        for x in -radius..=radius {
            for y in -vertical..=vertical {
                for z in -radius..=radius {
                    let chunk = TWIN + IVec3::new(x, y, z);
                    world_map.blocks.insert_chunk(chunk, Chunk::new());
                    world_map.chunk_arrived(chunk);
                    world_map.render.get_mut(&group_of(chunk)).unwrap().meshed = true;
                }
            }
        }
    }

    #[test]
    fn nothing_held_has_no_shell() {
        let mut area = HeldArea::new(TWIN, IVec3::NEG_X, 8);
        area.update(&ClientWorldMap::default());
        assert_eq!(area.shells(), None);
    }

    #[test]
    fn the_shells_shown_grow_as_the_chunks_come() {
        let mut world_map = ClientWorldMap::default();
        let mut area = HeldArea::new(TWIN, IVec3::NEG_X, 8);
        for radius in [0, 2, 5, 8] {
            hold(&mut world_map, radius);
            area.update(&world_map);
            assert_eq!(area.shells(), Some(radius));
        }
    }

    #[test]
    fn a_chunk_not_meshed_yet_is_not_shown() {
        let mut world_map = ClientWorldMap::default();
        hold(&mut world_map, 3);
        // A chunk of the fourth shell arrives, in a group shown with the third: the group is
        // to be meshed again, and the third shell stays shown
        let corner = TWIN + IVec3::new(-3, 0, 3);
        let fourth = TWIN + IVec3::new(-4, 0, 3);
        assert_eq!(group_of(corner), group_of(fourth));
        let mut area = HeldArea::new(TWIN, IVec3::NEG_X, 8);
        area.update(&world_map);
        world_map.blocks.insert_chunk(fourth, Chunk::new());
        world_map.chunk_arrived(fourth);
        area.update(&world_map);
        assert_eq!(area.shells(), Some(3));
        // Measured from scratch, the third shell's chunks of that group wait for its mesh
        let mut fresh = HeldArea::new(TWIN, IVec3::NEG_X, 8);
        fresh.update(&world_map);
        assert!(fresh.shells() < Some(3));
        world_map.render.get_mut(&group_of(fourth)).unwrap().meshed = true;
        fresh.update(&world_map);
        assert_eq!(fresh.shells(), Some(3));
    }

    #[test]
    fn the_chunks_behind_the_twin_do_not_count() {
        let mut world_map = ClientWorldMap::default();
        hold(&mut world_map, 4);
        // East of the twin facing west, all of the fourth shell goes
        for y in -4..=4 {
            for z in -4..=4 {
                let chunk = TWIN + IVec3::new(4, y, z);
                world_map.blocks.remove_chunk(chunk);
            }
        }
        let mut area = HeldArea::new(TWIN, IVec3::NEG_X, 8);
        area.update(&world_map);
        assert_eq!(area.shells(), Some(4));
        // Facing east, it is the shell in front
        let mut area = HeldArea::new(TWIN, IVec3::X, 8);
        area.update(&world_map);
        assert_eq!(area.shells(), Some(3));
    }

    #[test]
    fn a_chunk_of_the_area_that_goes_measures_it_again() {
        let mut world_map = ClientWorldMap::default();
        hold(&mut world_map, 6);
        let mut area = HeldArea::new(TWIN, IVec3::NEG_X, 6);
        area.update(&world_map);
        assert_eq!(area.shells(), Some(6));
        let gone = TWIN + IVec3::new(-2, 1, 0);
        world_map.blocks.remove_chunk(gone);
        area.forget(gone);
        area.update(&world_map);
        assert_eq!(area.shells(), Some(1));
        // One beyond the area changes nothing
        area.forget(TWIN + IVec3::new(-20, 0, 0));
        assert_eq!(area.shells(), Some(1));
    }

    #[test]
    fn a_chunk_that_goes_behind_the_twin_or_past_its_height_does_not_measure_it_again() {
        let mut world_map = ClientWorldMap::default();
        hold(&mut world_map, 6);
        let mut area = HeldArea::new(TWIN, IVec3::NEG_X, 6);
        area.update(&world_map);
        let checked = area.checked;
        // Behind the twin, facing west
        let behind = TWIN + IVec3::new(2, 0, 0);
        world_map.blocks.remove_chunk(behind);
        area.forget(behind);
        assert_eq!(area.checked, checked);
        // Above the area's height
        area.forget(TWIN + IVec3::new(-1, vertical_render_distance(6) + 1, 0));
        assert_eq!(area.checked, checked);
        // In front, in the fifth shell: only from there
        let fifth = TWIN + IVec3::new(-5, 0, 0);
        world_map.blocks.remove_chunk(fifth);
        area.forget(fifth);
        assert!(area.checked > 0 && area.checked < checked);
        area.update(&world_map);
        assert_eq!(area.shells(), Some(4));
    }

    #[test]
    fn the_areas_follow_the_open_portals() {
        use shared::world::{FaceDirection, Portal, PortalColor};
        let mut app = App::new();
        app.init_resource::<ClientWorldMap>()
            .init_resource::<HeldAreas>()
            .add_message::<ChunkLoadEvent>()
            .add_systems(Update, track_held_areas);
        let portal = |color, anchor| Portal {
            owner: 1,
            color,
            anchor,
            face: FaceDirection::MinusX,
            up: FaceDirection::PlusY,
        };
        let orange = portal(PortalColor::Orange, TWIN * 16 + IVec3::new(0, 2, 5));
        let twin_chunk = orange.chunk();
        {
            let mut world_map = app.world_mut().resource_mut::<ClientWorldMap>();
            world_map
                .blocks
                .portals_mut()
                .insert(portal(PortalColor::Blue, IVec3::ZERO));
            world_map.blocks.portals_mut().insert(orange);
            world_map.blocks.insert_chunk(twin_chunk, Chunk::new());
            world_map.chunk_arrived(twin_chunk);
            world_map
                .render
                .get_mut(&group_of(twin_chunk))
                .unwrap()
                .meshed = true;
        }
        app.update();
        let shells = |app: &App| {
            app.world()
                .resource::<HeldAreas>()
                .shells(twin_chunk, IVec3::NEG_X)
        };
        assert_eq!(shells(&app), Some(0));

        // Unloaded, the twin's chunk is no longer shown
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .remove_chunk(twin_chunk);
        app.world_mut()
            .write_message(ChunkLoadEvent::Unloaded(twin_chunk));
        app.update();
        assert_eq!(shells(&app), None);

        // Closed, the portal has no area
        app.world_mut()
            .resource_mut::<ClientWorldMap>()
            .blocks
            .portals_mut()
            .remove(1, PortalColor::Blue);
        app.update();
        assert!(app.world().resource::<HeldAreas>().0.is_empty());
    }

    #[test]
    fn a_new_chunk_render_is_not_meshed() {
        assert!(!ChunkRender::new().meshed);
    }
}
