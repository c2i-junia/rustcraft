//! Which portals the local player approaches, as the server reckons it (`ChunkReach::of_player`):
//! the server then sends the area around their twins, which the client meshes as it meshes the
//! player's own (`world_render_system`), and around which the groups cast shadows and the clouds
//! are meshed, as the player will see them after crossing.

use bevy::prelude::*;
use shared::players::Player;
use shared::world::{
    approach_radius, world_position_to_chunk_position, ChunkReach, PORTAL_APPROACH_RADIUS,
};

use crate::player::CurrentPlayerMarker;
use crate::world::{ClientWorldMap, RenderDistance, ServerRenderDistanceLimit};

/// How many chunks past its approach a twin stays approached for the shadows and the clouds, so
/// that walking to and fro across the edge does not cast and uncast every group around the twin
/// nor mesh the clouds again on each step.
const APPROACH_MARGIN: i32 = 1;

/// The approach of the local player and the twins it approaches.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Approach {
    /// How near a portal is approached, in chunks (`approach_radius`).
    pub radius: i32,
    /// The chunks of the twins approached, kept `APPROACH_MARGIN` chunks past the approach.
    pub twins: Vec<IVec3>,
}

impl Default for Approach {
    fn default() -> Self {
        Self {
            radius: PORTAL_APPROACH_RADIUS,
            twins: Vec::new(),
        }
    }
}

impl Approach {
    /// The reach of a player in the chunk `chunk` with the render distance `distance`, at this
    /// approach.
    pub fn reach(&self, chunk: IVec3, distance: i32, world_map: &ClientWorldMap) -> ChunkReach {
        ChunkReach::new(chunk, distance, world_map.blocks.portals()).approaching(self.radius)
    }

    /// The twins approached from `reach`: those approached now, and those approached before that
    /// are still within the margin.
    fn follow(&mut self, reach: &ChunkReach) {
        let now = reach.approached_twins();
        let kept = reach.approached_twins_within(APPROACH_MARGIN);
        let mut twins: Vec<IVec3> = self
            .twins
            .iter()
            .copied()
            .filter(|twin| kept.contains(twin))
            .collect();
        for twin in now {
            if !twins.contains(&twin) {
                twins.push(twin);
            }
        }
        if twins != self.twins {
            self.twins = twins;
        }
    }
}

/// Follows the approach of the local player and the twins it approaches.
pub fn track_approach(
    world_map: Res<ClientWorldMap>,
    player: Query<(&Player, &Transform), With<CurrentPlayerMarker>>,
    render_distance: Option<Res<RenderDistance>>,
    server_limit: Option<Res<ServerRenderDistanceLimit>>,
    mut approach: ResMut<Approach>,
) {
    let Ok((player, transform)) = player.single() else {
        return;
    };
    let distance = render_distance.map_or(0, |render| {
        render
            .distance
            .min(server_limit.map_or(u32::MAX, |limit| limit.0)) as i32
    });
    let radius = approach_radius(player);
    if approach.radius != radius {
        approach.radius = radius;
    }
    let reach = approach.reach(
        world_position_to_chunk_position(transform.translation),
        distance,
        &world_map,
    );
    // Only on a change, so that the systems that follow it see one
    let mut next = approach.clone();
    next.follow(&reach);
    if next.twins != approach.twins {
        approach.twins = next.twins;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{FaceDirection, Portal, PortalColor};
    use shared::CHUNK_SIZE;

    const TWIN: IVec3 = IVec3::new(40, 4, 0);

    fn app() -> App {
        let mut app = App::new();
        let mut world_map = ClientWorldMap::default();
        for (color, chunk) in [
            (PortalColor::Blue, IVec3::new(0, 4, 0)),
            (PortalColor::Orange, TWIN),
        ] {
            world_map.blocks.portals_mut().insert(Portal {
                owner: 1,
                color,
                anchor: chunk * CHUNK_SIZE + IVec3::new(4, 4, 4),
                face: FaceDirection::PlusZ,
                up: FaceDirection::PlusY,
            });
        }
        app.insert_resource(world_map)
            .insert_resource(RenderDistance { distance: 8 })
            .init_resource::<Approach>()
            .add_systems(Update, track_approach);
        app.world_mut().spawn((
            Player::new(1, "PlayerOne".into(), Vec3::ZERO, default()),
            Transform::default(),
            CurrentPlayerMarker {},
        ));
        app
    }

    fn walk_to(app: &mut App, chunk_x: i32) {
        let world = app.world_mut();
        let mut transform = world
            .query_filtered::<&mut Transform, With<CurrentPlayerMarker>>()
            .single_mut(world)
            .unwrap();
        transform.translation = Vec3::new((chunk_x * CHUNK_SIZE) as f32 + 8.0, 72.0, 8.0);
        app.update();
    }

    fn twins(app: &App) -> Vec<IVec3> {
        app.world().resource::<Approach>().twins.clone()
    }

    #[test]
    fn a_twin_stays_approached_a_chunk_past_the_approach() {
        let mut app = app();
        walk_to(&mut app, 0);
        assert_eq!(twins(&app), vec![TWIN]);
        // Past the approach, within the margin: still approached
        walk_to(&mut app, -PORTAL_APPROACH_RADIUS - 1);
        assert_eq!(twins(&app), vec![TWIN]);
        // Past the margin: no longer
        walk_to(&mut app, -PORTAL_APPROACH_RADIUS - 2);
        assert!(twins(&app).is_empty());
        // Back within the margin, not approached until within the approach
        walk_to(&mut app, -PORTAL_APPROACH_RADIUS - 1);
        assert!(twins(&app).is_empty());
        walk_to(&mut app, -PORTAL_APPROACH_RADIUS);
        assert_eq!(twins(&app), vec![TWIN]);
    }

    #[test]
    fn a_player_in_a_flying_sprint_approaches_from_farther() {
        let mut app = app();
        walk_to(&mut app, -5);
        assert!(twins(&app).is_empty());
        let world = app.world_mut();
        let mut player = world
            .query_filtered::<&mut Player, With<CurrentPlayerMarker>>()
            .single_mut(world)
            .unwrap();
        player.is_flying = true;
        player.is_sprinting = true;
        app.update();
        assert_eq!(app.world().resource::<Approach>().radius, 6);
        assert_eq!(twins(&app), vec![TWIN]);
    }
}
