use crate::{
    messages::PlayerFrameInput,
    players::{
        blocks::simulate_player_block_interactions, movement::simulate_player_movement, Player,
    },
    world::WorldMap,
};

pub fn simulate_player_actions(
    player: &mut Player,
    world_map: &mut impl WorldMap,
    action: &PlayerFrameInput,
) {
    // if !action.inputs.is_empty() {
    // debug!(
    //     "Simulating player actions for player {} -> {:?}",
    //     player.id, action
    // );
    // }

    // debug!("Camera = {:?}", action.camera);
    // debug!("Hotbar slot = {:?}", action.hotbar_slot);
    // debug!("Player position before = {:?}", player.position);
    // debug!("Player view mode = {:?}", action.view_mode);

    simulate_player_block_interactions(player, world_map, action);
    simulate_player_movement(player, world_map, action);
}
