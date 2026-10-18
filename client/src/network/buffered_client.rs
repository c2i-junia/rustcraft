use bevy::{platform::collections::HashSet, prelude::*};
use shared::{
    messages::PlayerFrameInput,
    players::{MovementState, ViewMode},
};

#[derive(Debug, Default, Resource)]
pub struct PlayerTickInputsBuffer {
    pub buffer: Vec<PlayerFrameInput>,
}

#[derive(Resource, Default)]
pub struct CurrentFrameInputs(pub PlayerFrameInput);

pub trait CurrentFrameInputsExt {
    fn reset(&mut self, time: u64, delta: u64);
}

impl CurrentFrameInputsExt for CurrentFrameInputs {
    /// Starts the frame's input. The inventory actions of a frame that was not queued for the
    /// server, because it was not simulated, are kept for this one: a click is never lost.
    fn reset(&mut self, new_time: u64, new_delta: u64) {
        let pending_actions = std::mem::take(&mut self.0.inventory_actions);
        self.0 = PlayerFrameInput {
            time_ms: new_time,
            delta_ms: new_delta,
            inputs: HashSet::default(),
            camera: Transform::default(),
            predicted: MovementState::default(),
            hotbar_slot: 0,
            view_mode: ViewMode::default(),
            inventory_actions: pending_actions,
        };
    }
}

// Represents the synchronized time of the client
// Currently, this is just the UNIX timestamp in milliseconds, assumed to be the same on the server and client
// A NTP-like system could be implemented in the future
#[derive(Resource)]
pub struct SyncTime {
    pub last_time_ms: u64,
    pub curr_time_ms: u64,
}

impl Default for SyncTime {
    fn default() -> Self {
        let current_time_ms = shared::utils::unix_time_ms();

        Self {
            last_time_ms: current_time_ms,
            curr_time_ms: current_time_ms,
        }
    }
}

pub trait SyncTimeExt {
    fn delta(&self) -> u64;
    fn advance(&mut self);
}

impl SyncTimeExt for SyncTime {
    fn delta(&self) -> u64 {
        self.curr_time_ms.saturating_sub(self.last_time_ms)
    }

    fn advance(&mut self) {
        self.last_time_ms = self.curr_time_ms;
        self.curr_time_ms = shared::utils::unix_time_ms();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_keeps_the_inventory_actions_no_frame_has_queued() {
        use shared::players::inventory::InventoryAction;

        let mut frame = CurrentFrameInputs::default();
        frame.0.inventory_actions.push(InventoryAction::Close);

        frame.reset(2_000, 0);

        assert_eq!(frame.0.time_ms, 2_000);
        assert_eq!(frame.0.inventory_actions, [InventoryAction::Close]);
    }

    #[test]
    fn delta_is_the_time_elapsed_between_two_ticks() {
        let time = SyncTime {
            last_time_ms: 1_000,
            curr_time_ms: 1_016,
        };
        assert_eq!(time.delta(), 16);
    }

    #[test]
    fn delta_is_zero_when_the_clock_steps_back() {
        let time = SyncTime {
            last_time_ms: 2_000,
            curr_time_ms: 1_000,
        };
        assert_eq!(time.delta(), 0);
    }
}
