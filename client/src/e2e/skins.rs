//! The Skin screen: the player chooses a skin among the cards, which a turning model wears, and
//! keeps it; a PNG dropped on the window is imported, or refused with the reason. The skin goes
//! with the player to the server, which hands it back with its spawn.

use std::io::Cursor;

use bevy::prelude::*;
use bevy::window::{FileDragAndDrop, PrimaryWindow};
use shared::players::skin::PlayerSkin;
use shared::players::Player;
use shared::GameFolderPaths;

use super::game::enter_world;
use super::harness::{Harness, SCREEN};
use super::helpers::main_menu;
use super::locator::Locator as L;
use super::seed::GameFolder;
use crate::player::skin::{load_skin_choice, ChosenSkin, SkinChoice};
use crate::player::CurrentPlayerMarker;
use crate::ui::menus::skins::{SkinName, SkinPreview, SkinPreviewModel, IMPORT_NOTE, SKINS_TITLE};
use crate::ui::menus::MenuState;
use crate::ui::theme::SLOT;

fn chosen(h: &Harness) -> SkinChoice {
    h.world().resource::<ChosenSkin>().choice.clone()
}

fn paths(h: &Harness) -> GameFolderPaths {
    h.world().resource::<GameFolderPaths>().clone()
}

/// How many pixels of the preview are not its empty background: the model it draws.
fn model_pixels(h: &mut Harness) -> usize {
    let rect = h.rect_of(&L::marker::<SkinPreview>());
    let shot = h.screenshot();
    let ground = Vec3::from_array(SLOT.to_srgba().to_u8_array_no_alpha().map(f32::from));
    let mut count = 0;
    for y in rect.min.y as u32..rect.max.y as u32 {
        for x in rect.min.x as u32..rect.max.x as u32 {
            let pixel = Vec3::from_array(shot.get_pixel(x, y).0.map(f32::from));
            if (pixel - ground).length() > 12.0 {
                count += 1;
            }
        }
    }
    count
}

fn preview_turn(w: &World) -> f32 {
    let mut models = w
        .try_query_filtered::<&Transform, With<SkinPreviewModel>>()
        .unwrap();
    let transform = models.single(w).unwrap();
    transform.rotation.to_euler(EulerRot::YXZ).0
}

/// Drops the file at `path` on the window.
fn drop_file(h: &mut Harness, path: std::path::PathBuf) {
    let w = h.world_mut();
    let window = w
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(w)
        .unwrap();
    w.write_message(FileDragAndDrop::DroppedFile {
        window,
        path_buf: path,
    });
    h.frames(2);
}

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x * 4) as u8, (y * 4) as u8, 200, 255])
    });
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_skin_chosen_on_the_skin_screen_is_kept_and_turns_in_the_preview() {
    let mut h = main_menu();
    h.click(&L::text(SKINS_TITLE));
    h.wait_for(&L::text(IMPORT_NOTE), SCREEN);
    assert_eq!(h.menu_state(), MenuState::Skins);
    // the skin of a player who has not chosen one is selected
    assert_eq!(h.focus_label().as_deref(), Some("Wanderer"));
    assert_eq!(h.text_of(&L::marker::<SkinName>()), "Wanderer");

    // The preview draws the model, which turns
    h.wait_for_ui_images();
    let turn = preview_turn(h.world());
    assert!(model_pixels(&mut h) > 2000, "no model in the preview");
    h.frames(10);
    assert_ne!(preview_turn(h.world()), turn, "the model does not turn");

    // A click on a card chooses its skin, saved at once
    h.click(&L::text("Miner"));
    h.frames(2);
    let miner = SkinChoice::BuiltIn("Miner".into());
    assert_eq!(chosen(&h), miner);
    assert_eq!(
        h.world().resource::<ChosenSkin>().skin,
        PlayerSkin::BuiltIn("Miner".into())
    );
    assert_eq!(h.text_of(&L::marker::<SkinName>()), "Miner");
    assert_eq!(load_skin_choice(&paths(&h)), miner);
    h.snapshot("miner");

    // The arrows choose the next one
    h.press(KeyCode::ArrowDown);
    h.frames(2);
    assert_eq!(chosen(&h), SkinChoice::BuiltIn("Gardener".into()));

    // Back on the main menu and again on the screen, the skin is still chosen
    h.press(KeyCode::Escape);
    h.wait_for(&L::text("Singleplayer"), SCREEN);
    h.click(&L::text(SKINS_TITLE));
    h.wait_for(&L::text(IMPORT_NOTE), SCREEN);
    assert_eq!(h.focus_label().as_deref(), Some("Gardener"));
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn a_png_dropped_on_the_skin_screen_is_imported_or_refused() {
    let mut h = main_menu();
    h.click(&L::text(SKINS_TITLE));
    h.wait_for(&L::text(IMPORT_NOTE), SCREEN);
    let source = tempfile::tempdir().unwrap();

    // An image of another size is refused, with the reason
    let small = source.path().join("Tiny.png");
    std::fs::write(&small, png(32, 32)).unwrap();
    drop_file(&mut h, small);
    h.wait_for(&L::text_containing("not 32x32"), SCREEN);
    assert_eq!(chosen(&h), SkinChoice::default());

    // A skin is copied to the skins folder, listed, and chosen
    let mine = source.path().join("Mine.png");
    std::fs::write(&mine, png(64, 64)).unwrap();
    drop_file(&mut h, mine);
    h.wait_for(&L::text("Mine"), SCREEN);
    assert_eq!(chosen(&h), SkinChoice::Custom("Mine.png".into()));
    assert!(!h.is_visible(&L::text_containing("not 32x32")));
    let folder = paths(&h).game_folder_path.join("skins");
    assert!(folder.join("Mine.png").is_file());
    assert_eq!(h.focus_label().as_deref(), Some("Mine"));
    assert!(matches!(
        h.world().resource::<ChosenSkin>().skin,
        PlayerSkin::Custom(ref pixels) if pixels.len() == 64 * 64 * 4
    ));

    // An old skin of 64x32 is taken too
    let old = source.path().join("Old.png");
    std::fs::write(&old, png(64, 32)).unwrap();
    drop_file(&mut h, old);
    h.wait_for(&L::text("Old"), SCREEN);
    assert_eq!(chosen(&h), SkinChoice::Custom("Old.png".into()));
    h.snapshot("imported");
}

#[test]
#[cfg_attr(not(target_os = "linux"), ignore = "needs lavapipe")]
fn the_chosen_skin_comes_back_from_the_server_with_the_player() {
    let folder = GameFolder::new()
        .with_world("Skins")
        .with_file("skin.ron", "(skin: BuiltIn(\"Sailor\"))");
    let mut h = enter_world(folder, "Skins");
    h.wait_until("the local player", SCREEN, |w| {
        w.try_query_filtered::<&Player, With<CurrentPlayerMarker>>()
            .unwrap()
            .single(w)
            .is_ok_and(|player| player.skin == PlayerSkin::BuiltIn("Sailor".into()))
    });
}
