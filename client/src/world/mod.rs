pub mod celestial;
pub mod data;
pub mod raycast;
pub mod rendering;
pub mod time;

pub use data::*;
pub use raycast::*;
pub use rendering::*;

use bevy::prelude::Resource;

#[derive(Resource)]
pub struct FirstChunkReceived(pub bool);
