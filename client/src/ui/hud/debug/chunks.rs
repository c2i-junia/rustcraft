use crate::player::CurrentPlayerMarker;
use crate::GameState;
use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::PrimitiveTopology;
use bevy::render::render_asset::RenderAssetUsages;
use shared::CHUNK_SIZE;

use super::DebugOptions;

#[derive(Component)]
pub struct ChunkGhost;

pub fn setup_chunk_ghost(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        ChunkGhost,
        StateScoped(GameState::Game),
        NotShadowCaster,
        Transform::default(),
        GlobalTransform::default(),
        Visibility::Visible,
        InheritedVisibility::default(),
        Mesh3d::from(meshes.add(create_repeated_wireframe_mesh(
            CHUNK_SIZE as f32,
            (CHUNK_SIZE as f32) * 16.0,
            CHUNK_SIZE as u32,
            Vec3::ZERO,
        ))),
        MeshMaterial3d::from(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 1.0, 1.0),
            unlit: true,
            ..default()
        })),
    ));
}

pub fn chunk_ghost_update_system(
    mut ghost_query: Query<(&mut Transform, &mut Visibility), With<ChunkGhost>>,
    player_query: Query<&Transform, (With<CurrentPlayerMarker>, Without<ChunkGhost>)>,
    debug_options: Res<DebugOptions>,
) {
    let mut ghost = ghost_query.single_mut();
    let player = player_query.single();

    let mut chunk = shared::world::block_vec3_to_chunk_v3_coord(player.translation);
    chunk.y = 0.0;
    ghost.0.translation = chunk * (CHUNK_SIZE as f32);
    *ghost.1 = if debug_options.is_chunk_debug_mode_enabled {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
}

fn create_repeated_wireframe_mesh(size: f32, height: f32, layers: u32, position: Vec3) -> Mesh {
    // Create a new mesh with a line list topology
    let mut mesh = Mesh::new(
        PrimitiveTopology::LineList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );

    // Compute vertical spacing for the layers
    let layer_height = height / layers as f32;

    // Positions of the vertices (we'll add them across multiple layers)
    let mut positions: Vec<[f32; 3]> = vec![];

    for i in 0..layers {
        // Calculate the y-offset for this layer
        let y_offset = i as f32 * layer_height;

        // Define the corners of the chunk for this layer
        let corners = [
            Vec3::new(position.x, position.y + y_offset, position.z), // bottom-back-left
            Vec3::new(position.x + size, position.y + y_offset, position.z), // bottom-back-right
            Vec3::new(position.x + size, position.y + y_offset, position.z + size), // bottom-front-right
            Vec3::new(position.x, position.y + y_offset, position.z + size), // bottom-front-left
            Vec3::new(position.x, position.y + y_offset + layer_height, position.z), // top-back-left
            Vec3::new(
                position.x + size,
                position.y + y_offset + layer_height,
                position.z,
            ), // top-back-right
            Vec3::new(
                position.x + size,
                position.y + y_offset + layer_height,
                position.z + size,
            ), // top-front-right
            Vec3::new(
                position.x,
                position.y + y_offset + layer_height,
                position.z + size,
            ), // top-front-left
        ];

        // Define the edges of the cube at this layer (same as before)
        let edges = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // Bottom face edges
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 4), // Top face edges
            (0, 4),
            (1, 5),
            (2, 6),
            (3, 7), // Vertical edges
        ];

        // Add each edge's start and end points as vertices
        for &(start, end) in &edges {
            positions.push([corners[start].x, corners[start].y, corners[start].z]);
            positions.push([corners[end].x, corners[end].y, corners[end].z]);
        }
    }

    // Insert the positions as vertex attributes
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);

    mesh
}
