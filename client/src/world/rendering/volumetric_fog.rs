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
//!
//! Bevy draws every fog volume in every view; RenderLayers do not reach its volumetric pass.
//! A volume can now belong to one camera (`one_camera_offset`): it is drawn only by the view
//! whose camera stands at its centre, so that each camera gets its own layer of haze around it
//! and no other's (`world/rendering/shaders.rs`, `follow_cameras_with_haze`). The tag is in its
//! `density_texture_offset`, which only a density texture reads, and a haze has none.
//! The shader is changed where Bevy keeps it, among its embedded assets, once the plugins are
//! built and before it is loaded, so every camera draws with the same shader and nothing of
//! Bevy's is copied. If Bevy's text no longer matches, the game logs an error at startup and a
//! unit test fails.

use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::pbr::VolumetricFogPlugin;
use bevy::prelude::*;

/// Where Bevy keeps the shader of its volumetric fog, among its embedded assets.
const SHADER_PATH: &str = "bevy_pbr/volumetric_fog/volumetric_fog.wgsl";

/// A line of Bevy's shader that is wrong under an oblique near plane, what replaces it, and the
/// declaration the replacement reads, which must come before it: a Bevy that renamed it would
/// leave a shader that does not compile, and no rays of light on any camera.
struct Change {
    old: &'static str,
    new: &'static str,
    reads: &'static str,
}

/// The `density_texture_offset.y` of a fog volume that belongs to one camera
/// (`one_camera_offset`).
const ONE_CAMERA_TAG: f32 = -4096.0;

/// The `density_texture_offset` of a fog volume drawn only by the view whose camera stands
/// within half a block of its centre along x and z, and marched only between the heights
/// `bottom` and `top`, whatever its own: a volume stretched to hold its camera, which Bevy
/// then draws from inside, hazes no more than the layer.
pub fn one_camera_offset(bottom: f32, top: f32) -> Vec3 {
    Vec3::new(bottom, ONE_CAMERA_TAG, top)
}

/// What draws a volume that belongs to one camera only in that camera's view: the camera, in
/// the volume's own space (0 to 1), within half a block of its centre along x and z, the size
/// of the volume read from its matrix (axis-aligned: its diagonal is one over its size).
const ONE_CAMERA_FILTER: &str = "let jitter_strength = volumetric_fog.jitter_strength;

    // A volume that belongs to one camera is drawn only in its view (RustCraft)
    if (density_texture_offset.y == -4096.0) {
        let camera_uvw = (uvw_from_world * vec4(view.world_position, 1.0)).xyz;
        let half_block = 0.5 * vec2(uvw_from_world[0].x, uvw_from_world[2].z);
        if (any(abs(camera_uvw.xz - vec2(0.5)) > half_block)) {
            return vec4(0.0, 0.0, 0.0, 0.0);
        }
    }";

/// The start depth of the ray from its start point (`CHANGES`), then, for a volume of one
/// camera, the ray cut to the heights its offset gives (`one_camera_offset`): it starts where
/// it enters them, its start point with it, and ends where it leaves them. Along the ray,
/// the height grows by `rise` per unit of view depth; the camera is the origin of the view.
const START_IN_THE_LAYER: &str = "var start_depth_view = -view_start_pos.z;

    // A volume of one camera is marched only between the heights of its offset (RustCraft)
    if (density_texture_offset.y == -4096.0) {
        let dir_view = normalize(view_start_pos.xyz);
        let dir_world = (view.world_from_view * vec4(dir_view, 0.0)).xyz;
        let rise = dir_world.y / -dir_view.z;
        let camera_y = view.world_position.y;
        var near = start_depth_view;
        var far = end_depth_view;
        if (abs(rise) > 1e-6) {
            let a = (density_texture_offset.x - camera_y) / rise;
            let b = (density_texture_offset.z - camera_y) / rise;
            near = max(near, min(a, b));
            far = min(far, max(a, b));
        } else if (camera_y < density_texture_offset.x || camera_y > density_texture_offset.z) {
            far = near;
        }
        if (near > start_depth_view) {
            view_start_pos = dir_view * (near / -dir_view.z);
            start_depth_view = near;
        }
        end_depth_view = far;
    }";

const CHANGES: [Change; 4] = [
    // A volume of one camera, only in its view
    Change {
        old: "let jitter_strength = volumetric_fog.jitter_strength;",
        new: ONE_CAMERA_FILTER,
        reads: "let density_texture_offset =",
    },
    // The start depth of the ray, with the near distance only, then from its start point, on
    // the near plane or the front face of the volume
    Change {
        old: "let start_depth_view = -depth_ndc_to_view_z(frag_coord.z);",
        new: START_IN_THE_LAYER,
        reads: "view_start_pos = position_ndc_to_view(",
    },
    // The ray ended where the depth buffer says, the far plane included, then only where
    // something was drawn
    Change {
        old: "end_depth_view = min(end_depth_view, view_end_depth_from_buffer);",
        new: "if (ndc_end_depth_from_buffer > 0.0) {\n        \
              end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    }",
        reads: "let ndc_end_depth_from_buffer =",
    },
    // The start point, which the ray cut to the heights of its volume moves
    Change {
        old: "let view_start_pos = position_ndc_to_view(",
        new: "var view_start_pos = position_ndc_to_view(",
        reads: "let ndc_end_depth_from_buffer =",
    },
];

/// `source`, Bevy's shader of the volumetric fog, with its rays right under an oblique near
/// plane, or what it lacks: each line to change, once, after the declaration it then reads.
fn for_oblique_near_planes(source: &str) -> Result<String, String> {
    CHANGES
        .iter()
        .try_fold(source.to_string(), |source, change| {
            let found: Vec<usize> = source.match_indices(change.old).map(|(at, _)| at).collect();
            let [at] = found[..] else {
                return Err(format!("`{}` {} times, not once", change.old, found.len()));
            };
            if !source[..at].contains(change.reads) {
                return Err(format!("`{}` before `{}`", change.reads, change.old));
            }
            Ok(source.replace(change.old, change.new))
        })
}

/// Changes Bevy's shader of the volumetric fog among its embedded assets, once every plugin is
/// built (`finish`), whatever their order, and before the shader is loaded, at the first update.
/// If Bevy's volumetric fog is there but its shader cannot be changed, it says so at startup and
/// leaves it as it is: the player's rays of light are right, those through a portal seen from
/// more than `MIN_OBLIQUE_DISTANCE` are not. `the_shader_of_bevy_is_changed_for_oblique_near_planes`
/// fails then too.
pub struct VolumetricFogPatchPlugin;

impl Plugin for VolumetricFogPatchPlugin {
    fn build(&self, _app: &mut App) {}

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<VolumetricFogPlugin>() {
            // Without Bevy's volumetric fog, there is nothing to change
            return;
        }
        if let Err(missing) = patch(app) {
            error!(
                "Bevy's {SHADER_PATH} could not be changed for the oblique near planes of the \
                 portal cameras, whose rays of light will be wrong: {missing}"
            );
        }
    }
}

fn patch(app: &App) -> Result<(), String> {
    let registry = app
        .world()
        .get_resource::<EmbeddedAssetRegistry>()
        .ok_or("no embedded assets")?;
    let path = Path::new(SHADER_PATH);
    let original = registry
        .remove_asset(path)
        .ok_or("no such embedded asset")?;
    let changed = std::str::from_utf8(original.value())
        .map_err(|error| error.to_string())
        .and_then(for_oblique_near_planes);
    let bytes = match &changed {
        Ok(changed) => changed.as_bytes().to_vec(),
        Err(_) => original.value().to_vec(),
    };
    registry.insert_asset(PathBuf::from(SHADER_PATH), path, bytes);
    changed.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::asset::AssetPlugin;

    const SAMPLE: &str = "    let density_texture_offset = volumetric_fog.density_texture_offset;\n    \
                          let jitter_strength = volumetric_fog.jitter_strength;\n    \
                          let ndc_end_depth_from_buffer = textureLoad(depth_texture, xy, 0);\n    \
                          let view_start_pos = position_ndc_to_view(frag_coord_to_ndc(frag_coord));\n    \
                          var end_depth_view = 0.0;\n    \
                          end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    \
                          let start_depth_view = -depth_ndc_to_view_z(frag_coord.z);\n";

    #[test]
    fn a_ray_starts_at_its_start_point_and_ends_at_the_volume_where_nothing_was_drawn() {
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        assert!(changed.contains("var start_depth_view = -view_start_pos.z;"));
        assert!(!changed.contains("depth_ndc_to_view_z(frag_coord.z)"));
        assert!(changed.contains(
            "if (ndc_end_depth_from_buffer > 0.0) {\n        \
             end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    }"
        ));
    }

    #[test]
    fn a_volume_of_one_camera_is_drawn_only_in_the_view_at_its_centre() {
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        let filter = changed.find("density_texture_offset.y == -4096.0").unwrap();
        // After the offset is read, before the ray
        assert!(changed.find("let density_texture_offset =").unwrap() < filter);
        assert!(filter < changed.find("let ndc_end_depth_from_buffer").unwrap());
        assert!(ONE_CAMERA_FILTER.contains(&format!("{ONE_CAMERA_TAG:.1}")));
        // Half a block, whatever the size of the volume: its scale is read from its matrix
        assert!(ONE_CAMERA_FILTER.contains("uvw_from_world[0].x"));
        let offset = one_camera_offset(22.0, 162.0);
        assert_eq!(offset, Vec3::new(22.0, ONE_CAMERA_TAG, 162.0));
    }

    /// A volume of one camera is marched only between the heights its offset gives, whatever
    /// its size: a portal camera's layer of haze is stretched to hold the camera, which Bevy then
    /// draws from inside, its rays starting on the oblique near plane rather than clipped by it,
    /// and the stretch is not hazed.
    #[test]
    fn a_volume_of_one_camera_is_marched_only_within_its_heights() {
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        // The start of the ray, and the start point, may move to where the ray enters the slab
        assert!(changed.contains("var view_start_pos = position_ndc_to_view("));
        assert!(changed.contains("var start_depth_view = -view_start_pos.z;"));
        let clamp = changed.find("density_texture_offset.x - camera_y").unwrap();
        assert!(changed.find("var start_depth_view").unwrap() < clamp);
        assert!(clamp < changed.find("view_start_pos = dir_view").unwrap());
        assert!(changed.contains("end_depth_view = far;"));
    }

    #[test]
    fn a_shader_without_the_lines_is_not_changed() {
        assert!(for_oblique_near_planes("nothing to change").is_err());
        // Each line once: a shader changed already is not changed again
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        assert!(for_oblique_near_planes(&changed).is_err());
    }

    #[test]
    fn a_shader_whose_names_the_changes_read_were_renamed_is_not_changed() {
        let renamed = SAMPLE.replace("let view_start_pos", "let ray_start");
        let missing = for_oblique_near_planes(&renamed).unwrap_err();
        assert!(
            missing.contains("view_start_pos = position_ndc_to_view("),
            "{missing}"
        );
        let renamed = SAMPLE.replace("let ndc_end_depth_from_buffer", "let end_ndc");
        assert!(for_oblique_near_planes(&renamed).is_err());
    }

    /// The real shader of Bevy, changed whatever the order of the plugins, its names declared
    /// before the lines that read them.
    #[test]
    fn the_shader_of_bevy_is_changed_for_oblique_near_planes() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .add_plugins(VolumetricFogPatchPlugin)
            .add_plugins(VolumetricFogPlugin);
        app.finish();
        let registry = app.world().resource::<EmbeddedAssetRegistry>();
        let shader = registry
            .remove_asset(Path::new(SHADER_PATH))
            .expect("Bevy's shader of the volumetric fog");
        let source = std::str::from_utf8(shader.value()).unwrap();
        for change in CHANGES {
            let at = source
                .find(change.new)
                .unwrap_or_else(|| panic!("not changed: {}", change.old));
            assert!(
                source[..at].contains(change.reads),
                "`{}` no longer declared before `{}`",
                change.reads,
                change.new
            );
        }
        assert!(
            !source.contains(CHANGES[1].old),
            "the start depth not changed"
        );
    }
}
