//! The shaders of the game are embedded in the binary and loaded from the `embedded://` asset
//! source, so that a release needs no file next to it while a build with the `dev` feature
//! (Bevy's `embedded_watcher`) reloads a shader when its file is saved.

use bevy::asset::io::file::FileAssetReader;
use bevy::asset::AssetPath;
use bevy::shader::ShaderRef;
use std::path::{Path, PathBuf};

/// Embeds the shader at `$path`, relative to the calling file, in the `embedded://` source, as
/// `embedded_asset!` does, but under the name Bevy's `embedded_watcher` looks it up by.
///
/// `embedded_asset!` names the file for the watcher by `file!()`, which is relative to the root
/// of the workspace (`client/src/...`), while the watcher names a saved file relative to Bevy's
/// base path, the folder of the client under `cargo run` (`src/...`): the two never meet and no
/// shader would reload.
macro_rules! embed_shader {
    ($app: expr, $path: expr) => {{
        $app.world_mut()
            .resource_mut::<bevy::asset::io::embedded::EmbeddedAssetRegistry>()
            .insert_asset(
                $crate::shader_assets::watched_path(file!(), $path),
                &bevy::asset::embedded_path!($path),
                include_bytes!($path),
            );
    }};
}
pub(crate) use embed_shader;

/// Where the `embedded_watcher` finds the shader at `path` next to `source_file` (the `file!()`
/// of the file that embeds it): relative to Bevy's base path when the shader is under it.
pub fn watched_path(source_file: &str, path: &str) -> PathBuf {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the client is a member of the workspace");
    let file = workspace
        .join(source_file)
        .parent()
        .expect("a source file is in a folder")
        .join(path);
    match file.strip_prefix(FileAssetReader::get_base_path()) {
        Ok(relative) => relative.to_path_buf(),
        // The game runs away from its sources: there is nothing to watch
        Err(_) => file,
    }
}

/// The shader of a material at `path` in the `embedded://` source, as `embedded_path!` names it
/// in the file that embedded it.
pub fn embedded_shader(path: PathBuf) -> ShaderRef {
    ShaderRef::Path(AssetPath::from_path_buf(path).with_source("embedded"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Under `cargo test`, as under `cargo run`, the base path is the folder of the client: the
    /// watcher names a shader from there.
    #[test]
    fn a_shader_is_watched_relative_to_the_folder_of_the_client() {
        assert_eq!(
            watched_path("client/src/world/rendering/materials.rs", "sky.wgsl"),
            Path::new("src/world/rendering/sky.wgsl")
        );
    }
}
