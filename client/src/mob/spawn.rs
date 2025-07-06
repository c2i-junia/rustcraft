use bevy::prelude::*;
use shared::messages::mob::MobUpdateEvent;

use crate::{mob::setup_fox, player::CurrentPlayerMarker, world::RenderDistance};

use super::MobRoot;

pub fn spawn_mobs_system(
    mut ev_update: EventReader<MobUpdateEvent>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut mobs: Query<(Entity, &MobRoot, &mut Transform), Without<CurrentPlayerMarker>>,
    player_pos: Query<&Transform, With<CurrentPlayerMarker>>,
    render_distance: Res<RenderDistance>,
) {
    let player_pos = player_pos.single().unwrap().translation;

    'event_loop: for event in ev_update.read() {
        let id = event.id;

        let position = event.mob.position;

        for (_, mob, mut transform) in mobs.iter_mut() {
            if mob.id == id {
                transform.translation = position;
                transform.rotation = event.mob.rotation;
                continue 'event_loop;
            }
        }

        if event.mob.kind == shared::world::MobKind::Fox
            && event.mob.position.distance(player_pos) < render_distance.distance as f32 * 5.0
        {
            info!("Spawning fox at {:?}", position);
            setup_fox(id, position, &mut commands, &asset_server, &mut graphs);
        }
    }

    // Despawn entities which are too far away
    for (entity, _, transform) in mobs.iter() {
        if transform.translation.distance(player_pos) > render_distance.distance as f32 * 5.0 {
            commands.entity(entity).despawn();
        }
    }
}
