//! The portals, in a world. The view through them is tested with portals and walls put into
//! the client's map, as décor the server knows nothing of; the portal gun places them with the
//! server.

use std::f32::consts::PI;
use std::time::{Duration, Instant};

use bevy::{camera::RenderTarget, prelude::*, time::TimeUpdateStrategy};
use image::RgbImage;
use shared::world::{
    BlockData, BlockDirection, BlockId, FaceDirection, Portal, PortalColor, PORTAL_HEIGHT,
    PORTAL_WIDTH,
};
use shared::{CHUNK_SIZE, DAY_DURATION_IN_TICKS, NOON_IN_DAY};

use super::game::{enter_world, enter_world_with, hold_click, slot};
use super::harness::{Harness, Options, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::CurrentPlayerMarker;
use crate::portal::material::PortalMaterial;
use crate::portal::projection::MIN_OBLIQUE_DISTANCE;
use crate::portal::shot::{Bolt, ShotFired};
use crate::portal::PortalCamera;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::celestial::{sky_rotation, sun_height, CelestialRoot, Stars};
use crate::world::time::ClientTime;
use crate::world::{ClientWorldMap, GraphicsSettings, ShaderQuality};
use shared::players::blocks::PortalShot;
use shared::players::Player;
use shared::world::{global_block_to_chunk_pos, raycast_far, ItemId};

/// The scene, in blocks from the cell of the player's feet.
struct Scene {
    feet: IVec3,
    blue: Portal,
    orange: Portal,
}

/// Through the blue portal, the player's view moves this far: the orange portal faces the other
/// way, 12 blocks east and 5 north.
const THROUGH: Vec3 = Vec3::new(12.0, 0.0, -5.0);

/// Waits for the player to stand on the ground with every chunk of the scenes loaded around it,
/// and returns the cell of its feet.
fn settle(h: &mut Harness) -> IVec3 {
    h.wait_until("the player on the ground", SERVER, |w| {
        let mut players = w
            .try_query_filtered::<&Transform, With<CurrentPlayerMarker>>()
            .unwrap();
        let map = &w.resource::<ClientWorldMap>().blocks;
        players.iter(w).any(|player| {
            let center = player.translation.floor().as_ivec3();
            // Every chunk the scene touches is loaded
            let corners = [-14, 24].into_iter().flat_map(|x| {
                [-3, 11]
                    .into_iter()
                    .flat_map(move |y| [-24, 12].map(|z| center + IVec3::new(x, y, z)))
            });
            corners
                .map(|corner| corner.div_euclid(IVec3::splat(CHUNK_SIZE)))
                .all(|chunk| map.is_chunk_loaded(chunk))
                && map.get_height_ground(player.translation) > 0
        })
    });
    // Settled
    h.frames(60);

    let world = h.world_mut();
    let player = world
        .query_filtered::<&Transform, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap()
        .translation;
    let blocks = &world.resource::<ClientWorldMap>().blocks;
    IVec3::new(
        player.x.floor() as i32,
        blocks.get_height_ground(player) + 1,
        player.z.floor() as i32,
    )
}

/// Waits for the player to stand on the ground, then builds around it, the player looking north
/// (-Z): an arena cleared of the terrain, with a stone floor, a wall 8 blocks ahead with the blue
/// portal facing the player, and further, to the east, a wall as high as the orange portal,
/// which faces north. Six blocks north of the orange portal stands a pillar of sand, 5 high;
/// between the portal and where its camera stands, south of its wall, a pillar of cactus, which
/// the camera must not see. A second pillar of sand stands to the left of the blue portal, seen
/// straight at the same depth.
fn build_scene(h: &mut Harness) -> Scene {
    let feet = settle(h);
    let world = h.world_mut();
    let mut map = world.resource_mut::<ClientWorldMap>();
    let blocks = &mut map.blocks;
    let mut fill = |min: IVec3, max: IVec3, id: Option<BlockId>| {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = feet + IVec3::new(x, y, z);
                    match id {
                        Some(id) => {
                            blocks.set_block(pos, BlockData::new(id, BlockDirection::Front))
                        }
                        None => {
                            blocks.remove_block(pos);
                        }
                    }
                }
            }
        }
    };
    fill(IVec3::new(-14, 0, -24), IVec3::new(24, 10, -1), None);
    fill(
        IVec3::new(-14, -2, -24),
        IVec3::new(24, -1, -1),
        Some(BlockId::Stone),
    );
    fill(
        IVec3::new(-2, 0, -8),
        IVec3::new(2, 3, -8),
        Some(BlockId::Cobblestone),
    );
    fill(
        IVec3::new(10, 0, -12),
        IVec3::new(13, 2, -12),
        Some(BlockId::Cobblestone),
    );
    fill(
        IVec3::new(12, 0, -18),
        IVec3::new(13, 4, -18),
        Some(BlockId::Sand),
    );
    fill(
        IVec3::new(12, 0, -8),
        IVec3::new(13, 2, -8),
        Some(BlockId::Cactus),
    );
    fill(
        IVec3::new(-7, 0, -13),
        IVec3::new(-6, 4, -13),
        Some(BlockId::Sand),
    );

    let blue = Portal {
        owner: 1,
        color: PortalColor::Blue,
        anchor: feet + IVec3::new(-1, 0, -8),
        face: FaceDirection::PlusZ,
        up: FaceDirection::PlusY,
    };
    // Facing north, its left is to the east
    let orange = Portal {
        owner: 1,
        color: PortalColor::Orange,
        anchor: feet + IVec3::new(12, 0, -12),
        face: FaceDirection::MinusZ,
        up: FaceDirection::PlusY,
    };
    blocks.portals_mut().insert(blue);
    blocks.portals_mut().insert(orange);
    let through = blocks.portals().through(&blue).unwrap();
    assert!(through.transform_vector3(Vec3::X).distance(Vec3::X) < 1e-4);
    assert!(through.transform_point3(Vec3::ZERO).distance(THROUGH) < 1e-4);

    look(h, 0.0);
    // Meshed and drawn
    h.frames(60);
    Scene { feet, blue, orange }
}

/// Turns the player's camera to the yaw `yaw`, level: 0 looks north.
fn look(h: &mut Harness, yaw: f32) {
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = yaw;
    controller.angle_y = 0.0;
}

/// Where `point` is on the screen of the player's camera, in pixels.
fn on_screen(h: &mut Harness, point: Vec3) -> Vec2 {
    let world = h.world_mut();
    let (camera, transform) = world
        .query_filtered::<(&Camera, &GlobalTransform), With<CameraController>>()
        .single(world)
        .unwrap();
    camera.world_to_viewport(transform, point).unwrap()
}

/// The mean colour of the 7 by 7 pixels of `shot` around `at`.
fn mean_color(shot: &RgbImage, at: Vec2) -> Vec3 {
    let mut sum = Vec3::ZERO;
    for dx in -3..=3 {
        for dy in -3..=3 {
            let pixel = shot.get_pixel((at.x as i32 + dx) as u32, (at.y as i32 + dy) as u32);
            sum += Vec3::from_array(pixel.0.map(f32::from));
        }
    }
    sum / 49.0
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_view_through_a_portal_is_the_other_side() {
    let mut h = enter_world(GameFolder::new().with_world("Portals"), "Portals");
    let scene = build_scene(&mut h);
    // The sand lit by the sun
    h.set_time("day");
    h.frames(5);
    let shot = h.screenshot();
    h.snapshot("through_the_blue_portal");

    let world = h.world_mut();
    let active: Vec<PortalColor> = world
        .query::<(&PortalCamera, &Camera)>()
        .iter(world)
        .filter(|(_, camera)| camera.is_active)
        .map(|(portal, _)| portal.color)
        .collect();
    // The orange portal is seen from behind
    assert_eq!(active, [PortalColor::Blue]);

    // The south face of the far pillar of sand, seen through the blue portal: its camera sees
    // it past the cactus, which lies behind the orange portal, and past its wall. Above the top
    // of that wall, so that the sun or the moon, low in the south, does not leave it in the
    // wall's shadow
    let feet = scene.feet.as_vec3();
    let far_sand = feet + Vec3::new(12.5, 3.4, -17.0);
    let through = on_screen(&mut h, far_sand - THROUGH);
    // The near pillar of sand, seen straight, at the same depth
    let near_sand = feet + Vec3::new(-6.0, 3.4, -12.0);
    let straight = on_screen(&mut h, near_sand);
    assert!((through.y - straight.y).abs() < 1.0);

    let (spot, straight_spot) = (through, straight);
    let (through, straight) = (mean_color(&shot, through), mean_color(&shot, straight));
    // Sand, not cactus nor cobblestone
    assert!(
        through.x > through.z + 25.0,
        "not sand through the portal: {through}"
    );
    // As bright and as warm as when seen straight, at the same depth: the view through the
    // portal is lit, fogged and tonemapped as the rest of the screen
    assert!(
        (through - straight).abs().max_element() < 8.0,
        "sand through the portal {through}, seen straight {straight}"
    );

    // Looking away turns the portal camera off; looking back turns it on again, which then
    // draws the chunks again from its first frame (`show_chunks_to_new_views`)
    look(&mut h, PI);
    h.frames(5);
    assert!(drawing(h.world_mut()).is_empty());
    look(&mut h, 0.0);
    h.frames(1);
    let shot = h.screenshot();
    let (again, straight) = (mean_color(&shot, spot), mean_color(&shot, straight_spot));
    assert!(
        (again - straight).abs().max_element() < 8.0,
        "after looking away and back, sand through the portal {again}, seen straight {straight}"
    );

    // Alone, the blue portal shows its colour
    h.world_mut()
        .resource_mut::<ClientWorldMap>()
        .blocks
        .portals_mut()
        .remove(scene.orange.owner, PortalColor::Orange);
    h.frames(10);
    let shot = h.screenshot();
    h.snapshot("the_blue_portal_alone");
    let blue = mean_color(&shot, spot);
    assert!(blue.z > blue.x + 40.0, "not blue: {blue}");
}

/// With every quality of the shaders, the view through a portal looks as the world seen
/// straight: the far pillar of sand through the blue portal is as bright and as warm as the
/// near one, at the same depth, and the sky seen through the blue portal is the sky seen beside
/// it. Each picture is kept with `UI_SNAPSHOTS=1`.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_view_through_a_portal_looks_as_the_world_with_every_shader() {
    let mut h = enter_world(GameFolder::new().with_world("Portals"), "Portals");
    let scene = build_scene(&mut h);
    h.set_time("day");
    let feet = scene.feet.as_vec3();
    let far_sand = feet + Vec3::new(12.5, 3.4, -17.0);
    let near_sand = feet + Vec3::new(-6.0, 3.4, -12.0);
    // In the top left corner of the opening, the sky; just left of the wall at the same height,
    // the sky in the same direction to within a few degrees
    let corner = scene.blue.frame().transform_point3(Vec3::new(
        -0.3 * PORTAL_WIDTH as f32,
        0.4 * PORTAL_HEIGHT as f32,
        0.0,
    ));
    let wall_edge = feet + Vec3::new(-2.0, 2.0, -7.0);
    for quality in ShaderQuality::ALL {
        h.world_mut().resource_mut::<GraphicsSettings>().shaders = quality;
        // The effects inserted, the exposure settled and TAA's history filled
        h.frames(40);
        let shot = h.screenshot();
        keep(
            &shot,
            "the_view_through_a_portal_looks_as_the_world_with_every_shader",
            &format!("{quality:?}").to_lowercase(),
        );
        let through = mean_color(&shot, on_screen(&mut h, far_sand - THROUGH));
        let straight = mean_color(&shot, on_screen(&mut h, near_sand));
        assert!(
            (through - straight).abs().max_element() < 10.0,
            "{quality:?}: sand through the portal {through}, seen straight {straight}"
        );
        let corner = on_screen(&mut h, corner);
        let beside = Vec2::new(on_screen(&mut h, wall_edge).x - 8.0, corner.y);
        let (through, straight) = (mean_color(&shot, corner), mean_color(&shot, beside));
        // The portal cameras draw the haze of the rays of light too (`volumetric_fog.rs`): the
        // sky through the portal is as hazed as the sky seen straight
        assert!(
            (through - straight).abs().max_element() < 4.0,
            "{quality:?}: sky through the portal {through}, seen straight {straight}"
        );
    }
}

const THROUGH_WALL: &str = "a_pair_through_a_thin_wall_shows_one_view";

/// The classic pair: a portal on each face of the same cells of a wall one block thick, 7
/// blocks north of the player's feet, looking north at the one on the near face. Through it,
/// the player sees what lies north of the wall, sand, as if the wall had a hole: the box of the
/// other portal, hollowed into the same cells from the far face, never shows in its opening,
/// whichever colour is on the near face.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_pair_through_a_thin_wall_shows_one_view() {
    let mut h = enter_world(GameFolder::new().with_world("Portals"), "Portals");
    let feet = settle(&mut h);
    {
        let world = h.world_mut();
        let blocks = &mut world.resource_mut::<ClientWorldMap>().blocks;
        let mut fill = |min: IVec3, max: IVec3, id: Option<BlockId>| {
            for x in min.x..=max.x {
                for y in min.y..=max.y {
                    for z in min.z..=max.z {
                        let pos = feet + IVec3::new(x, y, z);
                        match id {
                            Some(id) => {
                                blocks.set_block(pos, BlockData::new(id, BlockDirection::Front))
                            }
                            None => {
                                blocks.remove_block(pos);
                            }
                        }
                    }
                }
            }
        };
        fill(IVec3::new(-8, 0, -14), IVec3::new(8, 10, -1), None);
        fill(
            IVec3::new(-8, -2, -14),
            IVec3::new(8, -1, -1),
            Some(BlockId::Stone),
        );
        // The wall, and north of it a floor and a wall of sand, one free cell from it
        fill(
            IVec3::new(-3, 0, -7),
            IVec3::new(3, 4, -7),
            Some(BlockId::Cobblestone),
        );
        fill(
            IVec3::new(-8, -2, -14),
            IVec3::new(8, -1, -8),
            Some(BlockId::Sand),
        );
        fill(
            IVec3::new(-8, 0, -9),
            IVec3::new(8, 8, -9),
            Some(BlockId::Sand),
        );
    }
    h.set_time("day");
    look(&mut h, 0.0);

    for near in [PortalColor::Blue, PortalColor::Orange] {
        let far = match near {
            PortalColor::Blue => PortalColor::Orange,
            PortalColor::Orange => PortalColor::Blue,
        };
        // The cells x -1 and 0 from the feet: from the north, the anchor is on the right
        let near_portal = Portal {
            owner: 1,
            color: near,
            anchor: feet + IVec3::new(-1, 0, -7),
            face: FaceDirection::PlusZ,
            up: FaceDirection::PlusY,
        };
        let far_portal = Portal {
            owner: 1,
            color: far,
            anchor: feet + IVec3::new(0, 0, -7),
            face: FaceDirection::MinusZ,
            up: FaceDirection::PlusY,
        };
        {
            let world = h.world_mut();
            let blocks = &mut world.resource_mut::<ClientWorldMap>().blocks;
            blocks.portals_mut().insert(near_portal);
            blocks.portals_mut().insert(far_portal);
            assert_eq!(
                near_portal.cells().collect::<Vec<_>>().len(),
                far_portal
                    .cells()
                    .filter(|cell| near_portal.cells().any(|near| near == *cell))
                    .count()
            );
        }
        // Meshed and drawn
        h.frames(60);
        let shot = h.screenshot();
        keep(&shot, THROUGH_WALL, &format!("{near:?}_on_the_near_face"));
        assert_eq!(drawing(h.world_mut()), [(near, 1)]);

        // A grid over the opening, on the near face, 0.4 block in from its edges: sand
        // everywhere, not the view of the far portal, whose camera is off and whose image is
        // black. By its hue, as the day may have turned to night on a busy machine
        let plane = (feet + IVec3::new(0, 0, -6)).as_vec3();
        let mut not_sand = Vec::new();
        for i in 0..=6 {
            for j in 0..=8 {
                let point = plane
                    + Vec3::new(
                        -1.0 + 0.4 + 1.2 * i as f32 / 6.0,
                        0.4 + 2.2 * j as f32 / 8.0,
                        0.0,
                    );
                let color = mean_color(&shot, on_screen(&mut h, point));
                if !(color.x > 20.0 && color.x > 1.4 * color.z && color.y > 1.2 * color.z) {
                    not_sand.push((point - plane, color));
                }
            }
        }
        assert!(
            not_sand.is_empty(),
            "{near:?} on the near face, not sand at {not_sand:?}"
        );

        let world = h.world_mut();
        let blocks = &mut world.resource_mut::<ClientWorldMap>().blocks;
        blocks.portals_mut().remove(1, near);
        blocks.portals_mut().remove(1, far);
    }
}

/// Two portals facing each other, 15 blocks apart: the blue one on a wall of cobblestone 9 wide
/// and 6 high, 8 blocks north of the player's feet, facing south, and the orange one on a wall
/// alike 8 blocks south, facing north, on a stone floor cleared of the terrain. The player stands
/// between them, looking north at the blue one. Returns the blue portal.
fn build_facing_scene(h: &mut Harness) -> Portal {
    let feet = settle(h);
    let world = h.world_mut();
    let mut map = world.resource_mut::<ClientWorldMap>();
    let blocks = &mut map.blocks;
    let mut fill = |min: IVec3, max: IVec3, id: Option<BlockId>| {
        for x in min.x..=max.x {
            for y in min.y..=max.y {
                for z in min.z..=max.z {
                    let pos = feet + IVec3::new(x, y, z);
                    match id {
                        Some(id) => {
                            blocks.set_block(pos, BlockData::new(id, BlockDirection::Front))
                        }
                        None => {
                            blocks.remove_block(pos);
                        }
                    }
                }
            }
        }
    };
    fill(IVec3::new(-14, 0, -12), IVec3::new(14, 10, 12), None);
    fill(
        IVec3::new(-14, -2, -12),
        IVec3::new(14, -1, 12),
        Some(BlockId::Stone),
    );
    for z in [-8, 8] {
        fill(
            IVec3::new(-4, 0, z),
            IVec3::new(4, 5, z),
            Some(BlockId::Cobblestone),
        );
    }
    let blue = Portal {
        owner: 1,
        color: PortalColor::Blue,
        anchor: feet + IVec3::new(-1, 0, -8),
        face: FaceDirection::PlusZ,
        up: FaceDirection::PlusY,
    };
    // Facing north, its left is to the east
    let orange = Portal {
        owner: 1,
        color: PortalColor::Orange,
        anchor: feet + IVec3::new(0, 0, 8),
        face: FaceDirection::MinusZ,
        up: FaceDirection::PlusY,
    };
    blocks.portals_mut().insert(blue);
    blocks.portals_mut().insert(orange);
    let through = blocks.portals().through(&blue).unwrap();
    assert!(through.transform_vector3(Vec3::X).distance(Vec3::X) < 1e-4);
    assert!(through.transform_point3(Vec3::ZERO).distance(FACING) < 1e-4);

    look(h, 0.0);
    // Meshed and drawn
    h.frames(60);
    blue
}

/// Through the blue portal of the facing portals, the player's view moves this far south.
const FACING: Vec3 = Vec3::new(0.0, 0.0, 15.0);

/// The levels of the cameras of each colour that draw.
fn drawing(w: &mut World) -> Vec<(PortalColor, usize)> {
    let mut drawing: Vec<_> = w
        .query::<(&PortalCamera, &Camera)>()
        .iter(w)
        .filter(|(_, camera)| camera.is_active)
        .map(|(portal, _)| (portal.color, portal.level))
        .collect();
    drawing.sort_by_key(|(color, level)| (*color as u8, *level));
    drawing
}

/// Whether `color` is the blue of a portal rather than grey stone or a red body, or the sky,
/// which is bluer than red by about 65.
fn bluish(color: Vec3) -> bool {
    color.z > color.x + 40.0
}

/// Between two portals facing each other, the player looking into the blue one sees, through
/// it, its own body from behind, red as it is, and the blue portal again, which shows its view
/// in turn: the wall around the blue portal seen a third time, then that third blue portal, in
/// its colour.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn facing_portals_show_each_other_three_times_and_the_player_its_body() {
    // A render distance of 4 chunks: the third portal, seen 37.5 blocks away, is before the fog
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 4)")
        .with_world("Facing");
    let mut h = enter_world(folder, "Facing");
    let blue = build_facing_scene(&mut h);
    h.set_time("day");
    h.frames(5);
    let shot = h.screenshot();
    h.snapshot("facing_portals");
    keep(&shot, "facing_portals", "facing_portals");

    // The orange portal is behind the player
    let blue_levels: Vec<_> = (1..=crate::portal::PORTAL_RECURSION)
        .map(|level| (PortalColor::Blue, level))
        .collect();
    assert_eq!(drawing(h.world_mut()), blue_levels);

    let world = h.world_mut();
    let eye = world
        .query_filtered::<&Transform, With<CameraController>>()
        .single(world)
        .unwrap()
        .translation;
    let body = world
        .query_filtered::<&GlobalTransform, With<crate::player::PlayerBody>>()
        .single(world)
        .unwrap()
        .translation();

    // The body, seen through the blue portal: as far behind it as the orange portal is
    let seen = on_screen(&mut h, body - FACING);
    let color = mean_color(&shot, seen);
    assert!(
        color.x > color.y + 50.0 && color.x > color.z + 50.0,
        "not the red body through the portal: {color}"
    );

    // The blue portal seen through itself twice: the wall around it, where it shows over the
    // portal seen once, between the rim of the third opening and the top of the second
    let plane = blue.frame().translation.z;
    let top = blue.anchor.y as f32 + shared::world::PORTAL_HEIGHT as f32;
    let (second, third) = (eye.z - (plane - FACING.z), eye.z - (plane - 2.0 * FACING.z));
    let second_top = eye.y + (top - eye.y) * third / second;
    let wall = Vec3::new(eye.x, (top + 0.3 + second_top) / 2.0, plane) - 2.0 * FACING;
    let color = mean_color(&shot, on_screen(&mut h, wall));
    assert!(
        !bluish(color),
        "not the wall around the third portal in the second: {color}"
    );
    // And the third portal, in its colour, above the bodies seen through the first two
    let body_top = body.y + 0.9;
    let over_bodies = eye.y + (body_top - eye.y) * third / (eye.z - (body.z - FACING.z));
    let inside = Vec3::new(eye.x, (over_bodies + top) / 2.0, plane) - 2.0 * FACING;
    let color = mean_color(&shot, on_screen(&mut h, inside));
    assert!(bluish(color), "not the third blue portal: {color}");
}

/// The time a frame takes, the screen drawn into an image, with the blue portal in view, the
/// portals out of view, and the same view without portals, in turns over several rounds, so
/// that the noise of the machine falls on each alike. `BENCH_GPU=1` draws on the GPU instead of
/// lavapipe. `BENCH_SHADERS=high` (or `low`, `medium`, `ultra`) draws with those shaders, Off by
/// default.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_portal_frame_time() {
    let shaders = match std::env::var("BENCH_SHADERS").as_deref() {
        Ok("low") => ShaderQuality::Low,
        Ok("medium") => ShaderQuality::Medium,
        Ok("high") => ShaderQuality::High,
        Ok("ultra") => ShaderQuality::Ultra,
        _ => ShaderQuality::Off,
    };
    let options = Options {
        shaders: Some(shaders),
        ..default()
    };
    let mut h = enter_world_with(GameFolder::new().with_world("Portals"), "Portals", options);
    let scene = build_scene(&mut h);
    // The window has no surface: the screen is drawn where the screenshots are taken from
    let target = h.target.clone();
    h.set_camera_targets(RenderTarget::Image(target.into()));
    let (rounds, frames) = if std::env::var_os("BENCH_GPU").is_some() {
        (6, 300)
    } else {
        (3, 20)
    };
    let set_portals = |h: &mut Harness, on: bool| {
        let mut map = h.world_mut().resource_mut::<ClientWorldMap>();
        let portals = map.blocks.portals_mut();
        for portal in [scene.blue, scene.orange] {
            if on {
                portals.insert(portal);
            } else {
                portals.remove(portal.owner, portal.color);
            }
        }
    };
    let cases: [(&str, bool, f32); 3] = [
        ("the blue portal in view", true, 0.0),
        ("the portals out of view", true, std::f32::consts::PI),
        ("the same view without portals", false, 0.0),
    ];
    let mut samples = vec![Vec::new(); cases.len()];
    for _ in 0..rounds {
        for (case, (_, portals, yaw)) in cases.iter().enumerate() {
            set_portals(&mut h, *portals);
            look(&mut h, *yaw);
            h.frames(20);
            for _ in 0..frames {
                let start = Instant::now();
                h.frames(1);
                samples[case].push(start.elapsed().as_secs_f64() * 1e3);
            }
        }
    }
    for ((what, ..), mut samples) in cases.iter().zip(samples) {
        samples.sort_by(f64::total_cmp);
        println!(
            "{what}: median {:.2} ms, p95 {:.2} ms per frame",
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100]
        );
    }
}

/// The time a frame takes between the facing portals, render distance 8 (4 on lavapipe), the
/// screen drawn into an image: looking into the blue portal with recursion 3 and with recursion
/// 1, and away from both, in turns over several rounds. Then the frame on which the views of
/// the blue portal start, looking back at it after 10 frames looking west, when every chunk is
/// culled on the CPU for a frame (`show_chunks_to_new_views`), and the frame after it, when they
/// get their `NoCpuCulling` back, against the frames in view. `BENCH_GPU=1` draws on the GPU
/// instead of lavapipe.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_portal_recursion_frame_time() {
    use crate::portal::PortalRecursion;
    // Lavapipe draws fewer chunks and frames, within the harness's 300 s
    let gpu = std::env::var_os("BENCH_GPU").is_some();
    let (distance, rounds, warm, frames, starts) = if gpu {
        (8, 6, 20, 300, 60)
    } else {
        (4, 2, 5, 10, 5)
    };
    let folder = GameFolder::new()
        .with_file("video.ron", &format!("(render_distance: {distance})"))
        .with_world("Facing");
    let mut h = enter_world(folder, "Facing");
    build_facing_scene(&mut h);
    let target = h.target.clone();
    h.set_camera_targets(RenderTarget::Image(target.into()));
    let time = |h: &mut Harness| {
        let start = Instant::now();
        h.frames(1);
        start.elapsed().as_secs_f64() * 1e3
    };
    let cases: [(&str, usize, f32); 3] = [
        ("recursion 3, in view", 3, 0.0),
        ("recursion 1, in view", 1, 0.0),
        // West, along the walls: the orange portal faces the player's back
        ("out of view", 3, PI / 2.0),
    ];
    let mut samples = vec![Vec::new(); cases.len()];
    for _ in 0..rounds {
        for (case, (_, depth, yaw)) in cases.iter().enumerate() {
            h.world_mut().insert_resource(PortalRecursion(*depth));
            look(&mut h, *yaw);
            h.frames(warm);
            for _ in 0..frames {
                samples[case].push(time(&mut h));
            }
        }
    }
    let mut started = Vec::new();
    let mut after = Vec::new();
    for _ in 0..starts {
        look(&mut h, PI / 2.0);
        h.frames(10);
        look(&mut h, 0.0);
        started.push(time(&mut h));
        after.push(time(&mut h));
        assert_eq!(drawing(h.world_mut()).len(), 3);
    }
    let summary = |mut samples: Vec<f64>| {
        samples.sort_by(f64::total_cmp);
        format!(
            "median {:.2} ms, p95 {:.2} ms per frame ({} frames)",
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100],
            samples.len()
        )
    };
    for ((what, ..), samples) in cases.iter().zip(samples) {
        println!("{what}: {}", summary(samples));
    }
    println!("the frame the views start: {}", summary(started));
    println!("the frame after: {}", summary(after));
}

/// A mod with two commands: `/walls` builds two stone walls 4 wide and 4 high, from 10 blocks
/// above the centre of its author, 8 blocks north of it facing south and 8 blocks south facing
/// north; `/plug` puts a block in front of the north wall, 11 blocks up.
const WALLS_MOD: &str = r#"
    local rc = rustcraft
    rc.register_command("walls", function(ctx, args)
        local p = rc.player_position(ctx.player)
        for x = -2, 1 do
            for y = 10, 13 do
                rc.set_block({ x = p.x + x, y = p.y + y, z = p.z - 8 }, "stone")
                rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + 8 }, "stone")
            end
        end
        return "Walls built"
    end)
    rc.register_command("plug", function(ctx, args)
        local p = rc.player_position(ctx.player)
        rc.set_block({ x = p.x, y = p.y + 11, z = p.z - 7 }, "stone")
        return "Plugged"
    end)
"#;

impl Harness {
    /// Sends `command` from the chat.
    pub(super) fn command(&mut self, command: &str) {
        self.press(KeyCode::KeyT);
        self.type_text(command);
        self.press(KeyCode::Enter);
    }

    /// Sets the clock to the next noon (`day`) or midnight (`night`), and waits for it: for a
    /// third of a day around either, the sky stays the same colour.
    pub(super) fn set_time(&mut self, time: &str) {
        self.command(&format!("/time set {time}"));
        let day = time == "day";
        self.wait_until("the hour", SERVER, |w| {
            let height = sun_height(w.resource::<ClientTime>().0);
            if day {
                height > 0.6
            } else {
                height < -0.6
            }
        });
    }

    /// The colour of the sky on the screen, in sRGB levels: the clear colour of the player's
    /// camera.
    fn sky(&mut self) -> Vec3 {
        let world = self.world_mut();
        let camera = world
            .query_filtered::<&Camera, With<CameraController>>()
            .single(world)
            .unwrap();
        let ClearColorConfig::Custom(sky) = camera.clear_color else {
            panic!("the sky is the clear colour of the player's camera");
        };
        Vec3::from_array(sky.to_srgba().to_u8_array_no_alpha().map(f32::from))
    }

    /// Turns the player's camera toward `point`, on a face of the block `cell`, and waits for it
    /// to aim at that block, as far as the portal gun aims.
    #[track_caller]
    pub(super) fn aim_at(&mut self, point: Vec3, cell: IVec3) {
        let world = self.world_mut();
        let eyes = world
            .query_filtered::<&Transform, With<CameraController>>()
            .single(world)
            .unwrap()
            .translation;
        let direction = (point - eyes).normalize();
        let mut controller = world
            .query::<&mut CameraController>()
            .single_mut(world)
            .unwrap();
        // The camera turns by the yaw, then by the pitch, which looks down when positive
        controller.angle_x = (-direction.x).atan2(-direction.z);
        controller.angle_y = -direction.y.asin();
        self.wait_until("the aim", SERVER, |w| {
            let mut players = w
                .try_query_filtered::<&Transform, With<CurrentPlayerMarker>>()
                .unwrap();
            let mut cameras = w
                .try_query_filtered::<&Transform, With<CameraController>>()
                .unwrap();
            let (Ok(player), Ok(camera)) = (players.single(w), cameras.single(w)) else {
                return false;
            };
            let blocks = &w.resource::<ClientWorldMap>().blocks;
            raycast_far(blocks, camera, &player.translation, Default::default())
                .is_some_and(|target| target.position == cell)
        });
    }
}

/// The portal of the local player of colour `color`, as the client knows it.
pub(super) fn own_portal(w: &World, color: PortalColor) -> Option<Portal> {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    let id = players.single(w).ok()?.id;
    let portals = w.resource::<ClientWorldMap>().blocks.portals();
    portals.get(id, color).copied()
}

/// How many portal cameras of level 1 there are: one per open portal.
fn portal_cameras(w: &mut World) -> usize {
    w.query::<&PortalCamera>()
        .iter(w)
        .filter(|camera| camera.level == 1)
        .count()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_portal_gun_places_both_portals_and_a_blocked_one_goes() {
    let folder = GameFolder::new()
        .with_world("Gun")
        .with_items("Gun", "Tester", &[(0, ItemId::PortalGun, 1)])
        .with_mod("walls", WALLS_MOD);
    let mut h = enter_world(folder, "Gun");
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    // A new player is put on the ground from above: it stands once just above the ground
    h.wait_until("the player on the ground", SERVER, |w| {
        let mut players = w
            .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
            .unwrap();
        let blocks = &w.resource::<ClientWorldMap>().blocks;
        players.single(w).is_ok_and(|player| {
            let ground = blocks.get_height_ground(player.position);
            player.on_ground && ground > 0 && player.position.y < ground as f32 + 2.5
        })
    });
    // Settled
    h.frames(30);
    let world = h.world_mut();
    let centre = world
        .query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .single(world)
        .unwrap()
        .position
        .floor()
        .as_ivec3();
    h.command("/walls");
    let north = centre + IVec3::new(0, 11, -8);
    let south = centre + IVec3::new(0, 11, 8);
    h.wait_until("the walls", SERVER, |w| {
        let blocks = &w.resource::<ClientWorldMap>().blocks;
        [north, south]
            .iter()
            .all(|cell| blocks.get_block(*cell).is_some())
    });

    // A left click on the north wall places the blue portal there, and the server agrees
    h.aim_at(north.as_vec3() + Vec3::new(0.5, 0.5, 1.0), north);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.wait_until("the blue portal", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_some()
    });
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    assert_eq!(blue.face, FaceDirection::PlusZ);
    assert!(blue.cells().any(|cell| cell == north));
    // Alone, it is closed
    h.frames(30);
    assert_eq!(own_portal(h.world(), PortalColor::Blue), Some(blue));
    assert_eq!(portal_cameras(h.world_mut()), 0);

    // A right click on the south wall places the orange one: both open
    h.aim_at(south.as_vec3() + Vec3::new(0.5, 0.5, 0.0), south);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 3);
    h.wait_until("the orange portal", SERVER, |w| {
        own_portal(w, PortalColor::Orange).is_some()
    });
    let orange = own_portal(h.world(), PortalColor::Orange).unwrap();
    assert_eq!(orange.face, FaceDirection::MinusZ);
    assert!(orange.cells().any(|cell| cell == south));
    h.frames(30);
    assert_eq!(own_portal(h.world(), PortalColor::Blue), Some(blue));
    assert_eq!(own_portal(h.world(), PortalColor::Orange), Some(orange));
    assert_eq!(portal_cameras(h.world_mut()), 2);
    // Neither click broke or placed a block
    let blocks = &h.world().resource::<ClientWorldMap>().blocks;
    assert!(blue
        .cells()
        .chain(orange.cells())
        .all(|cell| blocks.get_block(cell).is_some()));
    assert!(blue
        .front_cells()
        .chain(orange.front_cells())
        .all(|cell| blocks.get_block(cell).is_none()));
    assert_eq!(slot(h.world(), 0), Some((ItemId::PortalGun, 1)));

    // A block put in front of the blue portal by the server takes it away, which closes the
    // orange one
    h.command("/plug");
    h.wait_until("the blue portal gone", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_none()
    });
    h.frames(10);
    assert_eq!(own_portal(h.world(), PortalColor::Orange), Some(orange));
    assert_eq!(portal_cameras(h.world_mut()), 0);
}

/// A mod with a command, `/course`, that builds around its author, from the block it is in: a
/// stone platform 3 blocks below, and on it two stone walls of 2 by 3 blocks, one 6 blocks
/// north facing south, one 6 blocks east facing west.
const COURSE_MOD: &str = r#"
    local rc = rustcraft
    rc.register_command("course", function(ctx, args)
        local p = rc.player_position(ctx.player)
        local function fill(x0, y0, z0, x1, y1, z1)
            for x = x0, x1 do
                for y = y0, y1 do
                    for z = z0, z1 do
                        rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + z }, "stone")
                    end
                end
            end
        end
        fill(-6, -3, -9, 8, -3, 4)
        fill(0, -2, -6, 1, 0, -6)
        fill(6, -2, -2, 6, 0, -1)
        return "Course built"
    end)
"#;

/// The local player, as the client has it.
pub(super) fn local_player(w: &World) -> Option<Player> {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).ok().cloned()
}

/// The yaw of the player's camera.
fn yaw(w: &mut World) -> f32 {
    w.query::<&CameraController>().single(w).unwrap().angle_x
}

/// Turns the player's camera to the yaw `yaw`, keeping its pitch.
fn set_yaw(h: &mut Harness, yaw: f32) {
    let world = h.world_mut();
    world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap()
        .angle_x = yaw;
}

/// The mean difference between two screenshots, per channel, from 0 to 255.
fn difference(a: &RgbImage, b: &RgbImage) -> f32 {
    let total: u64 = a
        .as_raw()
        .iter()
        .zip(b.as_raw())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum();
    total as f32 / a.as_raw().len() as f32
}

/// The first column of `shot`, left of the minimap, that is brighter than the columns two pixels
/// to either side of it, by more than 10 levels a pixel on average, both at the top of the screen
/// and at the bottom, above the hotbar: a thin line from the sky down to the floor, which neither
/// the edges of the walls nor the rims of the portals, seen whole, are.
fn hairline(shot: &RgbImage) -> Option<u32> {
    let brightness = |x: u32, rows: std::ops::Range<u32>| -> f32 {
        let total: u32 = rows
            .clone()
            .map(|y| {
                shot.get_pixel(x, y)
                    .0
                    .iter()
                    .map(|c| u32::from(*c))
                    .sum::<u32>()
            })
            .sum();
        total as f32 / (3 * rows.len()) as f32
    };
    let line = |rows: std::ops::Range<u32>| -> Vec<bool> {
        let columns: Vec<f32> = (0..1094).map(|x| brightness(x, rows.clone())).collect();
        (2..columns.len() - 2)
            .map(|x| columns[x] - columns[x - 2].max(columns[x + 2]) > 10.0)
            .collect()
    };
    let (top, bottom) = (line(0..60), line(590..630));
    (0..top.len())
        .find(|x| top[*x] && bottom[*x])
        .map(|x| x as u32 + 2)
}

/// Saves `shot` as `name` into `target/ui-snapshots/e2e__portals__<test>/`, when
/// `UI_SNAPSHOTS=1`.
fn keep(shot: &RgbImage, test: &str, name: &str) {
    if !std::env::var_os("UI_SNAPSHOTS").is_some_and(|value| value == "1") {
        return;
    }
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("../target/ui-snapshots/e2e__portals__{test}"));
    std::fs::create_dir_all(&folder).unwrap();
    shot.save(folder.join(format!("{name}.png"))).unwrap();
}

const WALKING: &str = "walking_through_a_portal_is_seamless";

/// In solo, the player walks into its blue portal and out of its orange one, which faces a
/// quarter turn to the left: its camera turns once, by that quarter turn, and nothing jumps:
/// the player moves no more than a frame's walk from a frame to the next, the crossing measured
/// through the portal, and the screen changes no more across the crossing than between two
/// frames on one side. That the server simulates the crossing as the client predicts it,
/// without a correction, is tested in `player::update`, not here.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn walking_through_a_portal_is_seamless() {
    // In the air, where the course is built clear of the terrain
    let folder = GameFolder::new()
        .with_world("Course")
        .with_player("Course", "Tester", |player| {
            player.position = Vec3::new(0.5, 120.9, 0.5);
            player.game_mode = shared::players::game_mode::GameMode::Creative;
            player.is_flying = true;
            player.inventory.inner.insert(
                0,
                shared::world::ItemStack {
                    item_id: ItemId::PortalGun,
                    item_type: ItemId::PortalGun.get_default_type(),
                    nb: 1,
                },
            );
        })
        .with_mod("course", COURSE_MOD);
    let mut h = enter_world(folder, "Course");
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    h.wait_until("the player in the air", SERVER, |w| {
        local_player(w).is_some_and(|player| {
            let map = &w.resource::<ClientWorldMap>().blocks;
            player.is_flying && shared::players::movement::is_terrain_loaded(player.position, map)
        })
    });
    // A world starts at sunset, which lasts minutes now that a day lasts twenty: its sky would
    // be taken for the orange portal
    h.set_time("day");
    let start = local_player(h.world()).unwrap().position.floor().as_ivec3();
    h.command("/course");
    let blue_cell = start + IVec3::new(0, -1, -6);
    let orange_cell = start + IVec3::new(6, -1, -2);
    h.wait_until("the course", SERVER, |w| {
        let blocks = &w.resource::<ClientWorldMap>().blocks;
        [start + IVec3::new(8, -3, 4), blue_cell, orange_cell]
            .iter()
            .all(|cell| blocks.get_block(*cell).is_some())
    });
    // Down onto the platform
    h.press(KeyCode::KeyF);
    h.wait_until("the player on the platform", SERVER, |w| {
        local_player(w).is_some_and(|player| player.on_ground)
    });
    h.frames(10);

    h.aim_at(blue_cell.as_vec3() + Vec3::new(0.5, 0.5, 1.0), blue_cell);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.aim_at(
        orange_cell.as_vec3() + Vec3::new(0.0, 0.5, 0.5),
        orange_cell,
    );
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 3);
    h.wait_until("both portals", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_some() && own_portal(w, PortalColor::Orange).is_some()
    });
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    let portals = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .clone();
    let through = portals.through(&blue).unwrap();
    let orange = *portals.get(blue.owner, PortalColor::Orange).unwrap();
    assert_eq!(blue.face, FaceDirection::PlusZ);

    // Toward the middle of the blue portal, level
    let player = local_player(h.world()).unwrap().position;
    let target = Vec3::from(blue.frame().translation);
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = (-(target.x - player.x)).atan2(-(target.z - player.z));
    controller.angle_y = 0.0;
    h.frames(10);

    // Where the player is and where it looks, after each frame of the walk
    let mut trace: Vec<(Vec3, f32)> = Vec::new();
    let record = |h: &mut Harness, trace: &mut Vec<(Vec3, f32)>| {
        let position = local_player(h.world()).unwrap().position;
        let yaw = yaw(h.world_mut());
        trace.push((position, yaw));
    };
    record(&mut h, &mut trace);
    // Up to a block before the portal
    h.key_down(KeyCode::KeyW);
    while blue.signed_distance(local_player(h.world()).unwrap().position) > 1.0 {
        record(&mut h, &mut trace);
        h.frames(1);
    }
    h.key_up(KeyCode::KeyW);
    record(&mut h, &mut trace);

    // Then a frame at a time, sneaking for short steps, a screenshot after each, until three
    // frames past the crossing
    let mut shots = Vec::new();
    let mut glances = Vec::new();
    let mut after = 0;
    h.key_down(KeyCode::ShiftLeft);
    while after < 3 {
        h.key_down(KeyCode::KeyW);
        h.key_up(KeyCode::KeyW);
        record(&mut h, &mut trace);
        let crossed = trace.last().unwrap().1 != trace[0].1;
        let at = trace.last().unwrap().0;
        println!(
            "shot {}: {} in front of the blue portal, {} of the orange one",
            shots.len(),
            blue.signed_distance(at),
            orange.signed_distance(at)
        );
        let shot = h.screenshot();
        keep(&shot, WALKING, &format!("{:02}", shots.len()));
        shots.push((crossed, shot));
        // Within 0.12 block of a portal, the camera of that portal has no oblique near plane,
        // inside the box of the twin. A glance along the wall, from there, sees through the
        // portal what that camera sees between itself and the twin's plane. A sneaking step is
        // 0.15 block at most, and the last one before the crossing and the first after it add
        // up to one: either is within 0.075 block of its portal
        let (portal, twin, facing) = if crossed {
            (orange, PortalColor::Blue, yaw(h.world_mut()) + PI)
        } else {
            (blue, PortalColor::Orange, yaw(h.world_mut()))
        };
        if after == 0 && portal.signed_distance(at) < MIN_OBLIQUE_DISTANCE {
            let looking = yaw(h.world_mut());
            set_yaw(&mut h, facing - 85f32.to_radians());
            h.frames(1);
            let shot = h.screenshot();
            keep(&shot, WALKING, &format!("glance_{}", glances.len()));
            glances.push((twin, shot));
            set_yaw(&mut h, looking);
            h.frames(1);
        }
        after += u32::from(crossed);
        assert!(shots.len() < 40, "never went through: {:?}", trace.last());
    }
    h.key_up(KeyCode::ShiftLeft);

    // And on, out of the orange portal
    h.key_down(KeyCode::KeyW);
    record(&mut h, &mut trace);
    for _ in 0..20 {
        h.frames(1);
        record(&mut h, &mut trace);
    }
    h.key_up(KeyCode::KeyW);
    h.frames(30);
    record(&mut h, &mut trace);

    // One quarter turn to the left
    let turns: Vec<f32> = trace
        .windows(2)
        .map(|pair| pair[1].1 - pair[0].1)
        .filter(|turn| *turn != 0.0)
        .collect();
    assert_eq!(turns.len(), 1, "{turns:?}");
    assert!(
        (turns[0] - std::f32::consts::FRAC_PI_2).abs() < 1e-4,
        "{turns:?}"
    );
    // No step longer than a frame's walk, the crossing measured through the portal
    for pair in trace.windows(2) {
        let ((from, from_yaw), (to, to_yaw)) = (pair[0], pair[1]);
        let from = if from_yaw != to_yaw {
            through.transform_point3(from)
        } else {
            from
        };
        assert!(from.distance(to) < 0.6, "a jump from {from} to {to}");
    }
    // Out of the orange portal, in front of it
    let end = trace.last().unwrap().0;
    assert!(orange.signed_distance(end) > 1.0, "{end}");

    // The screen across the crossing changes no more than between two frames on one side
    let crossing = shots.iter().position(|(crossed, _)| *crossed).unwrap();
    // Nor does the colour of the twin flash on a portal, as its camera comes within 0.12 block
    // of the twin, where it would stand inside the twin's box. The orange of the fox that may
    // pass by is far less
    assert!(
        !glances.is_empty(),
        "no shot within {MIN_OBLIQUE_DISTANCE} block of a portal"
    );
    let share = |shot: &RgbImage, color: PortalColor| {
        let coloured = shot
            .pixels()
            .filter(|pixel| {
                let [r, g, b] = pixel.0.map(i32::from);
                match color {
                    PortalColor::Orange => r > g + 40 && g > b + 20,
                    // More saturated than the sky
                    PortalColor::Blue => b > r + 100 && b > g + 40,
                }
            })
            .count();
        coloured as f32 / (shot.width() * shot.height()) as f32
    };
    for (index, (_, shot)) in shots.iter().enumerate() {
        let orange = share(shot, PortalColor::Orange);
        assert!(
            orange < 0.05,
            "orange on {orange} of the screen in shot {index}"
        );
    }
    for (twin, shot) in &glances {
        let share = share(shot, *twin);
        assert!(
            share < 0.05,
            "{twin:?} on {share} of the screen in a glance"
        );
        // Nor does the rim of the portal, seen edge-on, draw a line down the screen
        if let Some(x) = hairline(shot) {
            panic!("a line down the screen at x = {x} in a glance");
        }
    }
    let differences: Vec<f32> = shots
        .windows(2)
        .map(|pair| difference(&pair[0].1, &pair[1].1))
        .collect();
    println!("differences between the frames: {differences:?}, crossing before {crossing}");
    let across = differences[crossing - 1];
    let others = differences
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != crossing - 1)
        .map(|(_, difference)| *difference)
        .fold(0.0, f32::max);
    assert!(
        across <= 2.0 * others + 1.0,
        "the screen jumps across the portal: {across}, between frames {others}"
    );
}

/// A mod with two commands, from the block its author is in: `/wall` builds a stone wall 4 wide
/// and 5 high 8 blocks north, facing south, from a block below; `/landmark` builds a block of
/// sand 3 blocks wide and 2 high, below the eyes, between 3 and 5 blocks north, in front of
/// that wall.
pub(super) const FAR_MOD: &str = r#"
    local rc = rustcraft
    local function fill(p, x0, y0, z0, x1, y1, z1, block)
        for x = x0, x1 do
            for y = y0, y1 do
                for z = z0, z1 do
                    rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + z }, block)
                end
            end
        end
    end
    rc.register_command("wall", function(ctx, args)
        fill(rc.player_position(ctx.player), -2, -1, -8, 1, 3, -8, "stone")
        return "Wall built"
    end)
    rc.register_command("landmark", function(ctx, args)
        fill(rc.player_position(ctx.player), -1, -1, -5, 1, 0, -3, "sand")
        return "Landmark built"
    end)
"#;

/// How far south the player flies from the orange portal to place the blue one, in blocks: 10
/// chunks, far beyond its render distance (`FAR_RENDER_DISTANCE`) and the margin.
const FAR_FLIGHT: i32 = 160;
const FAR_RENDER_DISTANCE: u32 = 3;

/// In solo, with a render distance of 3 chunks, the player places its orange portal, has sand
/// put in front of it, flies 10 chunks away and places its blue portal there: through the blue
/// portal it sees the sand, whose chunks the server sends it only because the portal shows them.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_view_through_a_portal_reaches_a_far_place() {
    let folder = GameFolder::new()
        .with_world("Far")
        .with_player("Far", "Tester", |player| {
            player.position = Vec3::new(0.5, 140.9, 0.5);
            player.game_mode = shared::players::game_mode::GameMode::Creative;
            player.is_flying = true;
            player.inventory.inner.insert(
                0,
                shared::world::ItemStack {
                    item_id: ItemId::PortalGun,
                    item_type: ItemId::PortalGun.get_default_type(),
                    nb: 1,
                },
            );
        })
        .with_mod("far", FAR_MOD)
        // No clouds, which drift with the clock over the sky and the sun this test looks at
        .with_file(
            crate::constants::VIDEO_SETTINGS_PATH,
            &format!("(render_distance: {FAR_RENDER_DISTANCE}, clouds: Off)"),
        );
    let mut h = enter_world(folder, "Far");
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    let in_the_air = |w: &World| {
        local_player(w).is_some_and(|player| {
            let map = &w.resource::<ClientWorldMap>().blocks;
            player.is_flying && shared::players::movement::is_terrain_loaded(player.position, map)
        })
    };
    h.wait_until("the player in the air", SERVER, in_the_air);
    look(&mut h, 0.0);

    // The orange portal on a wall to the north, the sand in front of it
    let here = local_player(h.world()).unwrap().position.floor().as_ivec3();
    // The cell of the wall at the height of the eyes, so that the portal is centred on them
    let wall = |h: &mut Harness, at: IVec3| {
        let world = h.world_mut();
        let eyes = world
            .query_filtered::<&Transform, With<CameraController>>()
            .single(world)
            .unwrap()
            .translation;
        IVec3::new(at.x, eyes.y.floor() as i32, at.z - 8)
    };
    let wall_here = wall(&mut h, here);
    assert!((here.y - 1..=here.y + 3).contains(&wall_here.y));
    h.command("/wall");
    h.wait_until("the wall", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(wall_here)
            .is_some()
    });
    h.aim_at(wall_here.as_vec3() + Vec3::new(0.5, 0.5, 1.0), wall_here);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 3);
    h.wait_until("the orange portal", SERVER, |w| {
        own_portal(w, PortalColor::Orange).is_some()
    });
    // The front of the sand, below the eyes
    let sand = here + IVec3::new(0, 0, -3);
    h.command("/landmark");
    h.wait_until("the sand", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(sand)
            .is_some()
    });

    // South, far away
    look(&mut h, 0.0);
    h.key_down(KeyCode::KeyS);
    h.wait_until("the far place", Duration::from_secs(300), |w| {
        local_player(w).is_some_and(|player| player.position.z > (here.z + FAR_FLIGHT) as f32)
    });
    h.key_up(KeyCode::KeyS);
    h.frames(20);
    h.wait_until("the player in the air", SERVER, in_the_air);
    // Out of reach: the client no longer has the sand
    assert!(h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .get_block(sand)
        .is_none());

    // The blue portal on a wall to the north of the far place, facing the player
    let there = local_player(h.world()).unwrap().position.floor().as_ivec3();
    let wall_there = wall(&mut h, there);
    h.command("/wall");
    h.wait_until("the far wall", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(wall_there)
            .is_some()
    });
    h.aim_at(wall_there.as_vec3() + Vec3::new(0.5, 0.5, 1.0), wall_there);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.wait_until("the blue portal", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_some()
    });

    // The server sends the chunks around the orange portal, 10 chunks away
    h.wait_until("the sand, through the portal", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(sand)
            .is_some()
    });
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    let portals = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .clone();
    let orange = *portals.get(blue.owner, PortalColor::Orange).unwrap();
    let player = local_player(h.world()).unwrap().position;
    let target = Vec3::from(blue.frame().translation);
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = (-(target.x - player.x)).atan2(-(target.z - player.z));
    controller.angle_y = 0.0;
    // Meshed and drawn
    h.frames(90);
    // At noon, the sky the same colour for a while
    h.set_time("day");
    h.frames(5);

    let shot = h.screenshot();
    keep(
        &shot,
        "the_view_through_a_portal_reaches_a_far_place",
        "far_place",
    );
    h.snapshot("through_the_blue_portal_to_a_far_place");
    let world = h.world_mut();
    let active: Vec<PortalColor> = world
        .query::<(&PortalCamera, &Camera)>()
        .iter(world)
        .filter(|(_, camera)| camera.is_active)
        .map(|(portal, _)| portal.color)
        .collect();
    // The orange portal, 10 chunks away, faces the player too, but beyond its fog
    assert_eq!(active, [PortalColor::Blue]);

    // The south face of the sand, seen through the blue portal: where it would be in front of
    // the blue portal, as it is in front of the orange one
    let face = sand.as_vec3() + Vec3::new(0.5, 0.5, 1.0);
    let seen = portals.through(&orange).unwrap().transform_point3(face);
    assert!(blue.signed_distance(seen) < 0.0);
    let color = mean_color(&shot, on_screen(&mut h, seen));
    assert!(
        color.x > color.z + 25.0,
        "not sand through the portal: {color}"
    );

    // Around the wall, the sky only: the ground around the orange portal, sent for the blue
    // one, is not drawn by the player's camera, beyond its fog
    let sky = h.sky();
    let is_sky = |color: Vec3, sky: Vec3| (color - sky).abs().max_element() <= 2.0;
    let around_the_wall = |shot: &RgbImage, sky: Vec3| {
        for x in [80.0, 250.0, 950.0, 1200.0] {
            for y in [200.0, 280.0, 360.0, 440.0, 560.0, 600.0] {
                let color = mean_color(shot, Vec2::new(x, y));
                assert!(is_sky(color, sky), "not the sky at ({x}, {y}): {color}");
            }
        }
    };
    around_the_wall(&shot, sky);
    // Only the portal cameras draw it
    let world = h.world_mut();
    let sand_group = crate::world::group_of(global_block_to_chunk_pos(&sand));
    let entity = world.resource::<ClientWorldMap>().render[&sand_group]
        .entity
        .unwrap();
    assert_eq!(
        world.get::<bevy::camera::visibility::RenderLayers>(entity),
        Some(&bevy::camera::visibility::RenderLayers::layer(
            crate::portal::PORTAL_CHUNKS_LAYER
        ))
    );
    // Through the portal, the wall of the blue portal, seen from behind far beyond the portal
    // camera's fog, is the sky too
    let wall_back = portals
        .through(&orange)
        .unwrap()
        .transform_point3(wall_there.as_vec3() + Vec3::new(0.5, 0.5, 0.0));
    let color = mean_color(&shot, on_screen(&mut h, wall_back));
    assert!(
        is_sky(color, sky),
        "not the sky through the portal: {color}"
    );

    // At night too: the fog follows the sky, which the clear colours of both cameras are. The
    // stars, which twinkle on a pixel or two here and there, are hidden: one would spoil a
    // pixel of the sky checked. On no layer, as their visibility is set on every frame
    // (`underwater_fog_system`)
    h.set_time("night");
    let world = h.world_mut();
    let stars: Vec<Entity> = world
        .query_filtered::<Entity, With<Stars>>()
        .iter(world)
        .collect();
    assert!(!stars.is_empty());
    for star in stars {
        world
            .entity_mut(star)
            .insert(bevy::camera::visibility::RenderLayers::none());
    }
    h.frames(10);
    let shot = h.screenshot();
    keep(
        &shot,
        "the_view_through_a_portal_reaches_a_far_place",
        "far_place_night",
    );
    let night = h.sky();
    assert!(night.max_element() < 40.0, "not a night sky: {night}");
    around_the_wall(&shot, night);
    let color = mean_color(&shot, on_screen(&mut h, wall_back));
    assert!(
        is_sky(color, night),
        "not the night sky through the portal: {color}"
    );

    // Up to the portal, then, once the sun goes down in the south, a look through the portal at
    // it: the sun is drawn where it is in the sky of the far place, not around the player
    h.key_down(KeyCode::KeyW);
    h.wait_until("the player near the portal", SERVER, |w| {
        local_player(w).is_some_and(|player| blue.signed_distance(player.position) < 3.5)
    });
    h.key_up(KeyCode::KeyW);
    let eyes = |w: &mut World| {
        w.query_filtered::<&Transform, With<CameraController>>()
            .single(w)
            .unwrap()
            .translation
    };
    let sun = |w: &mut World| {
        let root = w
            .query_filtered::<&Transform, With<CelestialRoot>>()
            .single(w)
            .unwrap()
            .rotation;
        root * Vec3::Z
    };
    // Looking through the blue portal, which faces south, is looking south out of the orange
    // one: the player looks north, as high as the sun is
    let toward = |sun: Vec3| Vec3::new(-sun.x, sun.y, -sun.z);
    // The sun in the south, above the horizon, low enough to be seen through the opening: it
    // sets in the south a quarter of a day after noon. A day lasts twenty minutes, so the
    // client's clock, which the sky follows and the server only sets on `/time set`, is set
    // straight to the first such hour after the next noon
    let eyes_now = eyes(h.world_mut());
    let now = h.world().resource::<ClientTime>().0;
    let noon = now - now % DAY_DURATION_IN_TICKS + NOON_IN_DAY;
    let low_in_the_south = (noon..noon + DAY_DURATION_IN_TICKS / 2)
        .step_by(10)
        .find(|&tick| {
            let sun = sky_rotation(tick) * Vec3::Z;
            let ray = toward(sun);
            let along = -blue.signed_distance(eyes_now) / ray.dot(blue.normal());
            let hit = blue.local(eyes_now + ray * along);
            sun.z > 0.0 && sun.y > 0.03 && hit.x.abs() < 0.7 && hit.y.abs() < 1.2
        })
        .expect("the sun is seen through the portal some time in the afternoon");
    h.world_mut().insert_resource(ClientTime(low_in_the_south));
    h.frames(2);
    let look_at_sun = |h: &mut Harness| {
        let direction = toward(sun(h.world_mut()));
        let world = h.world_mut();
        let mut controller = world
            .query::<&mut CameraController>()
            .single_mut(world)
            .unwrap();
        controller.angle_x = (-direction.x).atan2(-direction.z);
        controller.angle_y = -direction.y.asin();
    };
    look_at_sun(&mut h);
    h.frames(1);
    look_at_sun(&mut h);
    h.frames(1);
    // Where the sun is as the screenshot is taken: it moves on while it is read back
    let eyes = eyes(h.world_mut());
    let direction = toward(sun(h.world_mut()));
    let at = on_screen(&mut h, eyes + direction * 100.0);
    let shot = h.screenshot();
    keep(
        &shot,
        "the_view_through_a_portal_reaches_a_far_place",
        "far_place_sun",
    );
    assert!(blue.signed_distance(eyes + direction * 10.0) < 0.0);
    // Beside the crosshair, on the disk of the sun, which is drawn so bright that it shows white
    let color = mean_color(&shot, at + Vec2::splat(6.0));
    let sky = h.sky();
    assert!(
        color.min_element() > 240.0 && sky.max_element() < 200.0,
        "not the sun through the portal: {color}, the sky being {sky}"
    );
}

/// A mod with two commands, from the block its author is in: `/floorcourse` builds a stone
/// platform 3 blocks down and, on it, a stone wall 2 wide and 3 high 6 blocks east, its face
/// to the west; `/pillar` a pillar of gold as wide and high, 4 blocks west of that wall.
const FLOOR_COURSE_MOD: &str = r#"
    local rc = rustcraft
    local function fill(p, x0, y0, z0, x1, y1, z1, block)
        for x = x0, x1 do
            for y = y0, y1 do
                for z = z0, z1 do
                    rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + z }, block)
                end
            end
        end
    end
    rc.register_command("floorcourse", function(ctx, args)
        local p = rc.player_position(ctx.player)
        fill(p, -6, -3, -9, 8, -3, 6, "stone")
        fill(p, 6, -2, -2, 6, 0, -1, "stone")
        return "Course built"
    end)
    rc.register_command("pillar", function(ctx, args)
        fill(rc.player_position(ctx.player), 2, -2, -2, 2, 0, -1, "gold_block")
        return "Pillar built"
    end)
"#;

const FALLING: &str = "falling_through_a_floor_portal_comes_out_of_a_wall";

/// In solo, the player hovers over a platform, shoots its orange portal on a wall and its blue
/// one on the platform under it: looking down at the blue portal, it sees out of the wall, a
/// pillar of gold standing in front of it. Then it stops flying, falls into the floor and comes
/// out of the wall, flying west as fast as it fell, set upright, and lands on the platform. The
/// server simulates the fall as the client predicts it: nothing jumps but the crossing.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn falling_through_a_floor_portal_comes_out_of_a_wall() {
    let folder = GameFolder::new()
        .with_world("FloorCourse")
        .with_player("FloorCourse", "Tester", |player| {
            player.position = Vec3::new(0.5, 120.9, 0.5);
            player.game_mode = shared::players::game_mode::GameMode::Creative;
            player.is_flying = true;
            player.inventory.inner.insert(
                0,
                shared::world::ItemStack {
                    item_id: ItemId::PortalGun,
                    item_type: ItemId::PortalGun.get_default_type(),
                    nb: 1,
                },
            );
        })
        .with_mod("floorcourse", FLOOR_COURSE_MOD);
    let mut h = enter_world(folder, "FloorCourse");
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    h.wait_until("the player in the air", SERVER, |w| {
        local_player(w).is_some_and(|player| {
            let map = &w.resource::<ClientWorldMap>().blocks;
            player.is_flying && shared::players::movement::is_terrain_loaded(player.position, map)
        })
    });
    h.set_time("day");
    let start = local_player(h.world()).unwrap().position.floor().as_ivec3();
    h.command("/floorcourse");
    let floor_cell = start + IVec3::new(0, -3, 0);
    let wall_cell = start + IVec3::new(6, -1, -2);
    h.wait_until("the course", SERVER, |w| {
        let blocks = &w.resource::<ClientWorldMap>().blocks;
        [start + IVec3::new(8, -3, 6), floor_cell, wall_cell]
            .iter()
            .all(|cell| blocks.get_block(*cell).is_some())
    });
    h.frames(10);

    // Orange on the wall, then blue on the platform under the player, looking down to the
    // north: its long side goes north
    h.aim_at(wall_cell.as_vec3() + Vec3::new(0.0, 0.5, 0.5), wall_cell);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 3);
    h.aim_at(floor_cell.as_vec3() + Vec3::new(0.5, 1.0, 0.45), floor_cell);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.wait_until("both portals", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_some() && own_portal(w, PortalColor::Orange).is_some()
    });
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    let orange = own_portal(h.world(), PortalColor::Orange).unwrap();
    assert_eq!(
        (blue.face, blue.up),
        (FaceDirection::PlusY, FaceDirection::MinusZ)
    );
    assert_eq!(orange.face, FaceDirection::MinusX);
    let through = h
        .world()
        .resource::<ClientWorldMap>()
        .blocks
        .portals()
        .through(&blue)
        .unwrap();

    // The pillar in front of the wall, seen through the floor, looking down at the middle of
    // the blue portal, the top of the view to the north
    h.command("/pillar");
    let pillar = start + IVec3::new(2, -1, -1);
    h.wait_until("the pillar", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(pillar)
            .is_some()
    });
    let centre = Vec3::from(blue.frame().translation);
    let eyes = local_player(h.world()).unwrap().position + Vec3::Y * 0.8;
    let direction = (centre - eyes).normalize();
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = (-direction.x).atan2(-direction.z);
    controller.angle_y = -direction.y.asin();
    h.frames(10);
    let at = on_screen(&mut h, centre);
    let shot = h.screenshot();
    keep(&shot, FALLING, "through_the_floor_portal");
    assert_eq!(
        drawing(h.world_mut()).first(),
        Some(&(PortalColor::Blue, 1))
    );
    // Beside the crosshair
    let gold = mean_color(&shot, at + Vec2::splat(12.0));
    assert!(
        gold.x > gold.z + 40.0 && gold.y > gold.z + 25.0,
        "not the gold pillar through the floor portal: {gold}"
    );

    // Falling, a frame at a time: where the player is, how fast it goes, where it looks
    let mut trace: Vec<(Vec3, Vec3, f32, f32)> = Vec::new();
    let record = |h: &mut Harness, trace: &mut Vec<(Vec3, Vec3, f32, f32)>| {
        let player = local_player(h.world()).unwrap();
        let world = h.world_mut();
        let controller = world.query::<&CameraController>().single(world).unwrap();
        trace.push((
            player.position,
            player.velocity,
            controller.angle_x,
            controller.angle_y,
        ));
    };
    record(&mut h, &mut trace);
    h.press(KeyCode::KeyF);
    for _ in 0..120 {
        h.frames(1);
        record(&mut h, &mut trace);
        let (position, ..) = *trace.last().unwrap();
        if orange.signed_distance(position) > 0.0 && local_player(h.world()).unwrap().on_ground {
            break;
        }
    }
    let crossing = trace
        .windows(2)
        .position(|pair| pair[1].0.x - pair[0].0.x > 3.0)
        .unwrap_or_else(|| panic!("never came out of the wall: {:?}", trace.last()));
    let (before, after) = (trace[crossing], trace[crossing + 1]);
    println!("before the crossing {before:?}, after it {after:?}");
    // Out of the wall, west, as fast as it fell, give or take a frame's gravity
    let (fall, flight) = (-before.1.y, -after.1.x);
    assert!(fall > 5.0, "fell at {fall}");
    assert!(
        flight > fall - 0.1 && flight < fall + 3.1,
        "flew out at {flight}, fell at {fall}"
    );
    // Upright, looking west, a little north as the look went a little east into the floor
    assert!(
        (after.2 - std::f32::consts::FRAC_PI_2).cos() > 0.95,
        "{after:?}"
    );
    assert!(after.3.abs() < 0.2, "{after:?}");
    // No jump back by a correction of the server, the portals being 6 blocks apart: no step
    // longer than 2 blocks, the crossing measured through the portal
    for (index, pair) in trace.windows(2).enumerate() {
        let from = if index == crossing {
            through.transform_point3(pair[0].0)
        } else {
            pair[0].0
        };
        assert!(
            from.distance(pair[1].0) < 2.0,
            "a jump from {from} to {}",
            pair[1].0
        );
    }
    // On the platform, west of the wall, standing still
    let shot = h.screenshot();
    keep(&shot, FALLING, "out_of_the_wall");
    let player = local_player(h.world()).unwrap();
    assert!(player.on_ground, "{:?}", trace.last());
    assert!(
        player.position.x < start.x as f32 + 5.5,
        "{}",
        player.position
    );
    assert_eq!(player.velocity, Vec3::ZERO);
}

/// A mod with a command, `/throwcourse`, that builds from the block its author is in: a stone
/// platform 3 blocks down, with a hole 4 blocks north on a stone below it, and on it a stone
/// wall 2 wide and 3 high, 5 blocks west and 8 north, its face to the east.
const THROW_COURSE_MOD: &str = r#"
    local rc = rustcraft
    local function fill(p, x0, y0, z0, x1, y1, z1, block)
        for x = x0, x1 do
            for y = y0, y1 do
                for z = z0, z1 do
                    rc.set_block({ x = p.x + x, y = p.y + y, z = p.z + z }, block)
                end
            end
        end
    end
    rc.register_command("throwcourse", function(ctx, args)
        local p = rc.player_position(ctx.player)
        fill(p, -8, -3, -10, 6, -3, -5, "stone")
        fill(p, -8, -3, -3, 6, -3, 4, "stone")
        fill(p, -8, -3, -4, -1, -3, -4, "stone")
        fill(p, 1, -3, -4, 6, -3, -4, "stone")
        fill(p, 0, -4, -4, 0, -4, -4, "stone")
        fill(p, -5, -2, -8, -5, 0, -7, "stone")
        return "Course built"
    end)
"#;

const THROWN: &str = "an_item_thrown_into_a_floor_portal_comes_out_of_a_wall";

/// Where the dropped items are drawn.
fn shown_items(w: &mut World) -> Vec<Vec3> {
    w.query_filtered::<&Transform, With<crate::dropped_items::DroppedItemRoot>>()
        .iter(w)
        .map(|transform| transform.translation)
        .collect()
}

/// In solo, the player stands on a platform with its blue portal on the floor just ahead,
/// its long side north, and its orange one on a wall to the north-west, facing east. Its
/// inventory full, it pours a water bucket into a hole beyond the floor portal, steps to the
/// edge of the portal, and takes the water back with a second bucket: the water bucket it has
/// no room for is thrown ahead, falls into the floor and flies east out of the wall, where it
/// lands. The client draws it jumping from one portal to the other, never between them.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn an_item_thrown_into_a_floor_portal_comes_out_of_a_wall() {
    let folder = GameFolder::new()
        .with_world("ThrowCourse")
        .with_player("ThrowCourse", "Tester", |player| {
            player.position = Vec3::new(0.5, 120.9, 0.5);
            player.game_mode = shared::players::game_mode::GameMode::Creative;
            player.is_flying = true;
            let stack = |item_id: ItemId, nb| shared::world::ItemStack {
                item_id,
                item_type: item_id.get_default_type(),
                nb,
            };
            let inventory = &mut player.inventory.inner;
            inventory.insert(0, stack(ItemId::PortalGun, 1));
            inventory.insert(1, stack(ItemId::Bucket, 2));
            inventory.insert(2, stack(ItemId::WaterBucket, 1));
            for slot in 3..shared::MAX_INVENTORY_SLOTS {
                inventory.insert(slot, stack(ItemId::Stone, 64));
            }
        })
        .with_mod("throwcourse", THROW_COURSE_MOD);
    let mut h = enter_world(folder, "ThrowCourse");
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    h.wait_until("the player in the air", SERVER, |w| {
        local_player(w).is_some_and(|player| {
            let map = &w.resource::<ClientWorldMap>().blocks;
            player.is_flying && shared::players::movement::is_terrain_loaded(player.position, map)
        })
    });
    h.set_time("day");
    let start = local_player(h.world()).unwrap().position.floor().as_ivec3();
    h.command("/throwcourse");
    let floor_cell = start + IVec3::new(1, -3, -2);
    let wall_cell = start + IVec3::new(-5, -1, -8);
    let hole = start + IVec3::new(0, -3, -4);
    h.wait_until("the course", SERVER, |w| {
        let blocks = &w.resource::<ClientWorldMap>().blocks;
        [start + IVec3::new(6, -3, 4), hole - IVec3::Y, wall_cell]
            .iter()
            .all(|cell| blocks.get_block(*cell).is_some())
    });
    // Down onto the platform
    h.press(KeyCode::KeyF);
    h.wait_until("the player on the platform", SERVER, |w| {
        local_player(w).is_some_and(|player| player.on_ground)
    });
    h.frames(10);

    // Orange on the wall, then blue on the floor ahead, looking down to the north
    h.aim_at(wall_cell.as_vec3() + Vec3::new(1.0, 0.5, 0.5), wall_cell);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 3);
    h.aim_at(floor_cell.as_vec3() + Vec3::new(0.05, 1.0, 0.5), floor_cell);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.wait_until("both portals", SERVER, |w| {
        own_portal(w, PortalColor::Blue).is_some() && own_portal(w, PortalColor::Orange).is_some()
    });
    let blue = own_portal(h.world(), PortalColor::Blue).unwrap();
    let orange = own_portal(h.world(), PortalColor::Orange).unwrap();
    assert_eq!(
        (blue.face, blue.up),
        (FaceDirection::PlusY, FaceDirection::MinusZ)
    );
    assert!(blue
        .cells()
        .any(|cell| cell == start + IVec3::new(0, -3, -1)));
    assert_eq!(orange.face, FaceDirection::PlusX);

    // North by short sneaking steps over the edge of the floor portal, short of where the
    // floor no longer holds the player, its centre 0.3 block over the portal: a step of a slow
    // frame goes 0.13 block
    set_yaw(&mut h, 0.0);
    h.key_down(KeyCode::ShiftLeft);
    for _ in 0..300 {
        if local_player(h.world()).unwrap().position.z < start.z as f32 - 0.05 {
            break;
        }
        h.key_down(KeyCode::KeyW);
        h.frames(1);
        h.key_up(KeyCode::KeyW);
        h.frames(1);
    }
    h.key_up(KeyCode::ShiftLeft);
    h.frames(10);
    let player = local_player(h.world()).unwrap();
    assert!(player.on_ground, "{}", player.position);
    assert!(
        blue.signed_distance(player.position) > 0.5,
        "{}",
        player.position
    );

    // The water bucket poured into the hole, aimed at its far side over the floor portal
    h.press(KeyCode::Digit3);
    let far_side = hole + IVec3::NEG_Z;
    h.aim_at(far_side.as_vec3() + Vec3::new(0.5, 0.9, 1.0), far_side);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_until("the water in the hole", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(hole)
            .is_some_and(|block| block.id == BlockId::Water)
    });

    // The second bucket takes the water back: the water bucket is thrown north, into the floor
    h.press(KeyCode::Digit2);
    let eyes = player.position + Vec3::Y * 0.8;
    let water_top = hole.as_vec3() + Vec3::new(0.5, 0.95, 0.5);
    let direction = (water_top - eyes).normalize();
    let world = h.world_mut();
    let mut controller = world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap();
    controller.angle_x = (-direction.x).atan2(-direction.z);
    controller.angle_y = -direction.y.asin();
    h.frames(5);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_until("the thrown water bucket", SERVER, |w| {
        w.try_query::<&crate::dropped_items::DroppedItemRoot>()
            .is_some_and(|mut items| items.iter(w).count() == 1)
    });
    assert_eq!(slot(h.world(), 1), Some((ItemId::Bucket, 1)));

    // Where it is drawn, a frame at a time, until it lies still
    let mut trace = vec![shown_items(h.world_mut())[0]];
    for _ in 0..300 {
        h.frames(1);
        trace.push(shown_items(h.world_mut())[0]);
        let still = trace.len() > 30
            && trace[trace.len() - 30..]
                .windows(2)
                .all(|pair| pair[0].distance(pair[1]) < 1e-3);
        if still {
            break;
        }
    }
    // Drawn over the floor portal until the server says it passed, in front of the wall
    // portal after, never between: one jump. A frame of the e2e lasts several ticks, under
    // load enough for the item to pass before it is first drawn, and to land before it is
    // drawn out of the wall
    let over_blue = |at: Vec3| {
        let local = blue.local(at);
        local.x.abs() < 1.2 && local.y.abs() < 1.7 && local.z < 2.0
    };
    let out_of_orange =
        |at: Vec3| orange.signed_distance(at) > 0.0 && orange.local(at).x.abs() < 1.2;
    let crossing = trace
        .iter()
        .position(|&at| !over_blue(at))
        .unwrap_or(trace.len());
    match crossing {
        0 => println!("first drawn out of the wall, at {}", trace[0]),
        _ => println!(
            "drawn before the crossing at {}, after it at {:?}",
            trace[crossing - 1],
            trace.get(crossing)
        ),
    }
    assert!(
        trace[crossing..].iter().all(|&at| out_of_orange(at)),
        "{trace:?}"
    );
    assert!(crossing < trace.len(), "never came out: {trace:?}");
    // Lying on the platform east of the wall, out of the orange portal
    let rest = *trace.last().unwrap();
    assert!(orange.signed_distance(rest) > 0.5, "{rest}");
    assert!(
        (rest.y - (start.y as f32 - 2.0 + 0.125)).abs() < 0.3,
        "{rest}"
    );
    let shot = h.screenshot();
    keep(&shot, THROWN, "thrown");
}

const SHOT: &str = "the_shot_flies_to_the_wall_and_the_portal_opens_where_it_lands";

/// The ages and flight times of the bolts in the world.
fn bolts(w: &World) -> Vec<(f32, f32)> {
    w.try_query::<&Bolt>()
        .unwrap()
        .iter(w)
        .map(|bolt| (bolt.age, bolt.flight()))
        .collect()
}

/// How many entities draw a portal: its box, its rim and its surfaces.
fn portal_drawings(w: &World) -> usize {
    w.try_query::<&MeshMaterial3d<PortalMaterial>>()
        .unwrap()
        .iter(w)
        .count()
}

/// Whether a pixel of `shot` within 8 pixels of `at` is the bright blue of a bolt.
fn bolt_blue_near(shot: &RgbImage, at: Vec2) -> bool {
    near(shot, at, |[r, g, b]| b > 200 && b > r + 60 && g > r)
}

/// In solo, the player shoots its blue portal at a wall: the bolt flies there, the portal not
/// drawn while it does, though the client map has it; it bursts into sparks and the portal is
/// drawn. Time is held still to look at the bolt mid-flight and at the sparks.
#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_shot_flies_to_the_wall_and_the_portal_opens_where_it_lands() {
    let folder = GameFolder::new()
        .with_world("Shot")
        .with_items("Shot", "Tester", &[(0, ItemId::PortalGun, 1)])
        .with_mod("walls", WALLS_MOD);
    let mut h = enter_world(folder, "Shot");
    h.wait_until("the gun in the hand", SERVER, |w| {
        slot(w, 0) == Some((ItemId::PortalGun, 1))
    });
    h.wait_until("the player on the ground", SERVER, |w| {
        local_player(w).is_some_and(|player| {
            let ground = w
                .resource::<ClientWorldMap>()
                .blocks
                .get_height_ground(player.position);
            player.on_ground && ground > 0 && player.position.y < ground as f32 + 2.5
        })
    });
    h.frames(30);
    h.set_time("day");
    let centre = local_player(h.world()).unwrap().position.floor().as_ivec3();
    h.command("/walls");
    let north = centre + IVec3::new(0, 11, -8);
    h.wait_until("the wall", SERVER, |w| {
        w.resource::<ClientWorldMap>()
            .blocks
            .get_block(north)
            .is_some()
    });
    h.aim_at(north.as_vec3() + Vec3::new(0.5, 0.5, 1.0), north);
    let yaw = yaw(h.world_mut());

    // Time held still, the shot is fired: the bolt waits by the eye
    h.world_mut().resource_mut::<Time<Virtual>>().pause();
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Left, 3);
    h.wait_until("the bolt", SERVER, |w| !bolts(w).is_empty());
    assert!(own_portal(h.world(), PortalColor::Blue).is_some());
    assert_eq!(
        portal_drawings(h.world()),
        0,
        "drawn before the bolt landed"
    );

    // Halfway, seen from a little aside
    let world = h.world_mut();
    let mut bolt = world.query::<&mut Bolt>().single_mut(world).unwrap();
    bolt.age = bolt.flight() * 0.45;
    let at = bolt.position();
    set_yaw(&mut h, yaw + 0.5);
    h.frames(3);
    let on = on_screen(&mut h, at);
    let shot = h.screenshot();
    keep(&shot, SHOT, "mid_flight");
    assert!(bolt_blue_near(&shot, on), "no bolt at {on}");
    assert_eq!(
        portal_drawings(h.world()),
        0,
        "drawn before the bolt landed"
    );

    // It lands, time still held: it bursts into sparks, and the portal is drawn
    set_yaw(&mut h, yaw);
    let world = h.world_mut();
    let mut bolt = world.query::<&mut Bolt>().single_mut(world).unwrap();
    bolt.age = bolt.flight();
    h.frames(1);
    let sparks = {
        let world = h.world_mut();
        world
            .query::<&crate::portal::particles::Particle>()
            .iter(world)
            .count()
    };
    assert!(sparks > 0, "no sparks");
    assert!(
        portal_drawings(h.world()) > 0,
        "not drawn as the bolt landed"
    );
    // A tenth of a second for them to fly apart, in four frames of 25 ms whatever lavapipe takes
    h.world_mut()
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            25,
        )));
    h.world_mut().resource_mut::<Time<Virtual>>().unpause();
    h.frames(4);
    h.world_mut().resource_mut::<Time<Virtual>>().pause();
    h.world_mut().insert_resource(TimeUpdateStrategy::Automatic);
    let shot = h.screenshot();
    keep(&shot, SHOT, "sparks");
    h.world_mut().resource_mut::<Time<Virtual>>().unpause();

    // The trail shrinks into the wall, and the bolt is gone
    h.wait_until("the bolt gone", SERVER, |w| bolts(w).is_empty());
    h.frames(5);
    keep(&h.screenshot(), SHOT, "opened");

    // Another player's shot, from the west below, refused: seen across the view, halfway,
    // without the shaders and with them, where it glows
    let to = north.as_vec3() + Vec3::new(1.5, 0.5, 1.0);
    let across = PortalShot {
        from: to + Vec3::new(-12.0, -6.0, 4.0),
        to,
        color: PortalColor::Orange,
        placed: false,
    };
    for quality in [ShaderQuality::Off, ShaderQuality::Medium] {
        h.world_mut().resource_mut::<GraphicsSettings>().shaders = quality;
        // The effects inserted and the exposure settled
        h.frames(40);
        h.world_mut().resource_mut::<Time<Virtual>>().pause();
        h.world_mut().write_message(ShotFired {
            shooter: 99,
            shot: across,
        });
        h.wait_until("the other bolt", SERVER, |w| !bolts(w).is_empty());
        let world = h.world_mut();
        let mut bolt = world.query::<&mut Bolt>().single_mut(world).unwrap();
        bolt.age = bolt.flight() * 0.5;
        let at = bolt.position();
        h.frames(3);
        let on = on_screen(&mut h, at);
        let shot = h.screenshot();
        let name = format!("across_{quality:?}").to_lowercase();
        keep(&shot, SHOT, &name);
        assert!(
            orange_near(&shot, on),
            "{quality:?}: no orange bolt at {on}"
        );
        h.world_mut().resource_mut::<Time<Virtual>>().unpause();
        h.wait_until("the other bolt gone", SERVER, |w| bolts(w).is_empty());
    }
}

/// Whether a pixel of `shot` within 8 pixels of `at` is the bright orange of a bolt.
fn orange_near(shot: &RgbImage, at: Vec2) -> bool {
    near(shot, at, |[r, g, b]| r > 200 && r > b + 80 && g > b)
}

/// Whether a pixel of `shot` within 8 pixels of `at` is as `wanted` says.
fn near(shot: &RgbImage, at: Vec2, wanted: impl Fn([i32; 3]) -> bool) -> bool {
    (-8..=8).any(|dx| {
        (-8..=8).any(|dy| {
            let (x, y) = (at.x as i32 + dx, at.y as i32 + dy);
            x >= 0
                && y >= 0
                && x < shot.width() as i32
                && y < shot.height() as i32
                && wanted(shot.get_pixel(x as u32, y as u32).0.map(i32::from))
        })
    })
}

/// The hour `bench_portal_weather_and_haze` holds.
#[derive(Resource)]
struct HeldTick(u64);

/// The weather `bench_portal_weather_and_haze` holds, whatever the levels would ease to.
#[derive(Resource)]
struct HeldWeather(crate::world::weather::ClientWeather);

/// What the drops and the haze cost the portal cameras on the GPU, between the facing portals,
/// render distance 8, looking into the blue portal with recursion 3 (up to six portal cameras),
/// in turns over several rounds: under a clear sky and in rain, with the drops in every view and
/// only in the player's, with the haze of the portal cameras as built (no more steps than High
/// past level 1), with the steps of the quality at every level, and with a single step;
/// recursion 1, and out of view. Then how much the capped haze changes the picture at dusk. The
/// GPU time of each frame is read back as the GPU finishes it (`record_gpu_frames`). Needs the
/// GPU of the machine:
/// `BENCH_GPU=1 BENCH_GPU_TIMES=1 BENCH_SHADERS=high cargo test -p client --release
/// e2e::portals::bench_portal_weather_and_haze -- --ignored --nocapture`.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_portal_weather_and_haze() {
    use crate::portal::PortalRecursion;
    use crate::world::precipitation::render::PrecipitationView;
    use crate::world::rendering::bench::GPU_FRAMES_MS;
    use crate::world::weather::{ease_weather_system, ClientWeather};
    use bevy::light::VolumetricFog;
    use shared::world::WeatherKind;

    assert!(
        std::env::var_os("BENCH_GPU").is_some() && std::env::var_os("BENCH_GPU_TIMES").is_some(),
        "BENCH_GPU=1 BENCH_GPU_TIMES=1"
    );
    let shaders = match std::env::var("BENCH_SHADERS").as_deref() {
        Ok("ultra") => ShaderQuality::Ultra,
        _ => ShaderQuality::High,
    };
    let folder = GameFolder::new()
        .with_file("video.ron", "(render_distance: 8)")
        .with_world("Facing");
    let options = Options {
        shaders: Some(shaders),
        ..default()
    };
    let mut h = enter_world_with(folder, "Facing", options);
    build_facing_scene(&mut h);
    let target = h.target.clone();
    h.set_camera_targets(RenderTarget::Image(target.into()));
    h.world_mut()
        .insert_resource(HeldWeather(ClientWeather::settled(WeatherKind::Clear)));
    h.app.add_systems(
        Update,
        (|held: Res<HeldWeather>, mut weather: ResMut<ClientWeather>| {
            *weather = held.0.clone();
        })
        .after(ease_weather_system),
    );

    #[derive(Clone, Copy, PartialEq)]
    enum Haze {
        /// The steps of the quality in every view, as before lot 12.
        Full,
        /// As built: no more steps than High past level 1.
        AsBuilt,
        OneStep,
    }
    let high = crate::world::rendering::shaders::ShaderFeatures::of(ShaderQuality::High)
        .god_rays
        .unwrap();
    let set_haze = |h: &mut Harness, haze: Haze| {
        let world = h.world_mut();
        let full = world
            .query_filtered::<&VolumetricFog, With<CameraController>>()
            .single(world)
            .unwrap()
            .step_count;
        let mut cameras = world.query::<(&PortalCamera, &mut VolumetricFog)>();
        for (camera, mut fog) in cameras.iter_mut(world) {
            fog.step_count = match haze {
                Haze::Full => full,
                Haze::AsBuilt if camera.level > 1 => full.min(high),
                Haze::AsBuilt => full,
                Haze::OneStep => 1,
            };
        }
    };
    let set_portal_drops = |h: &mut Harness, on: bool| {
        let world = h.world_mut();
        let cameras: Vec<Entity> = world
            .query_filtered::<Entity, With<PortalCamera>>()
            .iter(world)
            .collect();
        for camera in cameras {
            if on {
                world.entity_mut(camera).insert(PrecipitationView);
            } else {
                world.entity_mut(camera).remove::<PrecipitationView>();
            }
        }
    };
    let cases: [(&str, usize, WeatherKind, bool, Haze, f32); 7] = [
        (
            "clear, recursion 3",
            3,
            WeatherKind::Clear,
            true,
            Haze::AsBuilt,
            0.0,
        ),
        (
            "rain, recursion 3",
            3,
            WeatherKind::Rain,
            true,
            Haze::AsBuilt,
            0.0,
        ),
        (
            "rain, recursion 3, no drops in the portal views",
            3,
            WeatherKind::Rain,
            false,
            Haze::AsBuilt,
            0.0,
        ),
        (
            "clear, recursion 3, the haze steps of the quality at every level",
            3,
            WeatherKind::Clear,
            true,
            Haze::Full,
            0.0,
        ),
        (
            "clear, recursion 3, one haze step in the portal views",
            3,
            WeatherKind::Clear,
            true,
            Haze::OneStep,
            0.0,
        ),
        (
            "clear, recursion 1",
            1,
            WeatherKind::Clear,
            true,
            Haze::AsBuilt,
            0.0,
        ),
        (
            "clear, out of view",
            3,
            WeatherKind::Clear,
            true,
            Haze::AsBuilt,
            PI / 2.0,
        ),
    ];
    let (rounds, warm, frames) = (4, 30, 200);
    let mut samples = vec![Vec::new(); cases.len()];
    for _ in 0..rounds {
        for (case, &(_, depth, kind, drops, haze, yaw)) in cases.iter().enumerate() {
            h.world_mut().insert_resource(PortalRecursion(depth));
            h.world_mut()
                .insert_resource(HeldWeather(ClientWeather::settled(kind)));
            look(&mut h, yaw);
            h.frames(warm);
            set_haze(&mut h, haze);
            set_portal_drops(&mut h, drops);
            h.frames(warm);
            GPU_FRAMES_MS.lock().unwrap().clear();
            h.frames(frames);
            samples[case].extend(GPU_FRAMES_MS.lock().unwrap().drain(..));
            set_haze(&mut h, Haze::AsBuilt);
            set_portal_drops(&mut h, true);
        }
    }
    // How much the haze capped changes the picture, against two pictures as built, at dusk,
    // when the haze is thickest
    let now = h.world().resource::<ClientTime>().0;
    let noon = now - now % DAY_DURATION_IN_TICKS + NOON_IN_DAY + DAY_DURATION_IN_TICKS;
    let dusk = (noon..noon + DAY_DURATION_IN_TICKS)
        .step_by(5)
        .find(|tick| sun_height(*tick) < 0.06)
        .unwrap();
    h.world_mut().insert_resource(HeldTick(dusk));
    h.app.add_systems(
        FixedPostUpdate,
        (|held: Res<HeldTick>, mut time: ResMut<ClientTime>| time.0 = held.0)
            .after(crate::world::time::time_update_system),
    );
    h.world_mut().insert_resource(PortalRecursion(3));
    h.world_mut()
        .insert_resource(HeldWeather(ClientWeather::settled(WeatherKind::Clear)));
    look(&mut h, 0.0);
    let apart = |a: &RgbImage, b: &RgbImage| {
        let sum: f64 = a
            .pixels()
            .zip(b.pixels())
            .map(|(a, b)| {
                (0..3)
                    .map(|c| (a[c] as f64 - b[c] as f64).abs())
                    .sum::<f64>()
                    / 3.
            })
            .sum();
        sum / a.pixels().len() as f64
    };
    for (what, haze) in [
        ("the haze steps of High past level 1", Haze::AsBuilt),
        ("one haze step in the portal views", Haze::OneStep),
    ] {
        set_haze(&mut h, Haze::Full);
        h.frames(warm);
        let built = h.screenshot();
        h.frames(10);
        let again = h.screenshot();
        set_haze(&mut h, haze);
        h.frames(warm);
        let capped = h.screenshot();
        println!(
            "{what}: the picture {:.3} apart on average, two as built {:.3}",
            apart(&built, &capped),
            apart(&built, &again)
        );
    }
    for ((what, ..), mut samples) in cases.iter().zip(samples) {
        samples.sort_by(f64::total_cmp);
        println!(
            "{what}: GPU median {:.3} ms, p95 {:.3} ms per frame ({} frames)",
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100],
            samples.len()
        );
    }
}
