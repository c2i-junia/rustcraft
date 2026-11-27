//! The furniture as the server tells of it (`shared::world::decor`): the item shown in each item
//! frame, the plant in each flower pot, and the patterns dyed on each banner, drawn as entities
//! over the blocks, which the chunk meshes draw.

use std::collections::HashMap;
use std::f32::consts::FRAC_PI_4;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use shared::world::{BannerPatterns, BlockDirection, DecorKind, Decorations, Shown};

use crate::dropped_items::mesh::{ItemLook, ItemLooks};
use crate::state::GameState;
use crate::world::{ClientWorldMap, MaterialResource};

/// The cloth of a banner, in pixels: the part of its cell the patterns are drawn on.
pub const CLOTH_WIDTH: u32 = 12;
pub const CLOTH_HEIGHT: u32 = 14;
/// How big an item framed is drawn, in blocks, and a plant potted.
const FRAMED_SIZE: f32 = 0.5;
const POTTED_SIZE: f32 = 0.55;

/// What the server last said the frames, the pots and the banners show.
#[derive(Resource, Default, Debug)]
pub struct ClientDecorations {
    pub decorations: Decorations,
    /// Grows with each message, so that what is drawn is looked at again.
    revision: u64,
}

impl ClientDecorations {
    pub fn set(&mut self, decorations: Decorations) {
        self.decorations = decorations;
        self.revision += 1;
    }
}

/// What an entity of the furniture draws, by the cell it is drawn in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drawn {
    Shown(Shown, DecorKind, BlockDirection),
    Banner(BannerPatterns, BlockDirection),
}

#[derive(Component)]
struct DecorEntity;

/// The pixels of the patterns of a banner, RGBA rows from the top: each layer over the ones
/// before, the rest left clear for the cloth of the block to show.
pub fn banner_pixels(patterns: &BannerPatterns) -> Vec<u8> {
    let mut pixels = vec![0u8; (CLOTH_WIDTH * CLOTH_HEIGHT * 4) as usize];
    for (pattern, color) in patterns.layers() {
        let [r, g, b] = color.rgb();
        for y in 0..CLOTH_HEIGHT {
            for x in 0..CLOTH_WIDTH {
                if !pattern.covers(x, y, CLOTH_WIDTH, CLOTH_HEIGHT) {
                    continue;
                }
                // a little grain, as the wool has
                let shade = if (x * 7 + y * 13) % 5 == 0 { 0.9 } else { 1.0 };
                let i = ((y * CLOTH_WIDTH + x) * 4) as usize;
                let tint = |c: u8| (f32::from(c) * shade) as u8;
                pixels[i..i + 4].copy_from_slice(&[tint(r), tint(g), tint(b), 255]);
            }
        }
    }
    pixels
}

/// The point `local` of a cell turned to the front, turned to `direction` about the centre of
/// the cell, as the chunk meshes turn a block.
fn turned(local: Vec3, direction: BlockDirection) -> Vec3 {
    let [x, z] = direction.turn([local.x - 0.5, local.z - 0.5]);
    Vec3::new(x + 0.5, local.y, z + 0.5)
}

/// The rotation that turns +Z, the face of a picture, away from the wall on the side
/// `direction` of its cell.
fn facing_out(direction: BlockDirection) -> Quat {
    let out = -direction.offset().as_vec3();
    Quat::from_rotation_y(out.x.atan2(out.z))
}

/// Spawns the meshes of `look` at each of `transforms` under a new entity.
fn spawn_look(commands: &mut Commands, look: &ItemLook, transforms: &[Transform]) -> Entity {
    let mut root = commands.spawn((
        DecorEntity,
        Transform::default(),
        Visibility::default(),
        DespawnOnExit(GameState::Game),
    ));
    root.with_children(|root| {
        for &transform in transforms {
            match look {
                ItemLook::Cube(mesh, material) => {
                    root.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        transform,
                    ));
                }
                ItemLook::Sprite(mesh, material) => {
                    root.spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        transform,
                    ));
                }
            }
        }
    });
    root.id()
}

/// Draws what each frame and pot shows and the patterns of each banner, again where it
/// changed, and takes away what is no longer there.
#[allow(clippy::too_many_arguments)]
fn place_decorations(
    mut commands: Commands,
    decorations: Res<ClientDecorations>,
    world: Res<ClientWorldMap>,
    mut looks: ResMut<ItemLooks>,
    materials: Res<MaterialResource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
    entities: Query<(), With<DecorEntity>>,
    mut drawn: Local<HashMap<IVec3, (Drawn, Entity)>>,
    mut cloth: Local<Option<Handle<Mesh>>>,
) {
    let kind_at = |at: IVec3| {
        world
            .blocks
            .get_block(at)
            .and_then(|block| Some((block.id.decor_kind()?, block.direction)))
    };
    let mut wanted: HashMap<IVec3, Drawn> = HashMap::new();
    for &(at, shown) in &decorations.decorations.displays {
        if let Some((kind @ (DecorKind::ItemFrame | DecorKind::FlowerPot), direction)) = kind_at(at)
        {
            wanted.insert(at, Drawn::Shown(shown, kind, direction));
        }
    }
    for &(at, patterns) in &decorations.decorations.banners {
        if let Some((DecorKind::Banner, direction)) = kind_at(at) {
            wanted.insert(at, Drawn::Banner(patterns, direction));
        }
    }
    // gone or changed
    drawn.retain(|at, (what, entity)| {
        let keep = wanted.get(at) == Some(what);
        if !keep {
            commands.entity(*entity).despawn();
        }
        keep
    });
    // a new game left none of those drawn
    if drawn
        .values()
        .any(|(_, entity)| entities.get(*entity).is_err())
    {
        drawn.clear();
    }
    for (at, what) in wanted {
        if drawn.contains_key(&at) {
            continue;
        }
        let entity = match what {
            Drawn::Shown(shown, kind, direction) => {
                let Some(look) = looks.get(shown.item, &materials, &mut meshes, &images) else {
                    continue;
                };
                let cube = matches!(look, ItemLook::Cube(..));
                let transforms = match kind {
                    DecorKind::ItemFrame => {
                        let size = if cube { FRAMED_SIZE * 0.6 } else { FRAMED_SIZE };
                        let depth = if cube { size / 2.0 } else { 1.0 / 64.0 };
                        let centre = turned(Vec3::new(0.5, 0.5, 1.0 / 16.0 + depth), direction);
                        let roll = Quat::from_rotation_z(-f32::from(shown.turn) * FRAC_PI_4);
                        vec![Transform::from_translation(at.as_vec3() + centre)
                            .with_rotation(facing_out(direction) * roll)
                            .with_scale(Vec3::splat(size))]
                    }
                    _ => {
                        let size = if cube { 0.3 } else { POTTED_SIZE };
                        let centre = at.as_vec3() + Vec3::new(0.5, 6.0 / 16.0 + size / 2.0, 0.5);
                        let crossed: &[f32] = if cube {
                            &[0.0]
                        } else {
                            &[FRAC_PI_4, -FRAC_PI_4]
                        };
                        crossed
                            .iter()
                            .map(|&angle| {
                                Transform::from_translation(centre)
                                    .with_rotation(Quat::from_rotation_y(angle))
                                    .with_scale(Vec3::splat(size))
                            })
                            .collect()
                    }
                };
                spawn_look(&mut commands, &look, &transforms)
            }
            Drawn::Banner(patterns, direction) => {
                let mut image = Image::new(
                    Extent3d {
                        width: CLOTH_WIDTH,
                        height: CLOTH_HEIGHT,
                        depth_or_array_layers: 1,
                    },
                    TextureDimension::D2,
                    banner_pixels(&patterns),
                    TextureFormat::Rgba8UnormSrgb,
                    RenderAssetUsages::default(),
                );
                image.sampler = ImageSampler::nearest();
                let mesh = cloth
                    .get_or_insert_with(|| {
                        meshes.add(Rectangle::new(
                            CLOTH_WIDTH as f32 / 16.0,
                            CLOTH_HEIGHT as f32 / 16.0,
                        ))
                    })
                    .clone();
                let centre = turned(Vec3::new(0.5, 7.0 / 16.0, 2.0 / 16.0 + 0.003), direction);
                commands
                    .spawn((
                        DecorEntity,
                        Mesh3d(mesh),
                        MeshMaterial3d(standard.add(StandardMaterial {
                            base_color_texture: Some(images.add(image)),
                            alpha_mode: AlphaMode::Mask(0.5),
                            unlit: true,
                            ..default()
                        })),
                        Transform::from_translation(at.as_vec3() + centre)
                            .with_rotation(facing_out(direction)),
                        DespawnOnExit(GameState::Game),
                    ))
                    .id()
            }
        };
        drawn.insert(at, (what, entity));
    }
}

pub fn decor_plugin(app: &mut App) {
    app.init_resource::<ClientDecorations>()
        .init_resource::<ItemLooks>()
        .add_systems(OnExit(GameState::Game), |mut commands: Commands| {
            commands.insert_resource(ClientDecorations::default());
        })
        .add_systems(Update, place_decorations.run_if(in_state(GameState::Game)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::world::{BannerPattern, DyeColor};

    #[test]
    fn a_banner_without_patterns_is_clear_and_a_stripe_colours_its_top() {
        assert!(banner_pixels(&BannerPatterns::default())
            .iter()
            .all(|&p| p == 0));
        let striped = BannerPatterns::default()
            .with(BannerPattern::Chief, DyeColor::Red)
            .unwrap();
        let pixels = banner_pixels(&striped);
        let alpha = |x: u32, y: u32| pixels[((y * CLOTH_WIDTH + x) * 4 + 3) as usize];
        assert_eq!(alpha(5, 0), 255);
        assert_eq!(alpha(5, CLOTH_HEIGHT - 1), 0);
    }

    #[test]
    fn a_picture_on_a_wall_faces_away_from_it() {
        for direction in [
            BlockDirection::Front,
            BlockDirection::Right,
            BlockDirection::Back,
            BlockDirection::Left,
        ] {
            let face = facing_out(direction) * Vec3::Z;
            assert!(
                face.distance(-direction.offset().as_vec3()) < 1e-5,
                "{direction:?}"
            );
            // the front of the plate lies toward the wall
            let near_wall = turned(Vec3::new(0.5, 0.5, 0.0), direction) - Vec3::splat(0.5);
            assert!((near_wall.with_y(0.0) - direction.offset().as_vec3() * 0.5).length() < 1e-5);
        }
    }
}
