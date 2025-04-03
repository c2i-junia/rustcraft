use bevy::{prelude::*, render::mesh::VertexAttributeValues};
use shared::{
    messages::ItemStackUpdateEvent,
    world::{ItemStack, ItemType},
    CHUNK_SIZE,
};

use crate::{
    player::CurrentPlayerMarker,
    world::{MaterialResource, RenderDistance},
};

#[derive(Debug, Component)]
pub struct StackMarker {
    pub id: u128,
    pub stack: ItemStack,
}

pub fn stack_update_system(
    mut events: EventReader<ItemStackUpdateEvent>,
    mut commands: Commands,
    mut stacks: Query<(Entity, &mut StackMarker, &mut Transform), Without<CurrentPlayerMarker>>,
    mut meshes: ResMut<Assets<Mesh>>,
    time: Res<Time>,
    material_resource: Res<MaterialResource>,
    distance: Res<RenderDistance>,
    player_pos: Query<&Transform, With<CurrentPlayerMarker>>,
) {
    'ev_loop: for ev in events.read() {
        if let Some((stack, pos)) = ev.data {
            for (_, mut marker, mut transform) in stacks.iter_mut() {
                if marker.id == ev.id {
                    transform.translation = pos;
                    marker.stack = stack;
                    continue 'ev_loop;
                }
            }

            let mut mesh = Cuboid::from_size(if let ItemType::Block(_) = stack.item_type {
                Vec3::new(0.2, 0.2, 0.2)
            } else {
                Vec3::new(0.2, 0.2, 0.05)
            })
            .mesh()
            .build();

            let uv_attribute = mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0).unwrap();

            let VertexAttributeValues::Float32x2(uv_attribute) = uv_attribute else {
                panic!("Unexpected vertex format, expected Float32x2.");
            };

            if let Some(uv_coords) = material_resource
                .items
                .as_ref()
                .unwrap()
                .uvs
                .get(&format!("{:?}", stack.item_id))
            {
                for uv in uv_attribute.iter_mut() {
                    uv[0] = uv[0].clamp(uv_coords.u0, uv_coords.u1);
                    uv[1] = uv[1].clamp(uv_coords.v0, uv_coords.v1);
                }
            }

            // If no stack exists with this id, we have to create one
            commands.spawn((
                StackMarker { id: ev.id, stack },
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(
                    material_resource
                        .global_materials
                        .get(&crate::world::GlobalMaterial::Items)
                        .unwrap()
                        .clone_weak(),
                ),
                Transform::from_translation(pos),
            ));
        } else {
            for (entity, marker, _) in stacks.iter() {
                if marker.id == ev.id {
                    commands.entity(entity).despawn_recursive();
                    continue 'ev_loop;
                }
            }
        }
    }

    for (e, _, mut transform) in stacks.iter_mut() {
        if player_pos
            .single()
            .translation
            .distance(transform.translation)
            > distance.distance as f32 * CHUNK_SIZE as f32
        {
            commands.entity(e).despawn_recursive();
        } else {
            transform.rotate_local_y(1.0 * time.delta_secs());
        }
    }
}
