//! The portals on the server: those whose wall or front changed go, and the clients are told
//! of every change.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_renet::RenetServer;
use shared::messages::ServerToClientMessage;
use shared::world::{ChunkMap, Portal};

use crate::network::extensions::SendGameMessageExtension;
use crate::world::data::ServerWorldMap;

/// Removes the portals that no longer fit (`Portal::fits`) because one of their wall or front
/// cells changed since the change log was last drained. The others are not looked at: a
/// portal whose chunks are not loaded stays. Without a portal, the change log, which water
/// flowing can make long, is not looked at either.
pub fn remove_unfit_portals(chunks: &mut ChunkMap) {
    if chunks.portals().is_empty() {
        return;
    }
    let changed: HashSet<_> = chunks.changed_blocks().collect();
    let unfit: Vec<Portal> = chunks
        .portals()
        .iter()
        .filter(|portal| {
            portal
                .cells()
                .chain(portal.front_cells())
                .any(|cell| changed.contains(&cell))
                && !portal.fits(chunks)
        })
        .copied()
        .collect();
    for portal in unfit {
        chunks.portals_mut().remove(portal.owner, portal.color);
    }
}

/// `remove_unfit_portals`, once the blocks of the tick changed.
pub fn remove_unfit_portals_system(mut world_map: ResMut<ServerWorldMap>) {
    remove_unfit_portals(&mut world_map.chunks);
}

/// The message that tells a client every portal of the world.
pub fn all_portals(chunks: &ChunkMap) -> ServerToClientMessage {
    ServerToClientMessage::PortalUpdates(chunks.portals().iter().copied().collect())
}

/// `all_portals`, if the portals changed since the last call.
fn portal_updates(chunks: &mut ChunkMap) -> Option<ServerToClientMessage> {
    chunks
        .portals_mut()
        .take_changed()
        .then(|| all_portals(chunks))
}

/// Sends every portal to every player in the world when they changed in this tick.
pub fn broadcast_portals_system(
    mut server: ResMut<RenetServer>,
    mut world_map: ResMut<ServerWorldMap>,
) {
    let Some(message) = portal_updates(&mut world_map.chunks) else {
        return;
    };
    for client in server.clients_id() {
        if world_map.players.contains_key(&client) {
            server.send_game_message(client, message.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::math::IVec3;
    use shared::world::{BlockData, BlockDirection, BlockId, FaceDirection, Portal, PortalColor};

    /// A stone wall facing +Z, x in 0..=3, y in 10..=12, z = 0, with the blue portal of player
    /// 7 on its left half, the change log empty.
    fn walled_portal() -> (ChunkMap, Portal) {
        let mut chunks = ChunkMap::with_loaded_region(IVec3::new(-4, 0, -4), IVec3::new(8, 20, 8));
        chunks.fill_box(IVec3::new(0, 10, 0), IVec3::new(3, 12, 0), BlockId::Stone);
        let portal = Portal {
            owner: 7,
            color: PortalColor::Blue,
            anchor: IVec3::new(0, 10, 0),
            face: FaceDirection::PlusZ,
        };
        chunks.portals_mut().insert(portal);
        chunks.take_changed_blocks();
        chunks.portals_mut().take_changed();
        (chunks, portal)
    }

    #[test]
    fn a_portal_goes_when_a_wall_cell_is_broken() {
        let (mut chunks, _) = walled_portal();
        chunks.remove_block(IVec3::new(1, 12, 0));
        remove_unfit_portals(&mut chunks);
        assert!(chunks.portals().is_empty());
        assert!(chunks.portals_mut().take_changed());
    }

    #[test]
    fn a_portal_goes_when_a_cell_in_front_becomes_solid() {
        let (mut chunks, _) = walled_portal();
        chunks.fill_box(IVec3::new(0, 11, 1), IVec3::new(0, 11, 1), BlockId::Sand);
        remove_unfit_portals(&mut chunks);
        assert!(chunks.portals().is_empty());
    }

    #[test]
    fn a_portal_stays_when_water_flows_in_front_or_its_wall_is_only_hit() {
        let (mut chunks, portal) = walled_portal();
        chunks.fill_box(IVec3::new(1, 10, 1), IVec3::new(1, 10, 1), BlockId::Water);
        let mut hit = BlockData::new(BlockId::Stone, BlockDirection::Front);
        hit.breaking_progress = 3;
        chunks.set_block(IVec3::new(0, 11, 0), hit);
        // Next to the portal, not under it
        chunks.remove_block(IVec3::new(2, 11, 0));
        remove_unfit_portals(&mut chunks);
        assert_eq!(chunks.portals().iter().collect::<Vec<_>>(), [&portal]);
        assert!(!chunks.portals_mut().take_changed());
    }

    #[test]
    fn a_portal_stays_when_a_vine_hangs_on_its_wall_but_goes_for_a_cocoa_pod() {
        // Hung on the wall under the portal, in a front cell, as a player places it there
        let front = IVec3::new(1, 11, 1);
        let vine = BlockData::new(BlockId::Vine, BlockDirection::Front);
        let (mut chunks, portal) = walled_portal();
        chunks.set_block(front, vine);
        remove_unfit_portals(&mut chunks);
        assert_eq!(chunks.portals().iter().collect::<Vec<_>>(), [&portal]);
        assert!(!chunks.portals_mut().take_changed());

        // A pod collides with what walks into its box
        let (mut chunks, _) = walled_portal();
        chunks.set_block(front, BlockData::new(BlockId::Cocoa, BlockDirection::Front));
        remove_unfit_portals(&mut chunks);
        assert!(chunks.portals().is_empty());
    }

    #[test]
    fn a_portal_whose_cells_did_not_change_stays_even_out_of_the_loaded_world() {
        let (mut chunks, portal) = walled_portal();
        let far = Portal {
            anchor: IVec3::new(1000, 10, 1000),
            color: PortalColor::Orange,
            ..portal
        };
        chunks.portals_mut().insert(far);
        chunks.fill_box(IVec3::new(5, 5, 5), IVec3::new(5, 5, 5), BlockId::Dirt);
        remove_unfit_portals(&mut chunks);
        assert_eq!(chunks.portals().iter().count(), 2);
    }

    #[test]
    fn the_portals_are_sent_once_after_they_changed() {
        let (mut chunks, portal) = walled_portal();
        assert!(portal_updates(&mut chunks).is_none());
        chunks.portals_mut().touch();
        let Some(ServerToClientMessage::PortalUpdates(portals)) = portal_updates(&mut chunks)
        else {
            panic!("no portal updates");
        };
        assert_eq!(portals, [portal]);
        assert!(portal_updates(&mut chunks).is_none());
    }
}
