//! Bevy's volumetric fog (the god-ray haze), made right for a camera whose near plane is oblique,
//! as a portal camera's is (`portal/projection.rs`), so that the portal cameras draw the haze
//! the player's camera draws and the view through a portal is hazed as the world is.
//!
//! Bevy 0.19's `volumetric_fog.wgsl` marches each ray from where it enters the fog volume, or
//! from the near plane when the camera is inside it, to the nearer of the back of the volume and
//! the opaque surface in the depth buffer. It finds every point with the whole inverse
//! projection, which is right under any projection, but for two things:
//!
//! - **The start depth of the ray**, read with the near distance of the projection only
//!   (`depth_ndc_to_view_z`). Under an oblique near plane the ray starts on that plane, whose
//!   depth changes from pixel to pixel; the near distance is a constant that has nothing to do
//!   with it. It is now the depth of the start point the shader already has.
//! - **Where nothing was drawn** (the sky), the depth buffer holds 0, the far plane. With Bevy's
//!   infinite projection that is infinitely far, and the ray ends at the back of the volume.
//!   Lengyel's oblique matrix tilts the far plane to a finite plane, behind the camera for most
//!   of the view: the ray ended before it began, and the sky through the portal had no haze.
//!   A depth of 0 now leaves the ray to the back of the volume.
//!
//! Under a plain projection both give what Bevy computed: the player's camera draws as before.
//! The shader is changed where Bevy keeps it, among its embedded assets, before it is loaded, so
//! every camera draws with the same shader and nothing of Bevy's is copied.

use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::prelude::*;

/// Where Bevy keeps the shader of its volumetric fog, among its embedded assets.
const SHADER_PATH: &str = "bevy_pbr/volumetric_fog/volumetric_fog.wgsl";

/// The lines of Bevy's shader that are wrong under an oblique near plane, and what replaces each.
const CHANGES: [(&str, &str); 2] = [
    // The start depth of the ray, with the near distance only, then from its start point, on
    // the near plane or the front face of the volume
    (
        "let start_depth_view = -depth_ndc_to_view_z(frag_coord.z);",
        "let start_depth_view = -view_start_pos.z;",
    ),
    // The ray ended where the depth buffer says, the far plane included, then only where
    // something was drawn
    (
        "end_depth_view = min(end_depth_view, view_end_depth_from_buffer);",
        "if (ndc_end_depth_from_buffer > 0.0) {\n        \
         end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    }",
    ),
];

/// `source`, Bevy's shader of the volumetric fog, with its rays right under an oblique near
/// plane, or `None` if it no longer has, once each, the lines to change.
fn for_oblique_near_planes(source: &str) -> Option<String> {
    CHANGES.iter().try_fold(source.to_string(), |source, (old, new)| {
        (source.matches(old).count() == 1).then(|| source.replace(old, new))
    })
}

/// Changes Bevy's shader of the volumetric fog among its embedded assets, before it is loaded.
/// Added after Bevy's `PbrPlugin`, which embeds it. A new Bevy whose shader lacks the lines
/// changed stops the game at once, as `the_shader_of_bevy_is_changed_for_oblique_near_planes`
/// does: the haze through the portals would be wrong.
pub fn volumetric_fog_plugin(app: &mut App) {
    let Some(registry) = app.world().get_resource::<EmbeddedAssetRegistry>() else {
        return;
    };
    let path = Path::new(SHADER_PATH);
    let Some(original) = registry.remove_asset(path) else {
        // Without Bevy's volumetric fog, there is nothing to change
        return;
    };
    let source = std::str::from_utf8(original.value()).expect("a shader is text");
    let changed = for_oblique_near_planes(source).unwrap_or_else(|| {
        panic!("Bevy's {SHADER_PATH} no longer has the lines volumetric_fog.rs changes")
    });
    registry.insert_asset(PathBuf::from(SHADER_PATH), path, changed.into_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;
    use bevy::pbr::VolumetricFogPlugin;

    const SAMPLE: &str = "    let view_start_pos = position_ndc_to_view(frag_coord_to_ndc(frag_coord));\n    \
                          end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    \
                          let start_depth_view = -depth_ndc_to_view_z(frag_coord.z);\n";

    #[test]
    fn a_ray_starts_at_its_start_point_and_ends_at_the_volume_where_nothing_was_drawn() {
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        assert!(changed.contains("let start_depth_view = -view_start_pos.z;"));
        assert!(!changed.contains("depth_ndc_to_view_z(frag_coord.z)"));
        assert!(changed.contains(
            "if (ndc_end_depth_from_buffer > 0.0) {\n        \
             end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    }"
        ));
    }

    #[test]
    fn a_shader_without_the_lines_is_not_changed() {
        assert_eq!(for_oblique_near_planes("nothing to change"), None);
        // Each line once: a shader changed already is not changed again
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        assert_eq!(for_oblique_near_planes(&changed), None);
    }

    #[test]
    fn the_shader_of_bevy_is_changed_for_oblique_near_planes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .add_plugins(VolumetricFogPlugin)
            .add_plugins(volumetric_fog_plugin);
        let registry = app.world().resource::<EmbeddedAssetRegistry>();
        let shader = registry
            .remove_asset(Path::new(SHADER_PATH))
            .expect("Bevy's shader of the volumetric fog");
        let source = std::str::from_utf8(shader.value()).unwrap();
        for (old, new) in CHANGES {
            assert!(source.contains(new), "not changed: {old}");
        }
        assert!(!source.contains(CHANGES[0].0), "the start depth not changed");
    }
}
