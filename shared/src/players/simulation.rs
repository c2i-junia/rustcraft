use crate::{
    messages::PlayerFrameInput,
    players::{
        blocks::{simulate_player_block_interactions, BrokenBlock, CallerType, SpawnedMob},
        inventory::simulate_inventory_actions,
        movement::simulate_player_movement,
        Player,
    },
    world::ChunkMap,
};

/// What one input did that the server acts upon.
#[derive(Debug, Default, PartialEq)]
pub struct ActionsOutcome {
    /// The blocks the player broke.
    pub broken: Vec<BrokenBlock>,
    /// The mobs the player made appear with a spawn egg.
    pub spawned: Vec<SpawnedMob>,
    /// How many blocks the player fell, when it landed.
    pub fallen: Option<f32>,
}

/// Applies one input of `player`. A dead player does nothing.
pub fn simulate_player_actions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) -> ActionsOutcome {
    if player.is_dead() {
        return ActionsOutcome::default();
    }
    let interactions = simulate_player_block_interactions(player, world_map, action, caller_type);
    simulate_inventory_actions(player, world_map, action, caller_type);
    let fallen = simulate_player_movement(player, world_map, action);
    ActionsOutcome {
        broken: interactions.broken,
        spawned: interactions.spawned,
        fallen,
    }
}
