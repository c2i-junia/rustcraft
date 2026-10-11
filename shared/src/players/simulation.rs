use crate::{
    messages::PlayerFrameInput,
    players::{
        blocks::{simulate_player_block_interactions, CallerType},
        movement::simulate_player_movement,
        Player,
    },
    world::ChunkMap,
};

pub fn simulate_player_actions(
    player: &mut Player,
    world_map: &mut ChunkMap,
    action: &PlayerFrameInput,
    caller_type: CallerType,
) {
    simulate_player_block_interactions(player, world_map, action, caller_type);
    simulate_player_movement(player, world_map, action);
}
