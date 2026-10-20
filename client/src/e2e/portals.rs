//! The portals, in a world. The view through them is tested with portals and walls put into
//! the client's map, as décor the server knows nothing of; the portal gun places them with the
//! server.

use std::f32::consts::PI;
use std::time::{Duration, Instant};

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
use crate::portal::projection::MIN_OBLIQUE_DISTANCE;
use crate::portal::PortalCamera;
use crate::ui::hud::hotbar::Hotbar;
use crate::world::celestial::CelestialRoot;
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
fn local_player(w: &World) -> Option<Player> {
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
const FAR_MOD: &str = r#"
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
        .with_file(
            crate::constants::VIDEO_SETTINGS_PATH,
            &format!("(render_distance: {FAR_RENDER_DISTANCE})"),
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
    // The orange portal, 10 chunks away, faces the player too, beyond the fog
    assert!(active.contains(&PortalColor::Blue), "{active:?}");

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
    // The sun in the south, above the horizon, low enough to be seen through the opening: a day
    // lasts a minute
    let eyes_now = eyes(h.world_mut());
    h.wait_until("the sun low in the south", Duration::from_secs(90), |w| {
        let mut roots = w
            .try_query_filtered::<&Transform, With<CelestialRoot>>()
            .unwrap();
        roots.single(w).is_ok_and(|root| {
            let sun = root.rotation * Vec3::Z;
            let ray = toward(sun);
            let along = -blue.signed_distance(eyes_now) / ray.dot(blue.normal());
            let hit = blue.local(eyes_now + ray * along);
            sun.z > 0.0 && sun.y > 0.03 && hit.x.abs() < 0.7 && hit.y.abs() < 1.2
        })
    });
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
    let shot = h.screenshot();
    keep(
        &shot,
        "the_view_through_a_portal_reaches_a_far_place",
        "far_place_sun",
    );
    let eyes = eyes(h.world_mut());
    let direction = toward(sun(h.world_mut()));
    assert!(blue.signed_distance(eyes + direction * 10.0) < 0.0);
    let color = mean_color(&shot, on_screen(&mut h, eyes + direction * 100.0));
    assert!(
        color.x > color.z + 50.0 && color.y > color.z + 50.0,
        "not the sun through the portal: {color}"
    );
}
