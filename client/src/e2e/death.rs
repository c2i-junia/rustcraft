//! Death in survival, through the integrated server: what the death screen says for each cause
//! that is cheap to stage (the fall, the lava and the Gnawer's bites are in `survival` and
//! `mobs`), and the inventory left where the player died.

use bevy::prelude::*;
use shared::players::air::MAX_AIR;
use shared::players::hunger::MAX_FOOD;
use shared::players::{Player, PLAYER_HEIGHT};
use shared::world::{ItemId, ItemStack};

use super::game::{enter_game, enter_world, hold_click, slot};
use super::harness::{Harness, Options, SERVER};
use super::locator::Locator as L;
use super::seed::GameFolder;
use super::survival::lake_bottom;
use crate::camera::CameraController;
use crate::dropped_items::DroppedItemRoot;
use crate::player::CurrentPlayerMarker;
use crate::ui::hud::hotbar::Hotbar;
use crate::ui::menus::death::{DEATH_TITLE, RESPAWN};
use server::WorldGenerator;

/// The local player, as the client predicts it.
fn player(w: &World) -> Player {
    let mut players = w
        .try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
        .unwrap();
    players.single(w).unwrap().clone()
}

/// Where the items lying in the world are, as the client shows them.
fn lying(w: &World) -> Vec<Vec3> {
    let Some(mut items) = w.try_query::<(&DroppedItemRoot, &Transform)>() else {
        return Vec::new();
    };
    items.iter(w).map(|(_, at)| at.translation).collect()
}

fn stack(item_id: ItemId, nb: u32) -> ItemStack {
    ItemStack {
        item_id,
        item_type: item_id.get_default_type(),
        nb,
    }
}

impl Harness {
    /// Waits for the death screen, and checks that it says `message`.
    #[track_caller]
    fn wait_for_death(&mut self, message: &str) {
        self.wait_for(&L::text(crate::i18n::tr(DEATH_TITLE)), SERVER);
        assert!(
            self.is_visible(&L::text(message)),
            "the death screen does not say {message:?}: {:?}",
            player(self.world()).death_cause
        );
    }
}

/// Where a player stands on the ground 12 blocks east of the spawn of the world of the seed 42.
fn away_from_the_spawn() -> Vec3 {
    let generator = WorldGenerator::new(42);
    let spawn = generator.spawn_position();
    let (x, z) = (spawn.x.floor() as i32 + 12, spawn.z.floor() as i32);
    let ground = generator.terrain().column_shape(x, z).height;
    Vec3::new(
        x as f32 + 0.5,
        ground as f32 + 1.0 + PLAYER_HEIGHT / 2.0 + 0.01,
        z as f32 + 0.5,
    )
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_starving_player_dies_leaves_its_inventory_where_it_died_and_respawns_fed() {
    let died_at = away_from_the_spawn();
    let folder =
        GameFolder::new()
            .with_world("Starved")
            .with_player("Starved", "Tester", |saved| {
                saved.position = died_at;
                saved.health = 1;
                saved.food = 0;
                saved.saturation = 0.0;
                let inventory = &mut saved.inventory.inner;
                inventory.insert(0, stack(ItemId::Dirt, 10));
                inventory.insert(20, stack(ItemId::Stick, 3));
            });
    // It may die before its hotbar shows
    let mut h = enter_game(folder, "Starved", Options::default());
    h.wait_for_death("You starved to death");

    // Its two stacks fall around where it died, and its inventory is empty
    h.wait_until("the two stacks lying", SERVER, |w| lying(w).len() == 2);
    h.wait_until("the emptied inventory", SERVER, |w| {
        slot(w, 0).is_none() && slot(w, 20).is_none()
    });
    let near = |at: Vec3| at.xz().distance(died_at.xz()) < 3.0;
    assert!(
        lying(h.world()).into_iter().all(near),
        "{:?}",
        lying(h.world())
    );

    // Back at the spawn, fed and breathing; its items still lie where it died
    h.click(&L::text(crate::i18n::tr(RESPAWN)));
    h.wait_until("the respawn", SERVER, |w| !player(w).is_dead());
    let back = player(h.world());
    assert!(
        back.position.xz().distance(died_at.xz()) > 8.0,
        "{}",
        back.position
    );
    assert_eq!(back.food, MAX_FOOD);
    assert_eq!(back.air, MAX_AIR);
    h.frames(20);
    let items = lying(h.world());
    assert_eq!(items.len(), 2);
    assert!(items.into_iter().all(near));
    assert_eq!(slot(h.world(), 0), None);
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_player_out_of_air_at_the_bottom_of_a_lake_drowns() {
    let folder =
        GameFolder::new()
            .with_world("Drowned")
            .with_player("Drowned", "Tester", |saved| {
                saved.position = lake_bottom();
                saved.health = 1;
            });
    let mut h = enter_world(folder, "Drowned");
    // 15 s of air, then the first hit
    h.wait_until("the air going", SERVER, |w| player(w).air < MAX_AIR);
    h.wait_for_death("You ran out of air");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_player_in_the_fire_it_lit_at_its_feet_goes_up_in_flames() {
    let folder = GameFolder::new()
        .with_world("Burnt")
        .with_player("Burnt", "Tester", |saved| {
            saved.health = 1;
            saved
                .inventory
                .inner
                .insert(0, stack(ItemId::FlintAndSteel, 1));
        });
    let mut h = enter_world(folder, "Burnt");
    h.wait_until("the saved inventory", SERVER, |w| {
        slot(w, 0) == Some((ItemId::FlintAndSteel, 1))
    });
    h.wait_until("the player on the ground", SERVER, |w| player(w).on_ground);
    look_straight_down(&mut h);
    h.frames(10);
    hold_click(&mut h, &L::marker::<Hotbar>(), MouseButton::Right, 1);
    h.wait_for_death("You went up in flames");
}

/// Turns the camera to the ground under the feet.
fn look_straight_down(h: &mut Harness) {
    let world = h.world_mut();
    world
        .query::<&mut CameraController>()
        .single_mut(world)
        .unwrap()
        .angle_y = 1.5;
}
