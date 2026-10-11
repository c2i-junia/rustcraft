use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::world::{MobId, ServerMob};

#[derive(Message, Serialize, Deserialize, Debug, Clone)]
pub struct MobUpdateEvent {
    pub id: MobId,
    pub mob: ServerMob,
}
