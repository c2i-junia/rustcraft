use crate::{
    messages::PlayerFrameInput,
    players::{
        blocks::{simulate_player_block_interactions, BrokenBlock, CallerType},
        inventory::simulate_inventory_actions,
        movement::simulate_player_movement,
        Player,
    },
    world::ChunkMap,
};

/// Applies one input of `player`, and returns the blocks it broke.
pub fn simulate_player_actions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) -> Vec<BrokenBlock> {
    let broken = simulate_player_block_interactions(player, world_map, action, caller_type);
    simulate_inventory_actions(player, world_map, action, caller_type);
    simulate_player_movement(player, world_map, action);
    broken
}
