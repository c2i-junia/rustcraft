use crate::GameFolderPaths;
use bevy::math::{IVec3, Vec3};
use std::{env, path::PathBuf};

use crate::CHUNK_SIZE;

pub fn get_game_folder(game_folder_path: Option<&GameFolderPaths>) -> PathBuf {
    if let Some(folder_path) = game_folder_path {
        // custom path, convert to absolute path
        let mut exe_path = env::current_exe().expect("Failed to get executable path");
        exe_path.pop(); // Remove executable name
        exe_path.join(&folder_path.game_folder_path)
    } else {
        panic!();
    }
}

pub fn block_to_chunk_coord(x: i32) -> i32 {
    if x >= 0 {
        x / CHUNK_SIZE
    } else {
        (x - (CHUNK_SIZE - 1)) / CHUNK_SIZE
    }
}

pub fn block_vec3_to_chunk_v3_coord(v: Vec3) -> Vec3 {
    Vec3::new(
        block_to_chunk_coord(v.x as i32) as f32,
        block_to_chunk_coord(v.y as i32) as f32,
        block_to_chunk_coord(v.z as i32) as f32,
    )
}

pub fn world_position_to_chunk_position(v: Vec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(v.x as i32),
        block_to_chunk_coord(v.y as i32),
        block_to_chunk_coord(v.z as i32),
    )
}

pub fn to_global_pos(chunk_pos: &IVec3, local_block_pos: &IVec3) -> IVec3 {
    *chunk_pos * CHUNK_SIZE + *local_block_pos
}

pub fn to_local_pos(global_block_pos: &IVec3) -> IVec3 {
    IVec3 {
        x: ((global_block_pos.x % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
        y: ((global_block_pos.y % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
        z: ((global_block_pos.z % CHUNK_SIZE) + CHUNK_SIZE) % CHUNK_SIZE,
    }
}

pub fn global_block_to_chunk_pos(global_block_pos: &IVec3) -> IVec3 {
    IVec3::new(
        block_to_chunk_coord(global_block_pos.x),
        block_to_chunk_coord(global_block_pos.y),
        block_to_chunk_coord(global_block_pos.z),
    )
}

pub const SIX_OFFSETS: [IVec3; 6] = [
    IVec3::new(1, 0, 0),
    IVec3::new(-1, 0, 0),
    IVec3::new(0, 1, 0),
    IVec3::new(0, -1, 0),
    IVec3::new(0, 0, 1),
    IVec3::new(0, 0, -1),
];

pub fn chunk_in_radius(player_pos: &IVec3, chunk_pos: &IVec3, radius: i32) -> bool {
    (player_pos.x - chunk_pos.x).abs() <= radius && (player_pos.z - chunk_pos.z).abs() <= radius
}
