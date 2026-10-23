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
//! whose camera stands where the volume says, along x, y and z, and looks the way it says, so
//! that each camera gets its own layer of haze around it and no other's
//! (`world/rendering/shaders.rs`, `follow_cameras_with_haze`). Two cameras that stand in the
//! same place mostly look different ways: a portal maps a camera onto itself only where it turns
//! the world, and then keeps its look only if it looks straight up or down, when the two
//! cameras share a volume (`follow_cameras_with_haze`). The tag is in its
//! `density_texture_offset`, which only a density texture reads, and a haze has none.
//! The shader is changed where Bevy keeps it, among its embedded assets, once the plugins are
//! built and before it is loaded, so every camera draws with the same shader and nothing of
//! Bevy's is copied. If Bevy's text no longer matches, the game logs an error at startup and a
//! unit test fails, and the haze falls back to what it was before the portal cameras drew it
//! (`HazePerCamera`): the player's camera alone, with one volume, untagged.

use std::f32::consts::{FRAC_PI_2, PI};
use std::path::{Path, PathBuf};

use bevy::asset::io::embedded::EmbeddedAssetRegistry;
use bevy::pbr::VolumetricFogPlugin;
use bevy::prelude::*;

/// Where Bevy keeps the shader of its volumetric fog, among its embedded assets.
const SHADER_PATH: &str = "bevy_pbr/volumetric_fog/volumetric_fog.wgsl";

/// A line of Bevy's shader that is wrong under an oblique near plane, what replaces it, and the
/// lines the replacement depends on, which must come before it: a Bevy that renamed a
/// declaration it reads would leave a shader that does not compile, and no rays of light on any
/// camera; one that moved a line it must follow would overwrite what it sets.
struct Change {
    old: &'static str,
    new: &'static str,
    reads: &'static [&'static str],
}

/// The `density_texture_offset.y` of a fog volume that belongs to one camera
/// (`one_camera_offset`), and is marched only within the layer of haze, a portal camera's.
const ONE_CAMERA_TAG: f32 = -4096.0;
/// The same, for a volume marched as Bevy marches it, the player's camera's.
const ONE_CAMERA_TAG_UNCUT: f32 = -8192.0;

/// Whether each camera draws a layer of haze of its own, only it, the portal cameras too: the
/// shader is changed (`VolumetricFogPatchPlugin`). Otherwise Bevy's shader, as it is, draws
/// every volume in every view, and the portal cameras draw no haze, so that the player's view
/// has only its own.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct HazePerCamera(pub bool);

impl Default for HazePerCamera {
    fn default() -> Self {
        Self(true)
    }
}

/// How many steps the yaw and the pitch of a camera's look are each written in, in
/// `one_camera_offset`: both in one float, exactly.
const LOOK_STEPS: f32 = 2048.0;

/// The look `forward` of a camera, its yaw and its pitch each rounded to one of `LOOK_STEPS`
/// steps, in one float: a whole number under 2^22, which a float holds exactly. A step is
/// 0.18° of yaw and 0.09° of pitch, well within what tells two cameras apart in the shader
/// (`ONE_CAMERA_FILTER`, about a degree).
fn packed_look(forward: Vec3) -> f32 {
    let step = |value: f32, half_range: f32| {
        ((value / half_range + 1.0) / 2.0 * (LOOK_STEPS - 1.0))
            .round()
            .clamp(0.0, LOOK_STEPS - 1.0)
    };
    let yaw = forward.x.atan2(forward.z);
    let pitch = forward.y.clamp(-1.0, 1.0).asin();
    step(yaw, PI) * LOOK_STEPS + step(pitch, FRAC_PI_2)
}

/// The `density_texture_offset` of a fog volume drawn only by the view of the camera
/// `camera`: the view whose camera stands within half a block of the volume's centre along x
/// and z, within half a block of the camera's height, and looks within a degree of the
/// camera's look. A volume `cut`, a portal camera's, is marched only between the heights of the
/// layer of haze, whatever its own (`START_IN_THE_LAYER`): stretched to hold its camera, which
/// Bevy then draws from inside, it hazes no more than the layer. The player's, which is the
/// layer and nothing more, is marched as Bevy marches it.
pub fn one_camera_offset(camera: &GlobalTransform, cut: bool) -> Vec3 {
    let tag = if cut {
        ONE_CAMERA_TAG
    } else {
        ONE_CAMERA_TAG_UNCUT
    };
    Vec3::new(
        camera.translation().y,
        tag,
        packed_look(camera.forward().as_vec3()),
    )
}

/// The look `packed_look` wrote, as the shader reads it (`ONE_CAMERA_FILTER`).
fn unpacked_look(look: f32) -> Vec3 {
    let yaw_step = (look / LOOK_STEPS).floor();
    let yaw = (yaw_step / (LOOK_STEPS - 1.0) * 2.0 - 1.0) * PI;
    let pitch = ((look - yaw_step * LOOK_STEPS) / (LOOK_STEPS - 1.0) * 2.0 - 1.0) * FRAC_PI_2;
    Vec3::new(
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
}

/// Whether the view of the camera `view` draws a volume of one camera centred at `centre`
/// along x and z with the offset `offset`, as `ONE_CAMERA_FILTER` decides, its look a little
/// stricter (`SHARED_LOOK`): a camera that shares a volume is one the shader surely draws it in,
/// or its view would have no haze.
pub fn drawn_in(view: &GlobalTransform, centre: Vec2, offset: Vec3) -> bool {
    let at = view.translation();
    (at.xz() - centre).abs().max_element() <= 0.5
        && (at.y - offset.x).abs() <= 0.5
        && view.forward().dot(unpacked_look(offset.z)) >= SHARED_LOOK
}

/// The least cosine between two looks for `drawn_in`: the shader's (`ONE_CAMERA_LOOK`) made
/// stricter by far more than float noise moves it, so that the CPU never shares a volume the GPU
/// might leave out. Within that margin, about 0.03° wide, a camera could draw the volume and
/// its own; no motion through a pair brings a camera there but by chance, unlike the 90° turn
/// at the pitch clamp a looser tolerance (0.9997) caught (`a_camera_that_shares_a_volume_always_draws_it`).
const SHARED_LOOK: f32 = ONE_CAMERA_LOOK + 1e-5;

/// The least cosine between the look of a view and that of the camera a volume belongs to, for
/// the shader to draw the volume in that view (`ONE_CAMERA_FILTER`): about 1.15°.
const ONE_CAMERA_LOOK: f32 = 0.9998;

/// What draws a volume that belongs to one camera only in that camera's view: the camera, in
/// the volume's own space (0 to 1), within half a block of its centre along x and z, the size
/// of the volume read from its matrix (axis-aligned: its diagonal is one over its size); within
/// half a block of the height in the offset; and its look within about a degree of the look in
/// the offset (`packed_look`). A portal camera of a floor portal under a ceiling portal stands
/// straight above the player's camera; one of a pair that turns the world can stand where the
/// player's camera does, looking another way, or the same way if it looks straight up or down,
/// when the two share a volume (`drawn_in`).
const ONE_CAMERA_FILTER: &str = "let jitter_strength = volumetric_fog.jitter_strength;

    // A volume that belongs to one camera is drawn only in its view (RustCraft)
    if (density_texture_offset.y == -4096.0 || density_texture_offset.y == -8192.0) {
        let camera_uvw = (uvw_from_world * vec4(view.world_position, 1.0)).xyz;
        let half_block = 0.5 * vec2(uvw_from_world[0].x, uvw_from_world[2].z);
        let look = density_texture_offset.z;
        let yaw_step = floor(look / 2048.0);
        let yaw = (yaw_step / 2047.0 * 2.0 - 1.0) * 3.14159265;
        let pitch = ((look - yaw_step * 2048.0) / 2047.0 * 2.0 - 1.0) * 1.57079633;
        let owner_forward = vec3(cos(pitch) * sin(yaw), sin(pitch), cos(pitch) * cos(yaw));
        let view_forward = -normalize(view.world_from_view[2].xyz);
        if (any(abs(camera_uvw.xz - vec2(0.5)) > half_block)
            || abs(view.world_position.y - density_texture_offset.x) > 0.5
            || dot(view_forward, owner_forward) < 0.9998) {
            return vec4(0.0, 0.0, 0.0, 0.0);
        }
    }";

/// The start depth of the ray from its start point (`CHANGES`), then, for a volume of one
/// camera cut to the layer of haze (`one_camera_offset`, a portal camera's), the ray cut to the
/// heights of the layer, `HAZE_BOTTOM` and `HAZE_TOP` (`world/rendering/shaders.rs`): it
/// starts where it enters them, its start point with it, and ends where it leaves them. Along
/// the ray, the height grows by `rise` per unit of view depth; the camera is the origin of the
/// view. The player's volume is not cut: its rays are marched as before.
const START_IN_THE_LAYER: &str = "var start_depth_view = -view_start_pos.z;

    // A volume of one camera is marched only between the heights of the layer (RustCraft)
    if (density_texture_offset.y == -4096.0) {
        let bottom = 22.0;
        let top = 162.0;
        let dir_view = normalize(view_start_pos.xyz);
        let dir_world = (view.world_from_view * vec4(dir_view, 0.0)).xyz;
        let rise = dir_world.y / -dir_view.z;
        let camera_y = view.world_position.y;
        var near = start_depth_view;
        var far = end_depth_view;
        if (abs(rise) > 1e-6) {
            let a = (bottom - camera_y) / rise;
            let b = (top - camera_y) / rise;
            near = max(near, min(a, b));
            far = min(far, max(a, b));
        } else if (camera_y < bottom || camera_y > top) {
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
        reads: &["let density_texture_offset ="],
    },
    // The start depth of the ray, with the near distance only, then from its start point, on
    // the near plane or the front face of the volume
    Change {
        old: "let start_depth_view = -depth_ndc_to_view_z(frag_coord.z);",
        new: START_IN_THE_LAYER,
        // It sets where the ray ends: after Bevy ends it at the depth buffer
        reads: &[
            "view_start_pos = position_ndc_to_view(",
            "end_depth_view = min(end_depth_view, view_end_depth_from_buffer);",
        ],
    },
    // The ray ended where the depth buffer says, the far plane included, then only where
    // something was drawn
    Change {
        old: "end_depth_view = min(end_depth_view, view_end_depth_from_buffer);",
        new: "if (ndc_end_depth_from_buffer > 0.0) {\n        \
              end_depth_view = min(end_depth_view, view_end_depth_from_buffer);\n    }",
        reads: &["let ndc_end_depth_from_buffer ="],
    },
    // The start point, which the ray cut to the heights of its volume moves
    Change {
        old: "let view_start_pos = position_ndc_to_view(",
        new: "var view_start_pos = position_ndc_to_view(",
        reads: &[],
    },
];

/// `source`, Bevy's shader of the volumetric fog, with its rays right under an oblique near
/// plane, or what it lacks: each line to change, once, after the lines it depends on.
fn for_oblique_near_planes(source: &str) -> Result<String, String> {
    CHANGES
        .iter()
        .try_fold(source.to_string(), |source, change| {
            let found: Vec<usize> = source.match_indices(change.old).map(|(at, _)| at).collect();
            let [at] = found[..] else {
                return Err(format!("`{}` {} times, not once", change.old, found.len()));
            };
            if let Some(read) = change
                .reads
                .iter()
                .find(|read| !source[..at].contains(**read))
            {
                return Err(format!("`{read}` before `{}`", change.old));
            }
            Ok(source.replace(change.old, change.new))
        })
}

/// Changes Bevy's shader of the volumetric fog among its embedded assets, once every plugin is
/// built (`finish`), whatever their order, and before the shader is loaded, at the first update.
/// If Bevy's volumetric fog is there but its shader cannot be changed, it says so at startup,
/// leaves it as it is, and the haze falls back to the player's camera alone (`HazePerCamera`):
/// Bevy's shader would draw every camera's volume in every view, the player's included. The
/// views through the portals then have no rays of light.
/// `the_shader_of_bevy_is_changed_for_oblique_near_planes` fails then too.
pub struct VolumetricFogPatchPlugin;

impl Plugin for VolumetricFogPatchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HazePerCamera>();
    }

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<VolumetricFogPlugin>() {
            // Without Bevy's volumetric fog, there is nothing to change
            return;
        }
        if let Err(missing) = patch(app) {
            error!(
                "Bevy's {SHADER_PATH} could not be changed: the portal cameras draw no rays of \
                 light, and the player's camera only its own ({missing})"
            );
            app.insert_resource(HazePerCamera(false));
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
        for tag in [ONE_CAMERA_TAG, ONE_CAMERA_TAG_UNCUT] {
            assert!(ONE_CAMERA_FILTER.contains(&format!("density_texture_offset.y == {tag:.1}")));
        }
        // Half a block, whatever the size of the volume: its scale is read from its matrix
        assert!(ONE_CAMERA_FILTER.contains("uvw_from_world[0].x"));
        // The look as `packed_look` writes it
        assert!(ONE_CAMERA_FILTER.contains(&format!("look / {LOOK_STEPS:.1}")));
        assert!(ONE_CAMERA_FILTER.contains(&format!("/ {:.1} * 2.0 - 1.0", LOOK_STEPS - 1.0)));
        let camera = GlobalTransform::from(
            Transform::from_xyz(3.0, 70.0, -2.0).looking_to(Vec3::new(1.0, -0.5, 0.2), Vec3::Y),
        );
        let offset = one_camera_offset(&camera, true);
        assert_eq!(offset.x, 70.0);
        assert_eq!(offset.y, ONE_CAMERA_TAG);
        // The player's volume carries the other tag, which is not cut to the layer
        let offset = one_camera_offset(&camera, false);
        assert_eq!(offset.y, ONE_CAMERA_TAG_UNCUT);
        assert!(!START_IN_THE_LAYER.contains("-8192"));
    }

    /// The look of a camera, unpacked as `ONE_CAMERA_FILTER` unpacks it.

    #[test]
    fn the_look_of_a_camera_is_written_exactly_within_a_tenth_of_a_degree() {
        for yaw in (-180..=180).step_by(5) {
            for pitch in (-89..=89).step_by(4) {
                let (yaw, pitch) = ((yaw as f32).to_radians(), (pitch as f32).to_radians());
                let forward = Vec3::new(
                    pitch.cos() * yaw.sin(),
                    pitch.sin(),
                    pitch.cos() * yaw.cos(),
                );
                let look = packed_look(forward);
                // A whole number a float holds exactly
                assert_eq!(look, look.round());
                assert!(look < (1 << 22) as f32);
                let angle = forward.angle_between(unpacked_look(look)).to_degrees();
                assert!(angle < 0.1, "{forward} {angle}");
                // Well within what the filter allows
                assert!(forward.dot(unpacked_look(look)) > 0.9999);
            }
        }
    }

    /// Each camera draws its own volume and no other's: not one straight above or under it, as
    /// the portal cameras of a floor portal under a ceiling portal are, nor one in the same
    /// place looking another way, as through a pair that turns the world (unless it looks
    /// straight up or down, when the two share a volume).
    #[test]
    fn a_camera_draws_only_its_own_volume_whoever_stands_by_it() {
        let player = GlobalTransform::from(
            Transform::from_xyz(-40.0, 83.0, -2.5).looking_at(Vec3::new(-40.0, 79.0, 1.5), Vec3::Y),
        );
        let centre = player.translation().xz();
        for cut in [false, true] {
            let offset = one_camera_offset(&player, cut);
            assert!(drawn_in(&player, centre, offset));
            // Stacked: 11 blocks over it, the same look
            let above = GlobalTransform::from(
                Transform::from_translation(player.translation() + Vec3::Y * 11.0)
                    .with_rotation(player.rotation()),
            );
            assert!(!drawn_in(&above, centre, offset));
            assert!(!drawn_in(&player, centre, one_camera_offset(&above, cut)));
            // In the same place, turned about the vertical by a few degrees, or tipped over
            for turn in [
                Quat::from_rotation_y(3f32.to_radians()),
                Quat::from_rotation_y(90f32.to_radians()),
                Quat::from_rotation_x(90f32.to_radians()),
            ] {
                let turned = GlobalTransform::from(
                    Transform::from_translation(player.translation())
                        .with_rotation(turn * player.rotation()),
                );
                assert!(!drawn_in(&turned, centre, offset), "{turn}");
                assert!(!drawn_in(&player, centre, one_camera_offset(&turned, cut)));
            }
        }
    }

    /// Whether the shader draws, in the view of the camera `view`, the volume of one camera
    /// centred at `centre` along x and z with the offset `offset` (`ONE_CAMERA_FILTER`, its
    /// arithmetic in f32 as on the GPU).
    fn gpu_draws(view: &GlobalTransform, centre: Vec2, offset: Vec3) -> bool {
        let at = view.translation();
        (at.xz() - centre).abs().max_element() <= 0.5
            && (at.y - offset.x).abs() <= 0.5
            && view.forward().dot(unpacked_look(offset.z)) >= ONE_CAMERA_LOOK
    }

    /// A camera that shares a volume (`drawn_in`) is one the shader draws it in, or its view would
    /// have no haze. A pair on two walls at right angles turns the world by 90° about a vertical
    /// line: a player standing on it and looking down as far as it can (`MAX_PITCH_DEGREES`) has a
    /// portal camera where it stands, its look 1.38–1.45° off, the quantisation of the pitch
    /// included, on either side of the tolerance the CPU had at 0.9997: it shared the player's
    /// volume, which the shader left out of its view, and the portal showed no haze.
    #[test]
    fn a_camera_that_shares_a_volume_always_draws_it() {
        use shared::players::constants::MAX_PITCH_DEGREES;
        let at = Vec3::new(-40.0, 83.0, -2.5);
        let quarter = Quat::from_rotation_y(FRAC_PI_2);
        let mut cases = 0;
        for pitch in [
            MAX_PITCH_DEGREES,
            MAX_PITCH_DEGREES - 0.02,
            MAX_PITCH_DEGREES - 0.05,
        ] {
            for yaw in (0..1000).map(|step| step as f32 * 0.361) {
                let rotation = Quat::from_rotation_y(yaw.to_radians())
                    * Quat::from_rotation_x(-pitch.to_radians());
                let player =
                    GlobalTransform::from(Transform::from_translation(at).with_rotation(rotation));
                let portal = GlobalTransform::from(
                    Transform::from_translation(at).with_rotation(quarter * rotation),
                );
                let offset = one_camera_offset(&player, false);
                let shares = drawn_in(&portal, at.xz(), offset);
                let draws = gpu_draws(&portal, at.xz(), offset);
                assert!(!shares || draws, "{pitch}° {yaw}°: shares, not drawn");
                // There, it is not drawn: the portal camera has a volume of its own
                assert!(!draws, "{pitch}° {yaw}°");
                cases += 1;
            }
        }
        assert_eq!(cases, 3000);
        // Level, turned by a little more and a little less than the tolerance (1.15°)
        let level = GlobalTransform::from(
            Transform::from_translation(at).looking_to(Vec3::new(0.3, 0.0, 1.0), Vec3::Y),
        );
        let offset = one_camera_offset(&level, true);
        for turn in (0..400).map(|step| 1.0 + step as f32 * 0.001) {
            let turned = GlobalTransform::from(
                Transform::from_translation(at)
                    .with_rotation(Quat::from_rotation_y(turn.to_radians()) * level.rotation()),
            );
            let shares = drawn_in(&turned, at.xz(), offset);
            assert!(!shares || gpu_draws(&turned, at.xz(), offset), "{turn}°");
        }
    }

    /// The shader's tolerance on the look is `ONE_CAMERA_LOOK`.
    #[test]
    fn the_shader_tests_the_look_against_one_camera_look() {
        assert!(ONE_CAMERA_FILTER.contains(&format!(
            "dot(view_forward, owner_forward) < {ONE_CAMERA_LOOK})"
        )));
    }

    /// The rays are cut to the heights of the layer only in a portal camera's volume, by its
    /// tag: the player's view is marched as before lot 12, by construction. The heights are the
    /// layer's.
    #[test]
    fn only_a_portal_cameras_volume_is_cut_to_the_layer() {
        use crate::world::rendering::shaders::{HAZE_BOTTOM, HAZE_TOP};
        assert!(START_IN_THE_LAYER.contains("if (density_texture_offset.y == -4096.0) {"));
        assert!(START_IN_THE_LAYER.contains(&format!("let bottom = {HAZE_BOTTOM:.1};")));
        assert!(START_IN_THE_LAYER.contains(&format!("let top = {HAZE_TOP:.1};")));
    }

    /// The cut sets where the ray ends: it must come after Bevy ends it at the depth buffer,
    /// or Bevy would overwrite it and haze the whole stretch of a portal camera's volume.
    #[test]
    fn a_shader_that_ends_its_rays_after_it_starts_them_is_not_changed() {
        let start = "    let start_depth_view = -depth_ndc_to_view_z(frag_coord.z);\n";
        let moved = SAMPLE.replace(start, "");
        let moved = moved.replace(
            "    var end_depth_view = 0.0;\n",
            &format!("    var end_depth_view = 0.0;\n{start}"),
        );
        let missing = for_oblique_near_planes(&moved).unwrap_err();
        assert!(missing.contains("view_end_depth_from_buffer"), "{missing}");
        // In the shader changed, the cut comes after the end at the depth buffer
        let changed = for_oblique_near_planes(SAMPLE).unwrap();
        assert!(
            changed.find("end_depth_view = min(").unwrap()
                < changed.find("end_depth_view = far;").unwrap()
        );
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
        let clamp = changed.find("(bottom - camera_y) / rise").unwrap();
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
        assert_eq!(
            *app.world().resource::<HazePerCamera>(),
            HazePerCamera(true)
        );
        let registry = app.world().resource::<EmbeddedAssetRegistry>();
        let shader = registry
            .remove_asset(Path::new(SHADER_PATH))
            .expect("Bevy's shader of the volumetric fog");
        let source = std::str::from_utf8(shader.value()).unwrap();
        // The cut of the rays to the layer after Bevy ends them at the depth buffer
        assert!(
            source.find("if (ndc_end_depth_from_buffer > 0.0)").unwrap()
                < source.find("end_depth_view = far;").unwrap()
        );
        for change in CHANGES {
            let at = source
                .find(change.new)
                .unwrap_or_else(|| panic!("not changed: {}", change.old));
            // The end at the depth buffer is changed itself, after the cut checked it came first
            for read in change
                .reads
                .iter()
                .filter(|read| !read.starts_with("end_depth_view"))
            {
                assert!(
                    source[..at].contains(read),
                    "`{read}` no longer before `{}`",
                    change.new
                );
            }
        }
        assert!(
            !source.contains(CHANGES[1].old),
            "the start depth not changed"
        );
    }

    /// Another Bevy, whose shader no longer has the lines: the shader is left as it is, and the
    /// haze falls back to the player's camera alone, which Bevy's shader draws right.
    #[test]
    fn a_shader_that_cannot_be_changed_leaves_the_haze_to_the_players_camera() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<Mesh>()
            .add_plugins(VolumetricFogPatchPlugin)
            .add_plugins(VolumetricFogPlugin);
        app.world()
            .resource::<EmbeddedAssetRegistry>()
            .insert_asset(
                PathBuf::from(SHADER_PATH),
                Path::new(SHADER_PATH),
                b"another shader".to_vec(),
            );
        assert_eq!(
            *app.world().resource::<HazePerCamera>(),
            HazePerCamera(true)
        );
        app.finish();
        assert_eq!(
            *app.world().resource::<HazePerCamera>(),
            HazePerCamera(false)
        );
        let shader = app
            .world()
            .resource::<EmbeddedAssetRegistry>()
            .remove_asset(Path::new(SHADER_PATH))
            .unwrap();
        assert_eq!(shader.value(), b"another shader");
    }
}
