//! The items lying in the world, as the server tells of them: a small block, or a picture of
//! the item, that turns and bobs where it lies, and flies to the player who picks it up.

use bevy::prelude::*;
use shared::{messages::DroppedItemsUpdate, world::DroppedItemId};
use std::collections::HashMap;

use crate::state::GameState;

mod animate;
mod mesh;
mod sync;

/// The local player picked up an item.
#[derive(Message, Debug, Clone, Copy)]
pub struct ItemPickedUp;

/// An item lying in the world. It is drawn by its children, which `animate` turns and bobs.
#[derive(Component, Debug)]
pub(crate) struct DroppedItemRoot {
    /// Where the server last said the item was, and how fast it was moving.
    server_position: Vec3,
    velocity: Vec3,
    /// Seconds since that update.
    since_update: f32,
    /// The copies of the item drawn, which tell how big the stack is.
    copies: usize,
}

/// The child of a `DroppedItemRoot` that turns and bobs, and holds the meshes.
#[derive(Component, Debug)]
struct DroppedItemSpin {
    /// Where in its turn and bob the item starts, so that neighbours do not move in step.
    phase: f32,
    /// How high the centre of the meshes is above the centre of the item's box when at rest.
    lift: f32,
}

/// An item that is no longer on the ground: it flies to `target`, then is gone.
#[derive(Component, Debug)]
struct FlyingTo {
    target: Entity,
    seconds_left: f32,
}

/// The entity of each item the server told of.
#[derive(Resource, Default, Debug)]
struct DroppedItemEntities(HashMap<DroppedItemId, Entity>);

pub fn dropped_items_plugin(app: &mut App) {
    app.add_message::<DroppedItemsUpdate>()
        .add_message::<ItemPickedUp>()
        .init_resource::<DroppedItemEntities>()
        .init_resource::<mesh::ItemLooks>()
        .add_systems(
            Update,
            (
                sync::apply_dropped_items_updates,
                animate::follow_the_server,
                animate::spin_and_bob,
                animate::fly_to_target,
            )
                .chain()
                .run_if(in_state(GameState::Game)),
        )
        // The entities go with the game state; the ids are those of the session that ended
        .add_systems(
            OnExit(GameState::Game),
            |mut entities: ResMut<DroppedItemEntities>| entities.0.clear(),
        );
}
