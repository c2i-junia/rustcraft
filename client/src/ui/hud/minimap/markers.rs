//! What the minimap draws over its image: the arrow of the player, turned by the camera, the
//! dots of the other players and of the mobs, and the coordinates of the player under it.

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use shared::players::Player;

use super::compose::MAP_SIZE;
use super::{column_of, MinimapCoords, MinimapImage, MinimapView};
use crate::camera::CameraController;
use crate::mob::MobRoot;
use crate::player::CurrentPlayerMarker;
use crate::ui::theme::{ACCENT, BEVEL_LO, DANGER, TEXT};

/// The arrow of the player pointing north, from its top row: `#` its outline, `a` its fill.
const ARROW: [&str; 9] = [
    "...#...", "..#a#..", "..#a#..", ".#aaa#.", ".#aaa#.", "#aaaaa#", "#aa#aa#", "#a#.#a#",
    "##...##",
];
/// The arrow is drawn at twice the size of its pixels.
const ARROW_SCALE: f32 = 2.0;

/// The side of a dot, its outline included, in logical px.
const DOT_SIZE: f32 = 5.0;

/// The arrow of the player.
#[derive(Component)]
pub struct MinimapArrow;

/// The dot of a player or of a mob.
#[derive(Component)]
pub struct MinimapDot {
    pub target: Entity,
}

fn srgba(color: Color) -> [u8; 4] {
    color.to_srgba().to_u8_array()
}

/// Spawns the arrow at the centre of `view`.
pub fn spawn_arrow(commands: &mut Commands, images: &mut Assets<Image>, view: Entity) {
    let width = ARROW[0].len();
    let pixels: Vec<u8> = ARROW
        .iter()
        .flat_map(|row| row.chars())
        .flat_map(|pixel| match pixel {
            '#' => srgba(BEVEL_LO),
            'a' => srgba(ACCENT),
            _ => [0; 4],
        })
        .collect();
    let mut image = Image::new(
        Extent3d {
            width: width as u32,
            height: ARROW.len() as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    let size = Vec2::new(width as f32, ARROW.len() as f32) * ARROW_SCALE;
    commands.spawn((
        MinimapArrow,
        ImageNode::new(images.add(image)),
        Node {
            position_type: PositionType::Absolute,
            width: px(size.x),
            height: px(size.y),
            left: px((MAP_SIZE as f32 - size.x) / 2.0),
            top: px((MAP_SIZE as f32 - size.y) / 2.0),
            ..default()
        },
        ChildOf(view),
    ));
}

/// The angle of a view looking along `forward`, clockwise from the north (-Z), in radians.
pub fn heading(forward: Vec3) -> f32 {
    forward.x.atan2(-forward.z)
}

/// Turns the arrow the way the camera looks.
pub fn point_minimap_arrow(
    camera: Query<&GlobalTransform, With<CameraController>>,
    mut arrows: Query<&mut UiTransform, With<MinimapArrow>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };
    let rotation = Rot2::radians(heading(camera.forward().into()));
    for mut transform in &mut arrows {
        if transform.rotation != rotation {
            transform.rotation = rotation;
        }
    }
}

/// Where the centre of something at `position` falls on the map centred on the column `center`
/// at `zoom` blocks a pixel, in logical px from its top-left corner.
pub fn map_offset(position: Vec3, center: IVec2, zoom: i32) -> Vec2 {
    let half = MAP_SIZE as f32 / 2.0;
    Vec2::splat(half) + (position.xz() - center.as_vec2()) / zoom as f32
}

/// The top-left corner of a dot centred at `offset` on the map, or `None` when it falls beyond
/// the map and is not `held`. A dot that is held stays at the edge, toward where it is.
pub fn dot_corner(offset: Vec2, held: bool) -> Option<Vec2> {
    let corner = offset - Vec2::splat(DOT_SIZE / 2.0);
    let max = Vec2::splat(MAP_SIZE as f32 - DOT_SIZE);
    if held {
        Some(corner.clamp(Vec2::ZERO, max))
    } else {
        (corner.cmpge(Vec2::ZERO).all() && corner.cmple(max).all()).then_some(corner)
    }
}

/// Puts a dot on the map for every other player, held at its edge, and for every mob within it,
/// and takes away the dots of those gone.
#[allow(clippy::type_complexity)]
pub fn place_minimap_dots(
    mut commands: Commands,
    image: Option<Res<MinimapImage>>,
    view: Query<Entity, With<MinimapView>>,
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    others: Query<(Entity, &GlobalTransform), (With<Player>, Without<CurrentPlayerMarker>)>,
    mobs: Query<(Entity, &GlobalTransform), With<MobRoot>>,
    mut dots: Query<(Entity, &MinimapDot, &mut Node, &mut Visibility)>,
) {
    let (Some(image), Ok(view), Ok(player)) = (image, view.single(), player.single()) else {
        return;
    };
    let center = column_of(player.translation);
    let zoom = image.blocks_per_pixel();
    let targets: Vec<(Entity, Vec3, bool)> = others
        .iter()
        .map(|(entity, transform)| (entity, transform.translation(), true))
        .chain(
            mobs.iter()
                .map(|(entity, transform)| (entity, transform.translation(), false)),
        )
        .collect();

    let mut placed = Vec::new();
    for (dot, MinimapDot { target }, mut node, mut visibility) in &mut dots {
        let Some(&(_, position, held)) = targets.iter().find(|(entity, ..)| entity == target)
        else {
            commands.entity(dot).despawn();
            continue;
        };
        placed.push(*target);
        match dot_corner(map_offset(position, center, zoom), held) {
            Some(corner) => {
                // A write marks the node changed, which lays the UI out again: only on a move
                if node.left != px(corner.x) || node.top != px(corner.y) {
                    node.left = px(corner.x);
                    node.top = px(corner.y);
                }
                visibility.set_if_neq(Visibility::Inherited);
            }
            None => {
                visibility.set_if_neq(Visibility::Hidden);
            }
        }
    }
    for &(target, position, held) in &targets {
        if placed.contains(&target) {
            continue;
        }
        let corner = dot_corner(map_offset(position, center, zoom), held);
        let color = if held { TEXT } else { DANGER };
        commands.spawn((
            MinimapDot { target },
            Node {
                position_type: PositionType::Absolute,
                width: px(DOT_SIZE),
                height: px(DOT_SIZE),
                left: px(corner.unwrap_or_default().x),
                top: px(corner.unwrap_or_default().y),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(color),
            BorderColor::all(BEVEL_LO),
            if corner.is_some() {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
            ChildOf(view),
        ));
    }
}

/// Writes the coordinates of the player under the map, in whole blocks.
pub fn write_minimap_coords(
    player: Query<&Transform, With<CurrentPlayerMarker>>,
    mut coords: Query<&mut Text, With<MinimapCoords>>,
) {
    let Ok(player) = player.single() else {
        return;
    };
    let block = player.translation.floor().as_ivec3();
    let line = format!("X {}  Y {}  Z {}", block.x, block.y, block.z);
    for mut text in &mut coords {
        if text.0 != line {
            text.0.clone_from(&line);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::{FRAC_PI_2, PI};

    use super::*;

    #[test]
    fn the_heading_turns_clockwise_from_the_north() {
        assert!((heading(Vec3::NEG_Z) - 0.0).abs() < 1e-6);
        assert!((heading(Vec3::X) - FRAC_PI_2).abs() < 1e-6);
        assert!((heading(Vec3::new(-1.0, -0.5, 0.0)) + FRAC_PI_2).abs() < 1e-6);
        assert!((heading(Vec3::Z).abs() - PI).abs() < 1e-6);
    }

    #[test]
    fn a_dot_is_placed_by_the_zoom_and_held_at_the_edge_if_it_must() {
        let center = IVec2::new(10, -20);
        // the player's own column is the centre of the map
        assert_eq!(
            map_offset(Vec3::new(10.0, 70.0, -20.0), center, 1),
            Vec2::splat(64.0)
        );
        // 8 blocks east and 4 south, two blocks a pixel
        assert_eq!(
            map_offset(Vec3::new(18.0, 0.0, -16.0), center, 2),
            Vec2::new(68.0, 66.0)
        );

        assert_eq!(
            dot_corner(Vec2::splat(64.0), false),
            Some(Vec2::splat(61.5))
        );
        // beyond the map to the north-east: held at its corner, or not shown
        let beyond = Vec2::new(500.0, -30.0);
        assert_eq!(dot_corner(beyond, true), Some(Vec2::new(123.0, 0.0)));
        assert_eq!(dot_corner(beyond, false), None);
    }
}
