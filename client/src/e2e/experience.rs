//! The experience: the bar over the hotbar shows the saved levels, and the orbs show; and the announcement of
//! the depth tiers.

use bevy::prelude::*;
use shared::messages::{DroppedItemState, DroppedItemsUpdate};
use shared::players::armour::ArmourPiece;
use shared::world::depths::DepthTier;
use shared::world::{DroppedItem, ItemId, ItemStack};

use super::game::enter_world;
use super::harness::{SCREEN, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::depths::DepthBanner;
use crate::ui::hud::experience::{experience_label, ExperienceBar};
use crate::ui::hud::health::HealthBar;
use shared::players::Player;

fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_bar_shows_the_saved_levels_under_the_hearts() {
    let folder = GameFolder::new()
        .with_world("Levels")
        .with_player("Levels", "Tester", |player| {
            player
                .experience
                .gain(8 + 11 + 14 + 17 + 20 + 23 + 26 + 29 + 32 + 35 + 38 + 41);
            player.experience.gain(27);
            let item_id = ItemId::IronChestplate;
            player.inventory.inner.insert(
                ArmourPiece::Chestplate.slot(),
                ItemStack {
                    item_id,
                    item_type: item_id.get_default_type(),
                    nb: 1,
                },
            );
        });
    let mut h = enter_world(folder, "Levels");
    h.wait_until("the saved levels", SERVER, |w| {
        player(w).experience.level == 12
    });
    h.set_time("day");
    h.wait_for(&L::marker::<ExperienceBar>(), SCREEN);
    let bar = h.get(&L::marker::<ExperienceBar>());
    h.frames(30);
    let label = h.world().get::<AccessibleLabel>(bar).unwrap();
    assert_eq!(label.0, experience_label(&player(h.world()).experience));
    let hearts = h.rect_of(&L::marker::<HealthBar>());
    assert!(hearts.max.y < h.rect_of(&L::marker::<ExperienceBar>()).min.y);

    let world = h.world_mut();
    let camera = *world
        .query_filtered::<&GlobalTransform, With<Camera3d>>()
        .iter(world)
        .next()
        .unwrap();
    let upserts = [1, 5, 40].map(|points| {
        let ahead = camera.forward() * 2.0 + camera.right() * (points as f32 / 30.0 - 0.5);
        let orb = DroppedItem::orb(points, camera.translation() + ahead, Vec3::ZERO);
        let state = DroppedItemState {
            stack: orb.stack,
            position: orb.position,
            velocity: orb.velocity,
            passes: 0,
            experience: points,
        };
        (1000 + u64::from(points), state)
    });
    world.write_message(DroppedItemsUpdate {
        upserts: upserts.to_vec(),
        removals: vec![],
    });
    h.frames(5);
    h.snapshot("experience");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_tier_is_announced_the_first_time_only() {
    // Buried: the player is pushed nowhere, and underground at once
    let reached = vec![DepthTier::Caves];
    let folder = GameFolder::new()
        .with_world("Deep")
        .with_player("Deep", "Tester", |player| {
            player.position.y = 16.0;
            player.depths_reached = reached;
        });
    let mut h = enter_world(folder, "Deep");
    h.wait_for(&L::marker::<DepthBanner>(), SERVER);
    assert!(h.is_visible(&L::text("The Depths")));
    h.frames(20);
    h.snapshot("depths");
    h.wait_until("the banner gone", SERVER, |w| {
        let mut banners = w.try_query::<&DepthBanner>().unwrap();
        banners.iter(w).count() == 0
    });
}
