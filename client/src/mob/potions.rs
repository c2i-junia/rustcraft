//! The splash potions (`MobKind::ThrownPotion`): where one bursts, drops of its colour fly out
//! (`shared::world::alchemy`), as the server gives its effect around.

use bevy::prelude::*;
use rand::Rng;
use shared::world::ItemId;
use std::collections::HashMap;

use crate::portal::particles::spawn_particle;

/// A splash potion burst at `at`.
#[derive(Message)]
pub struct PotionBurst {
    pub at: Vec3,
    pub potion: Option<ItemId>,
}

/// The mesh of the drops, and their material for each colour, made with the first.
#[derive(Resource, Default)]
pub struct Drops {
    mesh: Option<Handle<Mesh>>,
    materials: HashMap<[u8; 3], Handle<StandardMaterial>>,
}

/// The drops of a burst.
const DROPS: usize = 24;
/// The colour of a burst whose potion is not known.
const GREY: [u8; 3] = [200, 200, 210];

/// Bursts drops of the colour of each splash potion that broke.
pub fn show_bursts(
    mut commands: Commands,
    mut bursts: MessageReader<PotionBurst>,
    mut drops: ResMut<Drops>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<StandardMaterial>>>,
) {
    let (Some(mut meshes), Some(mut materials)) = (meshes, materials) else {
        bursts.clear();
        return;
    };
    let mut rng = rand::thread_rng();
    for burst in bursts.read() {
        let color = burst
            .potion
            .and_then(|potion| potion.potion_color())
            .unwrap_or(GREY);
        let mesh = drops
            .mesh
            .get_or_insert_with(|| meshes.add(Cuboid::from_length(1.0)))
            .clone();
        let material = drops
            .materials
            .entry(color)
            .or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgba_u8(color[0], color[1], color[2], 230),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    ..default()
                })
            })
            .clone();
        for _ in 0..DROPS {
            let out = Vec3::new(
                rng.gen_range(-1.0..1.0),
                rng.gen_range(0.2..1.4),
                rng.gen_range(-1.0..1.0),
            );
            commands.queue(spawn_particle(
                mesh.clone(),
                material.clone(),
                burst.at + out * 0.2,
                rng.gen_range(0.4..0.9),
                rng.gen_range(0.06..0.12),
                out * 3.0,
            ));
        }
    }
}
