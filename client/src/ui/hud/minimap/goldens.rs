//! The golden screenshot of the minimap, over a made-up world. See `crate::ui::golden`.
//! It needs lavapipe, which is only installed on Linux.

use bevy::prelude::*;
use shared::players::Player;
use shared::world::{BlockData, BlockDirection, BlockId, Chunk};
use shared::CHUNK_SIZE;

use super::colors::compute_minimap_colors;
use super::markers::{place_minimap_dots, point_minimap_arrow};
use super::surface::MinimapSurface;
use super::world_map::{
    draw_world_map, place_world_map, spawn_world_map, start_world_map, WorldMap,
};
use super::{draw_minimap, spawn_minimap, start_minimap_surface, write_minimap_coords};
use crate::camera::CameraController;
use crate::mob::MobRoot;
use crate::player::CurrentPlayerMarker;
use crate::ui::golden::UiGolden;
use crate::ui::hud::corner::spawn_top_right_column;
use crate::ui::settings::InterfaceSettings;
use crate::world::ClientWorldMap;

/// A world of 8 × 8 columns of chunks around the origin: grassland with a lake and its beach,
/// a stony hill, a few trees and a road.
fn made_up_world() -> ClientWorldMap {
    let mut map = ClientWorldMap::default();
    for x in -4..4 {
        for z in -4..4 {
            for y in [3, 4] {
                map.blocks.insert_chunk(IVec3::new(x, y, z), Chunk::new());
            }
        }
    }
    let mut set = |x, y, z, id| {
        map.blocks.set_block(
            IVec3::new(x, y, z),
            BlockData::new(id, BlockDirection::Front),
        )
    };
    let half = 4 * CHUNK_SIZE;
    for x in -half..half {
        for z in -half..half {
            let lake = (x + 22).pow(2) + (z - 18).pow(2);
            let hill = (x - 30).pow(2) + (z + 26).pow(2);
            if lake < 320 {
                let floor = 62 - (320 - lake) / 70;
                set(x, floor, z, BlockId::Sand);
                for y in floor + 1..=62 {
                    set(x, y, z, BlockId::Water);
                }
            } else if lake < 440 {
                set(x, 62, z, BlockId::Sand);
            } else if hill < 520 {
                let height = 64 + (520 - hill) / 40;
                let id = if height > 70 {
                    BlockId::Stone
                } else {
                    BlockId::Grass
                };
                set(x, height, z, id);
            } else if z == 2 && x < -6 {
                set(x, 63, z, BlockId::Cobblestone);
            } else {
                let step = (x.div_euclid(9) + z.div_euclid(13)).rem_euclid(3) == 0;
                set(x, 63 + i32::from(step), z, BlockId::Grass);
            }
        }
    }
    for (x, z) in [(10, 20), (16, 26), (-40, -30), (-34, -40), (40, 30)] {
        for dx in -2..=2 {
            for dz in -2..=2 {
                set(x + dx, 69, z + dz, BlockId::OakLeaves);
            }
        }
    }
    map
}

/// The made-up world, the player in it looking north-east, another player and a fox, and its
/// surface scanned.
fn golden_in_the_made_up_world() -> UiGolden {
    let mut golden = UiGolden::new();
    let world = golden.world_mut();
    world.insert_resource(made_up_world());
    world.insert_resource(InterfaceSettings::default());
    world.spawn((CurrentPlayerMarker {}, Transform::from_xyz(0.5, 64.0, 0.5)));
    // looking north-east
    let camera = Transform::default().looking_to(Vec3::new(1.0, -0.3, -1.0), Vec3::Y);
    world.spawn((CameraController::default(), GlobalTransform::from(camera)));
    // a player beyond the map, held at its edge, and a fox
    world.spawn((
        Player::default(),
        GlobalTransform::from_xyz(40.0, 70.0, -90.0),
    ));
    world.spawn((
        MobRoot { id: 1 },
        GlobalTransform::from_xyz(-12.0, 64.0, 14.0),
    ));

    golden.run_system(compute_minimap_colors);
    golden.run_system(start_minimap_surface);
    golden
        .world_mut()
        .resource_scope(|world, mut surface: Mut<MinimapSurface>| {
            let map = world.resource::<ClientWorldMap>();
            surface.scan_dirty(&map.blocks, IVec2::ZERO, usize::MAX);
        });
    golden
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_minimap() {
    let mut golden = golden_in_the_made_up_world();
    golden.run_system(spawn_top_right_column);
    golden.run_system(spawn_minimap);
    golden.run_system(draw_minimap);
    golden.run_system(point_minimap_arrow);
    golden.run_system(place_minimap_dots);
    golden.run_system(write_minimap_coords);
    golden.assert_matches("minimap");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_world_map() {
    let mut golden = golden_in_the_made_up_world();
    golden.world_mut().insert_resource(Time::<()>::default());
    golden.run_system(start_world_map);
    golden.run_system(
        |mut commands: Commands, assets: Res<AssetServer>, map: Res<WorldMap>| {
            spawn_world_map(&mut commands, &assets, &map);
        },
    );
    golden.run_system(draw_world_map);
    // laid out once, for the size of the view
    golden.wait_for_fonts();
    golden.run_system(place_world_map);
    golden.run_system(point_minimap_arrow);
    golden.run_system(write_minimap_coords);
    golden.assert_matches("world_map");
}
