use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource, Serialize, Deserialize, Debug, Clone)]
pub struct ClientTime(pub u64);

pub fn time_update_system(mut time: ResMut<ClientTime>) {
    time.0 += 1;
    // NOTE: time should eventually be periodically synced with the server to avoid drift using a NTP-like protocol
}
