//! The portals on the server: those whose wall or front changed go, and the clients are told
//! of every change.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy_renet::RenetServer;
use shared::messages::{PlayerId, ServerToClientMessage};
use shared::players::blocks::PortalShot;
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

/// How far from the eyes of its shooter a shot may start, in blocks, to be told to the other
/// players. The client aims from its camera, placed at the eyes as they were before the input
/// (lowered while sneaking), or from the centre of the player in third person (`aim_ray`): a
/// shot further away comes from a camera the server cannot believe.
pub const SHOT_START_TOLERANCE: f32 = 2.0;

/// Whether `shot`, aimed by a player whose eyes were at `eyes` when it shot, may be told to the
/// other players: its ends are numbers, and it starts at the eyes, within
/// `SHOT_START_TOLERANCE`. The camera it starts from comes from the network, and every other
/// client would fly a bolt from there.
pub fn is_believable(shot: &PortalShot, eyes: Vec3) -> bool {
    shot.from.is_finite() && shot.to.is_finite() && shot.from.distance(eyes) <= SHOT_START_TOLERANCE
}

/// The messages telling a client of the shots of the tick: those of the other players, which
/// it did not predict.
fn shots_for(shots: &[(PlayerId, PortalShot)], client: PlayerId) -> Vec<ServerToClientMessage> {
    shots
        .iter()
        .filter(|(shooter, _)| *shooter != client)
        .map(|(shooter, shot)| ServerToClientMessage::PortalShot {
            shooter: *shooter,
            from: shot.from,
            to: shot.to,
            color: shot.color,
            placed: shot.placed,
        })
        .collect()
}

/// Sends the shots of the portal gun of this tick to every player in the world but their
/// shooters, then every portal to all of them when they changed in this tick: on the same
/// ordered channel, a client learns of a shot no later than of the portal it placed.
pub fn broadcast_portals_system(
    mut server: ResMut<RenetServer>,
    mut world_map: ResMut<ServerWorldMap>,
) {
    let shots = std::mem::take(&mut world_map.portal_shots);
    if !shots.is_empty() {
        for client in server.clients_id() {
            if world_map.players.contains_key(&client) {
                for message in shots_for(&shots, client) {
                    server.send_game_message(client, message);
                }
            }
        }
    }
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
    use shared::messages::{NetworkAction, PlayerFrameInput};
    use shared::players::blocks::{simulate_player_block_interactions, CallerType};
    use shared::players::Player;
    use shared::world::{
        BlockData, BlockDirection, BlockId, FaceDirection, Furnace, ItemId, ItemStack, Portal,
        PortalColor,
    };

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
            up: FaceDirection::PlusY,
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

    /// A player 3 blocks before the wall of `walled_portal`, holding `item`, aiming at the
    /// middle of the wall cell (1, 11, 0) under the portal, and the input that presses `button`.
    fn aiming_under_the_portal(item: ItemId, button: NetworkAction) -> (Player, PlayerFrameInput) {
        let position = Vec3::new(1.5, 11.0, 4.0);
        let camera = Transform::from_translation(position + Vec3::Y * 0.5)
            .looking_at(Vec3::new(1.5, 11.5, 1.0), Vec3::Y);
        let mut player = Player::new(1, "builder".to_string(), position, camera);
        let stack = ItemStack {
            item_id: item,
            item_type: item.get_default_type(),
            nb: 1,
        };
        player.inventory.inner.insert(0, stack);
        let input = PlayerFrameInput {
            delta_ms: 50,
            inputs: [button].into(),
            camera,
            ..Default::default()
        };
        (player, input)
    }

    #[test]
    fn a_portal_stays_when_a_bucket_pours_water_in_front_of_it() {
        let (mut chunks, portal) = walled_portal();
        let (mut player, input) =
            aiming_under_the_portal(ItemId::WaterBucket, NetworkAction::RightClick);
        simulate_player_block_interactions(&mut player, &mut chunks, &input, CallerType::Server);
        let front = IVec3::new(1, 11, 1);
        assert_eq!(
            chunks.get_block(front).map(|block| block.id),
            Some(BlockId::Water)
        );

        remove_unfit_portals(&mut chunks);
        assert_eq!(chunks.portals().iter().collect::<Vec<_>>(), [&portal]);
        assert!(!chunks.portals_mut().take_changed());
    }

    #[test]
    fn a_portal_goes_when_a_furnace_or_a_chest_is_placed_in_front_of_it() {
        for item in [ItemId::Furnace, ItemId::Chest] {
            let (mut chunks, _) = walled_portal();
            let (mut player, input) = aiming_under_the_portal(item, NetworkAction::RightClick);
            simulate_player_block_interactions(
                &mut player,
                &mut chunks,
                &input,
                CallerType::Server,
            );
            assert!(chunks.get_block(IVec3::new(1, 11, 1)).is_some(), "{item:?}");

            remove_unfit_portals(&mut chunks);
            assert!(chunks.portals().is_empty(), "{item:?}");
        }
    }

    #[test]
    fn a_furnace_in_the_wall_holds_a_portal_lit_or_out_until_it_is_broken() {
        let (mut chunks, portal) = walled_portal();
        let cell = IVec3::new(1, 11, 0);
        chunks.set_block(cell, BlockData::new(BlockId::Furnace, BlockDirection::Back));
        chunks.insert_furnace(cell, Furnace::default());
        remove_unfit_portals(&mut chunks);
        assert_eq!(chunks.portals().iter().collect::<Vec<_>>(), [&portal]);

        // Smelting lights it, and it goes out: the cell changes, the portal stays
        for id in [BlockId::LitFurnace, BlockId::Furnace] {
            chunks.set_block(cell, BlockData::new(id, BlockDirection::Back));
            remove_unfit_portals(&mut chunks);
            assert_eq!(
                chunks.portals().iter().collect::<Vec<_>>(),
                [&portal],
                "{id:?}"
            );
        }
        chunks.take_changed_blocks();

        // Broken by a player, it takes the portal with it
        let (mut player, input) =
            aiming_under_the_portal(ItemId::WoodenPickaxe, NetworkAction::LeftClick);
        let broken: Vec<_> = (0..200)
            .flat_map(|_| {
                simulate_player_block_interactions(
                    &mut player,
                    &mut chunks,
                    &input,
                    CallerType::Server,
                )
                .broken
            })
            .collect();
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].block, BlockId::Furnace);
        assert_eq!(broken[0].furnace, Some(Furnace::default()));
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

    #[test]
    fn a_shot_is_believed_from_the_eyes_or_the_centre_of_its_shooter_only() {
        let eyes = Vec3::new(3.5, 70.8, 1.5);
        let shot = |from: Vec3, to: Vec3| PortalShot {
            from,
            to,
            color: PortalColor::Blue,
            placed: false,
        };
        let ahead = Vec3::new(3.5, 70.8, -30.5);
        // From the eyes, the eyes lowered by sneaking, or the centre in third person
        for from in [eyes, eyes - Vec3::Y * 0.3, eyes - Vec3::Y * 0.8] {
            assert!(is_believable(&shot(from, ahead), eyes), "{from}");
        }
        // From a few blocks away, far away, or nowhere
        for from in [eyes + Vec3::X * 3.0, Vec3::splat(1.0e9), Vec3::NAN] {
            assert!(!is_believable(&shot(from, ahead), eyes), "{from}");
        }
        assert!(!is_believable(
            &shot(eyes, Vec3::new(f32::INFINITY, 0.0, 0.0)),
            eyes
        ));
    }
}
