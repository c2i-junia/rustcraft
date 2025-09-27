use bevy::{platform::collections::HashSet, prelude::*};
use shared::{messages::PlayerFrameInput, players::ViewMode};

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
    fn reset(&mut self, new_time: u64, new_delta: u64) {
        self.0 = PlayerFrameInput {
            time_ms: new_time,
            delta_ms: new_delta,
            inputs: HashSet::default(),
            camera: Transform::default(),
            position: Vec3::default(),
            hotbar_slot: 0,
            view_mode: ViewMode::default(),
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
        let current_time_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

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
        self.curr_time_ms - self.last_time_ms
    }

    fn advance(&mut self) {
        self.last_time_ms = self.curr_time_ms;
        self.curr_time_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
    }
}
