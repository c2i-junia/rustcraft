//! The portals, seen in a world. Placing them comes later: here the portals and their walls are
//! put into the client's map, as décor the server knows nothing of.

use std::time::Instant;

use bevy::{camera::RenderTarget, prelude::*};
use image::RgbImage;
use shared::world::{BlockData, BlockDirection, BlockId, FaceDirection, Portal, PortalColor};
use shared::CHUNK_SIZE;

use super::game::enter_world;
use super::harness::{Harness, SERVER};
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::CurrentPlayerMarker;
use crate::portal::PortalCamera;
use crate::world::ClientWorldMap;

/// The scene, in blocks from the cell of the player's feet.
struct Scene {
    feet: IVec3,
    blue: Portal,
    orange: Portal,
}

/// Through the blue portal, the player's view moves this far: the orange portal faces the other
/// way, 12 blocks east and 5 north.
const THROUGH: Vec3 = Vec3::new(12.0, 0.0, -5.0);

/// Waits for the player to stand on the ground, then builds around it, the player looking north
/// (-Z): an arena cleared of the terrain, with a stone floor, a wall 8 blocks ahead with the blue
/// portal facing the player, and further, to the east, a wall with the orange portal facing
/// north. Six blocks north of the orange portal stands a pillar of sand; between the portal and
/// where its camera stands, south of its wall, a pillar of cactus, which the camera must not see.
/// A second pillar of sand stands to the left of the blue portal, seen straight at the same
/// depth.
fn build_scene(h: &mut Harness) -> Scene {
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
                    .flat_map(move |y| [-24, -1].map(|z| center + IVec3::new(x, y, z)))
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
    let mut map = world.resource_mut::<ClientWorldMap>();
    let blocks = &mut map.blocks;
    let feet = IVec3::new(
        player.x.floor() as i32,
        blocks.get_height_ground(player) + 1,
        player.z.floor() as i32,
    );
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
        IVec3::new(13, 3, -12),
        Some(BlockId::Cobblestone),
    );
    fill(
        IVec3::new(12, 0, -18),
        IVec3::new(13, 2, -18),
        Some(BlockId::Sand),
    );
    fill(
        IVec3::new(12, 0, -8),
        IVec3::new(13, 2, -8),
        Some(BlockId::Cactus),
    );
    fill(
        IVec3::new(-7, 0, -14),
        IVec3::new(-6, 2, -14),
        Some(BlockId::Sand),
    );

    let blue = Portal {
        owner: 1,
        color: PortalColor::Blue,
        anchor: feet + IVec3::new(-1, 0, -8),
        face: FaceDirection::PlusZ,
    };
    // Facing north, its left is to the east
    let orange = Portal {
        owner: 1,
        color: PortalColor::Orange,
        anchor: feet + IVec3::new(12, 0, -12),
        face: FaceDirection::MinusZ,
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

    // The south face of the far pillar of sand, low, seen through the blue portal: its camera
    // sees it past the cactus, which lies behind the orange portal, and past its wall
    let feet = scene.feet.as_vec3();
    let far_sand = feet + Vec3::new(12.5, 0.3, -17.0);
    let through = on_screen(&mut h, far_sand - THROUGH);
    // The near pillar of sand, seen straight, at the same depth
    let near_sand = feet + Vec3::new(-6.0, 0.3, -12.0);
    let straight = on_screen(&mut h, near_sand);
    assert!((through.y - straight.y).abs() < 1.0);

    let spot = through;
    let (through, straight) = (mean_color(&shot, through), mean_color(&shot, straight));
    // Sand, not cactus nor cobblestone
    assert!(
        through.x > through.z + 25.0,
        "not sand through the portal: {through}"
    );
    // As bright and as warm as when seen straight: the view through the portal is tonemapped
    // once, as the rest of the screen
    assert!(
        (through - straight).abs().max_element() < 16.0,
        "sand through the portal {through}, seen straight {straight}"
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

/// The time a frame takes, the screen drawn into an image, with the blue portal in view, the
/// portals out of view, and the same view without portals, in turns over several rounds, so
/// that the noise of the machine falls on each alike. `BENCH_GPU=1` draws on the GPU instead of
/// lavapipe.
#[test]
#[ignore = "benchmark: run it in release with --ignored"]
fn bench_portal_frame_time() {
    let mut h = enter_world(GameFolder::new().with_world("Portals"), "Portals");
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
