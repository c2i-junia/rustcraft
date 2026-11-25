//! The armour on the models of the players: boxes around the parts of the body, larger than the
//! layers of the skin and cut from the texture of their material, laid out as a skin
//! (`scripts/generate_armour.py`). The armour of the local player is read from its inventory,
//! the others' is sent by the server (`PlayerUpdateEvent::worn`).

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use shared::players::armour::{Armour, ArmourMaterial, ArmourPiece, WornArmour};
use shared::players::{Inventory, Player};

use super::model::{box_mesh, skin_material, BodyPart, MODEL_PIXELS};
use super::CurrentPlayerMarker;

/// The armour a player wears, which its model shows.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Worn(pub WornArmour);

/// A box of armour on a part of a model.
#[derive(Component, Debug)]
pub struct WornMesh;

/// Where the texture of a material worn is.
fn layer_path(material: ArmourMaterial) -> String {
    format!("./graphics/armour/{material:?}.png")
}

/// The boxes of `piece`: the part each covers, the origin of its faces in the texture and by
/// how many pixels it stands out of the body: above the skin's second layer, the chestplate and
/// the boots above the leggings. Each of them its own distance, as they cross at the shoulders.
fn boxes(piece: ArmourPiece) -> Vec<(BodyPart, UVec2, f32)> {
    use BodyPart::*;
    let parts: &[(BodyPart, f32)] = match piece {
        ArmourPiece::Helmet => &[(Head, 1.0)],
        ArmourPiece::Chestplate => &[(Torso, 0.9), (RightArm, 0.8), (LeftArm, 0.8)],
        ArmourPiece::Leggings => &[(Torso, 0.55), (RightLeg, 0.5), (LeftLeg, 0.45)],
        ArmourPiece::Boots => &[(RightLeg, 0.8), (LeftLeg, 0.75)],
    };
    parts
        .iter()
        .map(|&(part, inflate)| {
            let (first, second) = part.skin_origins();
            let origin = match piece {
                ArmourPiece::Leggings => second,
                _ => first,
            };
            (part, origin, inflate)
        })
        .collect()
}

/// Reads the armour of the local player from its inventory: it is predicted, as the rest of it.
pub fn track_the_local_armour(
    inventory: Res<Inventory>,
    mut player: Query<&mut Worn, With<CurrentPlayerMarker>>,
) {
    for mut worn in &mut player {
        worn.set_if_neq(Worn(inventory.worn()));
    }
}

/// Dresses the model of each player whose armour changed in what it wears.
pub fn dress_models(
    mut commands: Commands,
    players: Query<(&Player, &Worn, &Children), Changed<Worn>>,
    children: Query<&Children>,
    parts: Query<(&BodyPart, Option<&RenderLayers>)>,
    dressed: Query<(), With<WornMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    assets: Res<AssetServer>,
) {
    for (player, worn, player_children) in &players {
        let scale = player.height / MODEL_PIXELS;
        let worn: Vec<Armour> = worn
            .0
             .0
            .iter()
            .filter_map(|item| item.as_ref()?.armour())
            .collect();
        let clothes: Vec<_> = worn
            .into_iter()
            .map(|armour| {
                let texture = assets.load(layer_path(armour.material));
                (armour, materials.add(skin_material(texture)))
            })
            .collect();
        for body in player_children {
            for entity in children.iter_descendants(*body) {
                if dressed.contains(entity) {
                    commands.entity(entity).despawn();
                }
                let Ok((part, layers)) = parts.get(entity) else {
                    continue;
                };
                let layers = layers.cloned().unwrap_or_default();
                for (armour, material) in &clothes {
                    let on_part = boxes(armour.piece).into_iter().filter(|(p, ..)| p == part);
                    for (part, origin, inflate) in on_part {
                        commands.spawn((
                            WornMesh,
                            Mesh3d(meshes.add(box_mesh(origin, part.size(), inflate, scale))),
                            MeshMaterial3d(material.clone()),
                            Transform::from_translation(part.offset() * scale),
                            layers.clone(),
                            ChildOf(entity),
                        ));
                    }
                }
            }
        }
    }
}
