//! The skin the player chose: one shipped with the game, or one it imported, a PNG copied to the
//! `skins/` folder of the game folder. The choice is saved in `skin.ron`; the skin itself is sent
//! to the server at each join (`AuthRegisterRequest::skin`), which hands it to the others.
//!
//! An imported skin is 64x64, or 64x32 as the skins of block games were before their left arm and
//! leg had pixels of their own: those are then the right ones, mirrored, as these games do.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use shared::players::skin::{PlayerSkin, BUILT_IN_SKINS, SKIN_BYTES, SKIN_SIDE};
use shared::GameFolderPaths;

use crate::constants::{CUSTOM_SKINS_PATH, SKIN_SETTINGS_PATH};

/// Where the skins shipped with the game are, among the assets.
const BUILT_IN_SKIN_FOLDER: &str = "graphics/skins";

/// The skin the player chose, as saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkinChoice {
    /// A skin shipped with the game, by its name in `BUILT_IN_SKINS`.
    BuiltIn(String),
    /// A skin the player imported, by its file name in the `skins/` folder.
    Custom(String),
}

impl Default for SkinChoice {
    fn default() -> Self {
        SkinChoice::BuiltIn(BUILT_IN_SKINS[0].to_string())
    }
}

impl SkinChoice {
    /// The name the skin screen shows: a built-in skin's, or the file name of an imported one
    /// without its extension.
    pub fn label(&self) -> &str {
        match self {
            SkinChoice::BuiltIn(name) => name,
            SkinChoice::Custom(file) => Path::new(file)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(file),
        }
    }
}

/// The skin the player chose, and the skin it sends to the servers it joins: its pixels, for an
/// imported one, read when it was chosen.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ChosenSkin {
    pub choice: SkinChoice,
    pub skin: PlayerSkin,
}

impl ChosenSkin {
    /// The saved choice and its skin. An imported skin that cannot be read any more, or a name
    /// the game does not know, gives the default skin.
    pub fn load(paths: &GameFolderPaths) -> Self {
        let saved = load_skin_choice(paths);
        match skin_of_choice(&saved, paths) {
            Some(skin) => ChosenSkin {
                choice: saved,
                skin,
            },
            None => ChosenSkin::default(),
        }
    }
}

/// The content of `skin.ron`.
#[derive(Serialize, Deserialize)]
struct SavedSkin {
    skin: SkinChoice,
}

/// The saved choice, or the default skin when none was saved or the file cannot be read.
pub fn load_skin_choice(paths: &GameFolderPaths) -> SkinChoice {
    let path = paths.game_folder_path.join(SKIN_SETTINGS_PATH);
    fs::read_to_string(path)
        .ok()
        .and_then(|content| ron::from_str::<SavedSkin>(&content).ok())
        .map(|saved| saved.skin)
        .unwrap_or_default()
}

/// Writes `choice` to `skin.ron` in the game folder.
pub fn write_skin_choice(choice: &SkinChoice, paths: &GameFolderPaths) -> io::Result<()> {
    let path = paths.game_folder_path.join(SKIN_SETTINGS_PATH);
    let saved = SavedSkin {
        skin: choice.clone(),
    };
    let serialized =
        ron::ser::to_string_pretty(&saved, PrettyConfig::new()).map_err(io::Error::other)?;
    fs::write(&path, serialized)?;
    info!("Skin saved to {path:?}");
    Ok(())
}

/// The skin `choice` names: `None` when it names no skin of the game, or an imported skin that
/// cannot be read.
pub fn skin_of_choice(choice: &SkinChoice, paths: &GameFolderPaths) -> Option<PlayerSkin> {
    match choice {
        SkinChoice::BuiltIn(name) => BUILT_IN_SKINS
            .contains(&name.as_str())
            .then(|| PlayerSkin::BuiltIn(name.clone())),
        SkinChoice::Custom(file) => {
            let bytes = fs::read(custom_skins_folder(paths).join(file)).ok()?;
            decode_skin(&bytes).ok().map(PlayerSkin::Custom)
        }
    }
}

/// Why a file cannot be a skin.
#[derive(Debug, PartialEq, Eq)]
pub enum SkinError {
    /// It is no PNG image.
    NotPng,
    /// A PNG of another size than 64x64 or 64x32.
    WrongSize(u32, u32),
    /// It cannot be read, or copied to the `skins/` folder.
    Io(String),
}

impl SkinError {
    /// What the skin screen says of it.
    pub fn message(&self) -> String {
        match self {
            SkinError::NotPng => "This file is not a PNG image.".to_string(),
            SkinError::WrongSize(width, height) => {
                format!("A skin is 64x64 pixels (or 64x32), not {width}x{height}.")
            }
            SkinError::Io(error) => format!("The skin could not be copied: {error}"),
        }
    }
}

/// The pixels of the skin in the PNG `bytes`, `SKIN_BYTES` of RGBA, row by row: a 64x32 skin is
/// brought to 64x64 (`widen_legacy_skin`).
pub fn decode_skin(bytes: &[u8]) -> Result<Vec<u8>, SkinError> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|_| SkinError::NotPng)?
        .to_rgba8();
    match image.dimensions() {
        (SKIN_SIDE, SKIN_SIDE) => Ok(image.into_raw()),
        (SKIN_SIDE, LEGACY_HEIGHT) => Ok(widen_legacy_skin(&image.into_raw())),
        (width, height) => Err(SkinError::WrongSize(width, height)),
    }
}

/// The height of a skin of the old layout, without the second layers of the body, the arms and
/// the legs, nor a left arm and leg of their own.
pub const LEGACY_HEIGHT: u32 = 32;

/// The faces of a box of `size` (w, h, d) whose faces start at `origin`, as (x, y, w, h): top,
/// bottom, right, front, left, back, as the skins lay them out.
fn faces(origin: (u32, u32), (w, h, d): (u32, u32, u32)) -> [(u32, u32, u32, u32); 6] {
    let (u, v) = origin;
    [
        (u + d, v, w, d),
        (u + d + w, v, w, d),
        (u, v + d, d, h),
        (u + d, v + d, w, h),
        (u + d + w, v + d, d, h),
        (u + 2 * d + w, v + d, w, h),
    ]
}

/// A 64x32 skin brought to 64x64: its rows on top, and the left arm and leg made of the right
/// ones mirrored, each face flipped and the right side swapped with the left, as block games do.
/// A hat without a single transparent pixel is a skin drawn without one: it is cleared, else it
/// would hide the head.
pub fn widen_legacy_skin(legacy: &[u8]) -> Vec<u8> {
    let row = (SKIN_SIDE * 4) as usize;
    let mut pixels = vec![0; SKIN_BYTES];
    pixels[..legacy.len().min(SKIN_BYTES)].copy_from_slice(&legacy[..legacy.len().min(SKIN_BYTES)]);
    let at = |x: u32, y: u32| y as usize * row + x as usize * 4;

    let hat = (32..64).flat_map(|x| (0..16).map(move |y| (x, y)));
    if hat.clone().all(|(x, y)| pixels[at(x, y) + 3] >= 128) {
        for (x, y) in hat {
            pixels[at(x, y)..at(x, y) + 4].fill(0);
        }
    }

    let limb = (4, 12, 4);
    // the right leg and arm, and where the left ones go
    for (from, to) in [((0, 16), (16, 48)), ((40, 16), (32, 48))] {
        let source = faces(from, limb);
        let target = faces(to, limb);
        // top, bottom, front and back to themselves; the right side becomes the left one
        for (face, mirror) in [(0, 0), (1, 1), (2, 4), (3, 3), (4, 2), (5, 5)] {
            let (sx, sy, w, h) = source[face];
            let (tx, ty, ..) = target[mirror];
            for y in 0..h {
                for x in 0..w {
                    let from = at(sx + x, sy + y);
                    let to = at(tx + w - 1 - x, ty + y);
                    let pixel: [u8; 4] = pixels[from..from + 4].try_into().unwrap();
                    pixels[to..to + 4].copy_from_slice(&pixel);
                }
            }
        }
    }
    pixels
}

/// The `skins/` folder of the game folder, where the imported skins are.
pub fn custom_skins_folder(paths: &GameFolderPaths) -> PathBuf {
    paths.game_folder_path.join(CUSTOM_SKINS_PATH)
}

/// The skins of `folder` that can be worn, by file name, with their pixels, sorted by name. The
/// files that are not skins are left out.
pub fn list_custom_skins(folder: &Path) -> Vec<(String, Vec<u8>)> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut skins: Vec<_> = entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
        })
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let pixels = decode_skin(&fs::read(entry.path()).ok()?).ok()?;
            Some((name, pixels))
        })
        .collect();
    skins.sort_by_key(|(name, _)| name.to_lowercase());
    skins
}

/// Copies the PNG at `source` to `folder`, under its own name, once it was found to be a skin;
/// returns that name and its pixels. A skin of the same name is replaced.
pub fn import_skin(source: &Path, folder: &Path) -> Result<(String, Vec<u8>), SkinError> {
    let bytes = fs::read(source).map_err(|error| SkinError::Io(error.to_string()))?;
    let pixels = decode_skin(&bytes)?;
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(SkinError::NotPng)?
        .to_string();
    fs::create_dir_all(folder).map_err(|error| SkinError::Io(error.to_string()))?;
    fs::write(folder.join(&name), &bytes).map_err(|error| SkinError::Io(error.to_string()))?;
    info!("Skin {name} imported to {folder:?}");
    Ok((name, pixels))
}

/// The image of a skin's pixels, `SKIN_BYTES` of RGBA, drawn without smoothing.
pub fn skin_image(pixels: &[u8]) -> Image {
    let mut image = Image::new(
        Extent3d {
            width: SKIN_SIDE,
            height: SKIN_SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels.to_vec(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );
    image.sampler = ImageSampler::nearest();
    image
}

/// The asset path of the built-in skin `name`, or of the default skin for a name the game does
/// not know.
pub fn built_in_skin_path(name: &str) -> String {
    let name = if BUILT_IN_SKINS.contains(&name) {
        name
    } else {
        BUILT_IN_SKINS[0]
    };
    format!("{BUILT_IN_SKIN_FOLDER}/{name}.png")
}

/// The texture of `skin`: the asset of a built-in one, an image made of the pixels of an
/// imported one.
pub fn skin_texture(
    skin: &PlayerSkin,
    assets: &AssetServer,
    images: &mut Assets<Image>,
) -> Handle<Image> {
    match skin {
        PlayerSkin::BuiltIn(name) => assets.load(built_in_skin_path(name)),
        PlayerSkin::Custom(pixels) if pixels.len() == SKIN_BYTES => images.add(skin_image(pixels)),
        PlayerSkin::Custom(_) => assets.load(built_in_skin_path(BUILT_IN_SKINS[0])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32, pixel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_fn(width, height, |x, y| image::Rgba(pixel(x, y)));
        let mut bytes = Vec::new();
        image
            .write_to(&mut io::Cursor::new(&mut bytes), image::ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn pixel(pixels: &[u8], x: u32, y: u32) -> [u8; 4] {
        let at = (y * SKIN_SIDE + x) as usize * 4;
        pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn a_64x64_png_is_a_skin_of_its_own_pixels() {
        let bytes = png(64, 64, |x, y| [x as u8, y as u8, 7, 255]);
        let pixels = decode_skin(&bytes).unwrap();
        assert_eq!(pixels.len(), SKIN_BYTES);
        assert_eq!(pixel(&pixels, 10, 50), [10, 50, 7, 255]);
    }

    #[test]
    fn a_png_of_another_size_or_no_png_is_refused() {
        assert_eq!(
            decode_skin(&png(32, 32, |_, _| [0; 4])),
            Err(SkinError::WrongSize(32, 32))
        );
        assert_eq!(
            decode_skin(&png(128, 128, |_, _| [0; 4])),
            Err(SkinError::WrongSize(128, 128))
        );
        assert_eq!(decode_skin(b"not an image"), Err(SkinError::NotPng));
        assert!(SkinError::WrongSize(32, 32).message().contains("64x64"));
    }

    #[test]
    fn a_64x32_skin_gets_its_left_limbs_from_the_right_ones_mirrored() {
        // each pixel says where it is
        let bytes = png(64, 32, |x, y| [x as u8, y as u8, 0, 255]);
        let pixels = decode_skin(&bytes).unwrap();
        assert_eq!(pixels.len(), SKIN_BYTES);
        // the top half is kept, but the opaque hat, cleared
        assert_eq!(pixel(&pixels, 9, 9), [9, 9, 0, 255]);
        assert_eq!(pixel(&pixels, 40, 8)[3], 0);
        // the front of the left leg is the front of the right one, flipped: as Minecraft copies
        // (4, 20) to (20, 52) mirrored
        assert_eq!(pixel(&pixels, 20, 52), [7, 20, 0, 255]);
        assert_eq!(pixel(&pixels, 23, 63), [4, 31, 0, 255]);
        // the right side of the right leg is the left side of the left one: (0, 20) to (24, 52)
        assert_eq!(pixel(&pixels, 24, 52), [3, 20, 0, 255]);
        // the left side of the right leg becomes the right side of the left one: (8, 20) to
        // (16, 52)
        assert_eq!(pixel(&pixels, 16, 52), [11, 20, 0, 255]);
        // the top of the left arm is the top of the right one: (44, 16) to (36, 48)
        assert_eq!(pixel(&pixels, 36, 48), [47, 16, 0, 255]);
        // the back of the left arm: (52, 20) to (44, 52)
        assert_eq!(pixel(&pixels, 44, 52), [55, 20, 0, 255]);
        // the second layers the old layout lacks stay transparent
        assert_eq!(pixel(&pixels, 20, 36)[3], 0);
        assert_eq!(pixel(&pixels, 4, 52)[3], 0);
    }

    #[test]
    fn a_hat_with_transparent_pixels_is_kept() {
        let bytes = png(64, 32, |x, y| {
            if (x, y) == (40, 8) {
                [0; 4]
            } else {
                [1, 2, 3, 255]
            }
        });
        let pixels = decode_skin(&bytes).unwrap();
        assert_eq!(pixel(&pixels, 41, 8), [1, 2, 3, 255]);
    }

    #[test]
    fn the_skin_choice_is_saved_and_loaded_back() {
        let dir = tempfile::tempdir().unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.path().to_path_buf(),
            assets_folder_path: dir.path().join("assets"),
        };
        assert_eq!(load_skin_choice(&paths), SkinChoice::default());
        for choice in [
            SkinChoice::BuiltIn("Sailor".into()),
            SkinChoice::Custom("mine.png".into()),
        ] {
            write_skin_choice(&choice, &paths).unwrap();
            assert_eq!(load_skin_choice(&paths), choice);
        }
        fs::write(paths.game_folder_path.join(SKIN_SETTINGS_PATH), "garbage").unwrap();
        assert_eq!(load_skin_choice(&paths), SkinChoice::default());
    }

    #[test]
    fn an_imported_skin_is_copied_listed_and_worn() {
        let dir = tempfile::tempdir().unwrap();
        let paths = GameFolderPaths {
            game_folder_path: dir.path().to_path_buf(),
            assets_folder_path: dir.path().join("assets"),
        };
        let folder = custom_skins_folder(&paths);
        let source = dir.path().join("Knight.png");
        fs::write(&source, png(64, 64, |_, _| [9, 9, 9, 255])).unwrap();
        let (name, pixels) = import_skin(&source, &folder).unwrap();
        assert_eq!(name, "Knight.png");
        // a file of another size is refused, and not copied
        let big = dir.path().join("Big.png");
        fs::write(&big, png(16, 16, |_, _| [0; 4])).unwrap();
        assert_eq!(
            import_skin(&big, &folder),
            Err(SkinError::WrongSize(16, 16))
        );
        fs::write(folder.join("notes.txt"), "hello").unwrap();
        fs::write(folder.join("broken.png"), "not a png").unwrap();

        let listed = list_custom_skins(&folder);
        assert_eq!(listed, vec![("Knight.png".to_string(), pixels.clone())]);

        let choice = SkinChoice::Custom(name);
        assert_eq!(choice.label(), "Knight");
        assert_eq!(
            skin_of_choice(&choice, &paths),
            Some(PlayerSkin::Custom(pixels))
        );
        write_skin_choice(&choice, &paths).unwrap();
        assert_eq!(ChosenSkin::load(&paths).choice, choice);
        // gone from the folder, it is the default skin again
        fs::remove_file(folder.join("Knight.png")).unwrap();
        assert_eq!(ChosenSkin::load(&paths), ChosenSkin::default());
    }

    #[test]
    fn an_unknown_built_in_skin_is_drawn_as_the_default_one() {
        assert_eq!(built_in_skin_path("Miner"), "graphics/skins/Miner.png");
        assert_eq!(built_in_skin_path("../x"), "graphics/skins/Wanderer.png");
    }
}
