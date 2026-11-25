use bevy::math::IVec3;

use crate::{
    messages::{NetworkAction, PlayerFrameInput},
    players::{
        blocks::{
            simulate_player_block_interactions, BrokenBlock, CallerType, PortalShot, SpawnedMob,
        },
        hunger::BREAK_EXHAUSTION,
        inventory::simulate_inventory_actions,
        movement::simulate_player_movement,
        Player,
    },
    world::{ChunkMap, ItemStack},
};

/// What one input did that the server acts upon.
#[derive(Debug, Default, PartialEq)]
pub struct ActionsOutcome {
    /// The blocks the player broke.
    pub broken: Vec<BrokenBlock>,
    /// The mobs the player made appear with a spawn egg.
    pub spawned: Vec<SpawnedMob>,
    /// What the player filled and had no room for, to drop.
    pub dropped: Vec<ItemStack>,
    /// What the player threw out of its inventory, to throw ahead of it.
    pub thrown: Vec<ItemStack>,
    /// The signs the player reads, and the bedrolls it lies on.
    pub signs: Vec<IVec3>,
    pub bedrolls: Vec<IVec3>,
    /// How many blocks the player fell, when it landed.
    pub fallen: Option<f32>,
    /// The shots of the portal gun.
    pub shots: Vec<PortalShot>,
    /// How much the move and the broken blocks tired the player, for the server to apply
    /// (`Player::exhaust`).
    pub exhaustion: f32,
}

/// Applies one input of `player`. A dead player does nothing.
pub fn simulate_player_actions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) -> ActionsOutcome {
    player.is_swinging = !player.is_dead() && action.is_pressed(NetworkAction::LeftClick);
    player.hotbar_slot = action.hotbar_slot;
    if player.is_dead() {
        return ActionsOutcome::default();
    }
    let interactions = simulate_player_block_interactions(player, world_map, action, caller_type);
    let inventory = simulate_inventory_actions(player, world_map, action, caller_type);
    let movement = simulate_player_movement(player, world_map, action);
    let exhaustion = movement.exhaustion + interactions.broken.len() as f32 * BREAK_EXHAUSTION;
    ActionsOutcome {
        broken: interactions.broken,
        spawned: interactions.spawned,
        dropped: interactions.dropped,
        signs: interactions.signs,
        bedrolls: interactions.bedrolls,
        thrown: inventory
            .iter()
            .filter_map(|outcome| outcome.thrown)
            .collect(),
        fallen: movement.fallen,
        shots: interactions.shots,
        exhaustion,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::players::inventory::InventoryAction;
    use crate::world::ItemId;
    use bevy::prelude::{Transform, Vec3};

    #[test]
    fn what_a_player_throws_is_in_the_outcome_apart_from_what_it_had_no_room_for() {
        let dirt = |nb| ItemStack {
            item_id: ItemId::Dirt,
            item_type: ItemId::Dirt.get_default_type(),
            nb,
        };
        let mut player = Player::new(1, "tester".into(), Vec3::ZERO, Transform::default());
        player.inventory.inner.insert(0, dirt(5));
        let input = PlayerFrameInput {
            inventory_actions: vec![
                InventoryAction::DropSlot {
                    slot: 0,
                    all: false,
                },
                InventoryAction::DropSlot { slot: 0, all: true },
            ],
            ..Default::default()
        };

        let outcome = simulate_player_actions(
            &mut player,
            &mut ChunkMap::default(),
            &input,
            CallerType::Server,
        );

        assert_eq!(outcome.thrown, [dirt(1), dirt(4)]);
        assert!(outcome.dropped.is_empty());
        assert!(player.inventory.inner.is_empty());
    }

    #[test]
    fn a_broken_block_tires_the_player() {
        use crate::messages::NetworkAction;
        use crate::world::BlockId;
        let target = IVec3::new(0, 0, -3);
        let camera = Transform::from_translation(Vec3::splat(0.5));
        let mut player = Player::new(1, "miner".into(), Vec3::splat(0.5), camera);
        let mut world = ChunkMap::with_loaded_region(IVec3::splat(-16), IVec3::splat(15));
        world.fill_box(target, target, BlockId::Dandelion);
        let input = PlayerFrameInput {
            delta_ms: crate::players::blocks::HIT_MS,
            inputs: [NetworkAction::LeftClick].into(),
            camera,
            ..Default::default()
        };
        let mut tired = 0.0;
        while world.get_block(target).is_some() {
            tired += simulate_player_actions(&mut player, &mut world, &input, CallerType::Server)
                .exhaustion;
        }
        assert_eq!(tired, BREAK_EXHAUSTION);
    }
}
