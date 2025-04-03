use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use shared::world::ServerChunk;
use std::collections::HashMap;

pub const SAVE_PATH: &str = "saves/";

#[derive(Serialize, Deserialize)]
pub struct Save {
    pub map: HashMap<IVec3, ServerChunk>,
}
