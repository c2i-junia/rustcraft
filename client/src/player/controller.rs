use crate::camera::CameraController;
use crate::constants::GRAVITY;
use crate::input::data::GameAction;
use crate::input::keyboard::*;
use crate::network::request_world_update;
use crate::player::{Player, ViewMode};
use crate::ui::hud::debug::DebugOptions;
use crate::ui::hud::UIMode;
use crate::world::render_distance::RenderDistance;
use crate::world::{ClientWorldMap, WorldRenderRequestUpdateEvent};
use crate::KeyMap;
use bevy::prelude::*;
use bevy_renet::renet::RenetClient;
use shared::world::{block_to_chunk_coord, chunk_in_radius, WorldMap};

use super::CurrentPlayerMarker;
use crate::world::FirstChunkReceived;

fn is_block_at_position(position: Vec3, world_map: &ClientWorldMap) -> bool {
    if let Some(block) = world_map.get_block_by_coordinates(&IVec3::new(
        position.x.round() as i32,
        position.y.round() as i32,
        position.z.round() as i32,
    )) {
        block.id.has_hitbox()
    } else {
        false
    }
}

fn check_player_collision(
    player_position: Vec3,
    player: &Player,
    world_map: &ClientWorldMap,
) -> bool {
    // Vérification de la collision avec les pieds et la tête du joueur
    let foot_position = Vec3::new(
        player_position.x,
        player_position.y - player.height / 2.0,
        player_position.z,
    );
    let head_position = Vec3::new(
        player_position.x,
        player_position.y + player.height / 2.0,
        player_position.z,
    );

    // On vérifie les coins du joueur
    let offsets = [
        Vec3::new(-player.width / 2.0, 0.0, -player.width / 2.0), // bas gauche devant
        Vec3::new(player.width / 2.0, 0.0, -player.width / 2.0),  // bas droite devant
        Vec3::new(-player.width / 2.0, 0.0, player.width / 2.0),  // bas gauche derrière
        Vec3::new(player.width / 2.0, 0.0, player.width / 2.0),   // bas droite derrière
    ];

    // Vérifier la collision au niveau des pieds
    for offset in &offsets {
        let check_pos = foot_position + *offset;
        if is_block_at_position(check_pos, world_map) {
            return true;
        }
    }

    // Vérifier la collision au niveau de la tête
    for offset in &offsets {
        let check_pos = head_position + *offset;
        if is_block_at_position(check_pos, world_map) {
            return true;
        }
    }

    false
}

#[derive(Component)]
pub struct PlayerMaterialHandle {
    pub handle: Handle<StandardMaterial>,
}

// System to move the player based on keyboard input
pub fn player_movement_system(
    queries: (
        Query<(&mut Transform, &mut Player, &mut PlayerMaterialHandle), With<CurrentPlayerMarker>>,
        Query<&Transform, (With<Camera>, With<CameraController>, Without<Player>)>,
    ),
    resources: (
        Res<Time>,
        Res<ButtonInput<KeyCode>>,
        Res<UIMode>,
        Res<KeyMap>,
        ResMut<Assets<StandardMaterial>>,
        ResMut<ClientWorldMap>,
        Res<RenderDistance>,
        ResMut<ViewMode>,
        ResMut<DebugOptions>,
        ResMut<RenetClient>,
    ),
    mut previous_player_chunk: Local<IVec3>,
    mut commands: Commands,
    mut ev_writer: EventWriter<WorldRenderRequestUpdateEvent>,
    first_chunk_received: ResMut<FirstChunkReceived>,
) {
    let (mut player_query, camera_query) = queries;
    let (
        time,
        keyboard_input,
        ui_mode,
        key_map,
        mut materials,
        mut world_map,
        render_distance,
        mut view_mode,
        mut debug_options,
        mut client,
    ) = resources;

    let res = player_query.get_single_mut();
    // return early if player is not spawned
    if res.is_err() {
        debug!("player not found");
        return;
    }

    let (mut player_transform, mut player, material_handle) = player_query.single_mut();

    let camera_transform = camera_query.single();

    if *ui_mode == UIMode::Closed {
        if is_action_just_pressed(GameAction::ToggleViewMode, &keyboard_input, &key_map) {
            view_mode.toggle();
        }

        if is_action_just_pressed(GameAction::ToggleChunkDebugMode, &keyboard_input, &key_map) {
            debug_options.toggle_chunk_debug_mode();
        }

        // fly mode (f key)
        if is_action_just_pressed(GameAction::ToggleFlyMode, &keyboard_input, &key_map) {
            player.toggle_fly_mode();
        }
    }

    let force_chunk_reload =
        is_action_just_pressed(GameAction::ReloadChunks, &keyboard_input, &key_map);

    // Render chunks around player
    let player_chunk = IVec3::new(
        block_to_chunk_coord(player_transform.translation.x as i32),
        block_to_chunk_coord(player_transform.translation.y as i32),
        block_to_chunk_coord(player_transform.translation.z as i32),
    );

    // If player changed chunks between this frame and the previous
    if player_chunk != *previous_player_chunk {
        let r = render_distance.distance as i32;
        let mut requested_chunks: Vec<IVec3> = Vec::new();

        for x in -r..=r {
            for z in -r..=r {
                for y in 0..=8 {
                    let chunk_pos = IVec3::new(player_chunk.x + x, y, player_chunk.z + z);
                    let chunk = world_map.map.get(&chunk_pos);

                    if chunk.is_none() {
                        requested_chunks.push(chunk_pos);
                    }
                }
            }
        }

        // Only retain chunks in the render radius
        world_map.map.retain(|pos, chunk| {
            // If chunk is empty, or not in render radius
            if !chunk_in_radius(&player_chunk, pos, r) || chunk.map.is_empty() {
                // Remove chunk, and delete its associated entity if it exists
                if let Some(entity) = chunk.entity {
                    commands.entity(entity).despawn_recursive();
                    chunk.entity = None;
                }
                false
            } else {
                true
            }
        });

        // Send a request to the server for the chunks to load
        request_world_update(
            &mut client,
            requested_chunks,
            &render_distance,
            player_chunk,
        );

        // Update player chunk position
        *previous_player_chunk = player_chunk;
    }

    // If a re-render has been requested by the player
    if force_chunk_reload {
        // Send an event to re-render all chunks loaded
        for (pos, chunk) in world_map.map.iter_mut() {
            // Despawn the chunk's entity
            if let Some(e) = chunk.entity {
                commands.entity(e).despawn_recursive();
                chunk.entity = None;
            }
            // Request a render for this chunk
            ev_writer.send(WorldRenderRequestUpdateEvent::ChunkToReload(*pos));
        }
    }

    let material_handle = &material_handle.handle;
    match *view_mode {
        ViewMode::FirstPerson => {
            // make player transparent
            if let Some(material) = materials.get_mut(material_handle) {
                material.base_color = Color::srgba(0.0, 0.0, 0.0, 0.0);
            }
        }
        ViewMode::ThirdPerson => {
            if let Some(material) = materials.get_mut(material_handle) {
                material.base_color = Color::srgba(1.0, 0.0, 0.0, 1.0);
            }
        }
    }

    let speed = if player.is_flying { 15.0 } else { 5.0 };

    let jump_velocity = 10.0;

    // flying mode
    if player.is_flying && *ui_mode == UIMode::Closed {
        if is_action_pressed(GameAction::FlyUp, &keyboard_input, &key_map) {
            player_transform.translation.y += speed * 2.0 * time.delta_secs();
        }
        if is_action_pressed(GameAction::FlyDown, &keyboard_input, &key_map) {
            player_transform.translation.y -= speed * 2.0 * time.delta_secs();
        }
    }

    // Calculate movement directions relative to the camera
    let mut forward = camera_transform.forward().xyz();
    forward.y = 0.0;

    let mut right = camera_transform.right().xyz();
    right.y = 0.0;

    let mut direction = Vec3::ZERO;

    if *ui_mode == UIMode::Closed {
        // Adjust direction based on key presses
        if is_action_pressed(GameAction::MoveBackward, &keyboard_input, &key_map) {
            direction -= forward;
        }
        if is_action_pressed(GameAction::MoveForward, &keyboard_input, &key_map) {
            direction += forward;
        }
        if is_action_pressed(GameAction::MoveLeft, &keyboard_input, &key_map) {
            direction -= right;
        }
        if is_action_pressed(GameAction::MoveRight, &keyboard_input, &key_map) {
            direction += right;
        }
    }

    // Move the player (xy plane only), only if there are no blocks and UI is closed
    if first_chunk_received.0 && direction.length_squared() > 0.0 {
        direction = direction.normalize();

        // Déplacement sur l'axe X
        let new_pos_x = player_transform.translation
            + Vec3::new(direction.x, 0.0, 0.0) * speed * time.delta_secs();

        if player.is_flying || !check_player_collision(new_pos_x, &player, &world_map) {
            player_transform.translation.x = new_pos_x.x;
        }

        // Déplacement sur l'axe Z
        let new_pos_z = player_transform.translation
            + Vec3::new(0.0, 0.0, direction.z) * speed * time.delta_secs();

        if player.is_flying || !check_player_collision(new_pos_z, &player, &world_map) {
            player_transform.translation.z = new_pos_z.z;
        }
    }

    // Handle jumping (if on the ground) and gravity, only if not flying
    if !player.is_flying {
        if player.on_ground && is_action_pressed(GameAction::Jump, &keyboard_input, &key_map) {
            // Player can jump only when grounded
            player.vertical_velocity = jump_velocity;
            player.on_ground = false;
        } else if !player.on_ground {
            // Apply gravity when the player is in the air
            player.vertical_velocity += GRAVITY * time.delta_secs();
        }
    }

    // apply gravity and verify vertical collisions
    let mut new_y = player_transform.translation.y;
    if first_chunk_received.0 {
        new_y = player_transform.translation.y + player.vertical_velocity * time.delta_secs();
    }

    // Vérifier uniquement les collisions verticales (sol et plafond)
    if check_player_collision(
        Vec3::new(
            player_transform.translation.x,
            new_y,
            player_transform.translation.z,
        ),
        &player,
        &world_map,
    ) {
        // Si un bloc est détecté sous le joueur, il reste sur le bloc
        player.on_ground = true;
        player.vertical_velocity = 0.0; // Réinitialiser la vélocité verticale si le joueur est au sol
    } else {
        // Si aucun bloc n'est détecté sous le joueur, il continue de tomber
        player_transform.translation.y = new_y;
        player.on_ground = false;
    }

    // If the player is below the world, reset their position
    const FALL_LIMIT: f32 = -50.0;
    if player_transform.translation.y < FALL_LIMIT {
        player_transform.translation = Vec3::new(0.0, 100.0, 0.0);
        player.vertical_velocity = 0.0;
    }
}
