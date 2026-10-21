//! The server's updates of the dropped items, turned into entities.

use bevy::prelude::*;
use shared::{
    messages::{DroppedItemState, DroppedItemsUpdate, ItemRemoval},
    players::Player,
    world::{DroppedItemId, DROPPED_ITEM_SIZE},
};

use super::{
    mesh::{copies, copy_offset, ItemLook, ItemLooks},
    DroppedItemEntities, DroppedItemRoot, DroppedItemSpin, FlyingTo, ItemPickedUp,
};
use crate::{network::CurrentPlayerProfile, state::GameState, world::MaterialResource};

/// How long an item picked up or merged takes to fly to its target, in seconds.
const FLIGHT_SECONDS: f32 = 0.15;

pub(super) fn apply_dropped_items_updates(
    mut commands: Commands,
    mut updates: MessageReader<DroppedItemsUpdate>,
    mut entities: ResMut<DroppedItemEntities>,
    mut looks: ResMut<ItemLooks>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<MaterialResource>,
    images: Res<Assets<Image>>,
    mut roots: Query<(&mut DroppedItemRoot, &mut Transform)>,
    players: Query<(Entity, &Player)>,
    profile: Res<CurrentPlayerProfile>,
    mut picked_up: MessageWriter<ItemPickedUp>,
) {
    for update in updates.read() {
        // The upserts first: an item may merge into one this update tells of
        for (id, state) in &update.upserts {
            let mut shown_at = state.position;
            if let Some(entity) = entities.0.remove(id) {
                if let Ok((mut root, mut transform)) = roots.get_mut(entity) {
                    follow_update(&mut root, &mut transform, state);
                    if root.copies == copies(state.stack.nb) {
                        entities.0.insert(*id, entity);
                        continue;
                    }
                    // A stack that grew or shrank is drawn again, from where it was shown
                    shown_at = transform.translation;
                }
                commands.entity(entity).despawn();
            }
            let Some(look) = looks.get(state.stack.item_id, &materials, &mut meshes, &images)
            else {
                continue;
            };
            let entity = spawn_item(&mut commands, *id, state, &look, shown_at);
            entities.0.insert(*id, entity);
        }

        for (id, removal) in &update.removals {
            let Some(entity) = entities.0.remove(id) else {
                continue;
            };
            let target = match removal {
                ItemRemoval::Gone => None,
                ItemRemoval::PickedUp(player_id) => {
                    if *player_id == profile.id {
                        picked_up.write(ItemPickedUp);
                    }
                    players
                        .iter()
                        .find(|(_, player)| player.id == *player_id)
                        .map(|(entity, _)| entity)
                }
                ItemRemoval::Merged(into) => entities.0.get(into).copied(),
            };
            match target {
                Some(target) => {
                    commands
                        .entity(entity)
                        .remove::<DroppedItemRoot>()
                        .insert(FlyingTo {
                            target,
                            seconds_left: FLIGHT_SECONDS,
                        });
                }
                None => commands.entity(entity).despawn(),
            }
        }
    }
}

/// Takes the update `state` of the item `root`, shown at `transform`: the item moves on from
/// where it is shown toward where the server says (`animate::follow_the_server`), but one that
/// went through a portal since the last update (`DroppedItemState::passes`) is put there at
/// once, rather than eased across the gap between the portals.
fn follow_update(root: &mut DroppedItemRoot, transform: &mut Transform, state: &DroppedItemState) {
    root.server_position = state.position;
    root.velocity = state.velocity;
    root.since_update = 0.0;
    if root.passes != state.passes {
        root.passes = state.passes;
        transform.translation = state.position;
    }
}

/// An item shown at `shown_at`, which then moves to where the server says it is, with as
/// many copies of its look as its stack shows.
fn spawn_item(
    commands: &mut Commands,
    id: DroppedItemId,
    state: &DroppedItemState,
    look: &ItemLook,
    shown_at: Vec3,
) -> Entity {
    let size = look.size();
    let copies = copies(state.stack.nb);
    let spin = DroppedItemSpin {
        // The golden angle spreads the phases of the ids evenly
        phase: id as f32 * 2.399_963,
        // A look bigger than the box would sink into the ground
        lift: (size - DROPPED_ITEM_SIZE).max(0.0) / 2.0,
    };
    commands
        .spawn((
            Name::new(format!("Dropped item {id}")),
            DroppedItemRoot {
                server_position: state.position,
                velocity: state.velocity,
                since_update: 0.0,
                copies,
                passes: state.passes,
            },
            Transform::from_translation(shown_at),
            Visibility::default(),
            DespawnOnExit(GameState::Game),
        ))
        .with_children(|root| {
            root.spawn((spin, Transform::default(), Visibility::default()))
                .with_children(|spin| {
                    for index in 0..copies {
                        let transform = Transform::from_translation(copy_offset(index) * size)
                            .with_scale(Vec3::splat(size));
                        match look {
                            ItemLook::Cube(mesh, material) => spin.spawn((
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material.clone()),
                                transform,
                            )),
                            ItemLook::Sprite(mesh, material) => spin.spawn((
                                Mesh3d(mesh.clone()),
                                MeshMaterial3d(material.clone()),
                                transform,
                            )),
                        };
                    }
                });
        })
        .id()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{ItemId, ItemStack};

    fn state(position: Vec3, passes: u32) -> DroppedItemState {
        DroppedItemState {
            stack: ItemStack {
                item_id: ItemId::Dirt,
                item_type: ItemId::Dirt.get_default_type(),
                nb: 1,
            },
            position,
            velocity: Vec3::new(-8.0, 0.0, 0.0),
            passes,
        }
    }

    #[test]
    fn an_item_that_went_through_a_portal_is_put_on_the_other_side_at_once() {
        let shown = Vec3::new(1.0, 64.2, -0.5);
        let mut root = DroppedItemRoot {
            server_position: shown,
            velocity: Vec3::NEG_Y * 8.0,
            since_update: 0.05,
            copies: 1,
            passes: 0,
        };
        let mut transform = Transform::from_translation(shown);

        // A move on the same side is eased: the item stays where it is shown for now
        follow_update(&mut root, &mut transform, &state(shown - Vec3::Y * 0.4, 0));
        assert_eq!(transform.translation, shown);
        assert_eq!(root.server_position, shown - Vec3::Y * 0.4);

        // Out of a portal a block away, closer than an item ever jumps by itself
        let out = Vec3::new(2.4, 64.6, 0.5);
        follow_update(&mut root, &mut transform, &state(out, 1));
        assert_eq!(transform.translation, out);
        assert_eq!((root.server_position, root.passes), (out, 1));
        assert_eq!(root.velocity, Vec3::new(-8.0, 0.0, 0.0));
    }
}
