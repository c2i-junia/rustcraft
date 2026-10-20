//! The portals, in a world. The view through them is tested with portals and walls put into
//! the client's map, as décor the server knows nothing of; the portal gun places them with the
//! server.

use std::time::Instant;

use bevy::{camera::RenderTarget, prelude::*};
use image::RgbImage;
use shared::world::{BlockData, BlockDirection, BlockId, FaceDirection, Portal, PortalColor};
use shared::CHUNK_SIZE;

use super::game::{enter_world, hold_click, slot};
use super::harness::{Harness, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::camera::CameraController;
use crate::player::CurrentPlayerMarker;
use crate::portal::PortalCamera;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::ClientWorldMap;
use shared::players::Player;
use shared::world::{raycast, ItemId};

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
/// portal facing the player, and further, to the east, a wall as high as the orange portal,
/// which faces north. Six blocks north of the orange portal stands a pillar of sand, 5 high;
/// between the portal and where its camera stands, south of its wall, a pillar of cactus, which
/// the camera must not see. A second pillar of sand stands to the left of the blue portal, seen
/// straight at the same depth.
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

    let spot = through;
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
    fn command(&mut self, command: &str) {
        self.press(KeyCode::KeyT);
        self.type_text(command);
        self.press(KeyCode::Enter);
    }

    /// Turns the player's camera toward `point`, on a face of the block `cell`, and waits for it
    /// to aim at that block.
    #[track_caller]
    fn aim_at(&mut self, point: Vec3, cell: IVec3) {
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
            raycast(blocks, camera, &player.translation, Default::default())
                .is_some_and(|target| target.position == cell)
        });
    }
}

/// The portal of the local player of colour `color`, as the client knows it.
fn own_portal(w: &World, color: PortalColor) -> Option<Portal> {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    let id = players.single(w).ok()?.id;
    let portals = w.resource::<ClientWorldMap>().blocks.portals();
    portals.get(id, color).copied()
}

/// How many portal cameras there are: one per open portal.
fn portal_cameras(w: &mut World) -> usize {
    w.query::<&PortalCamera>().iter(w).count()
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
