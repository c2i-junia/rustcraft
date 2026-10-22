# Portals, lot 10: the god-ray haze and SSAO through a portal

Requirements and evidence: `.superpowers/sdd/portals/seam-diagnosis.md` (causes 2 and 3, F2 and
F3), `.superpowers/sdd/portals/lot-9-report.md` (the state after F1). Branch `feat/portals`, from
`4274fa30`.

## Goal

Walking through a close portal is seamless at High (the default) and Ultra too, at noon, at
dusk and at night: the view through the portal gets the god-ray haze the player's camera draws
(F2), and SSAO through the portal measures no worse than the noise (F3). Out of scope: far
twins (F4), the dark horizon colour (F6).

## F2: the spike, and the choice

The ruling asks for a time-boxed spike of option (c), the view through the portal taking part
in the player camera's screen effects through a reprojected depth, falling back to (a), a
portal-camera-only fork of Bevy's volumetric pass, or a simpler route reaching the same result.

**What Bevy 0.19 reads (source of `bevy_pbr-0.19.1`).**

- `volumetric_fog` runs in `Core3d` after `MainPass` and before `EarlyPostProcess`. It reads
  `ViewDepthTexture`, the depth the main pass wrote, not the prepass.
- Each step of a ray samples the directional light's cascades of the view drawing it
  (`get_cascade_index(P_view.z)`, `world_to_directional_light_local(P_world)`), at the world
  position along the ray from that view's eye.
- SSAO runs between `Prepass` and `MainPass`. Its texture is read by `pbr_fragment.wgsl` while
  each mesh is lit: it darkens the ambient light of a surface as it is shaded. It is not a
  post-process over the screen.

**Why (c) is rejected.**

1. **It cannot give SSAO.** The portal surface shows an image its camera already lit. A depth
   written for the portal surface, in the prepass or later, cannot re-light it. F3 does not come
   for free.
2. **The shafts would be the wrong ones.** The player's volumetric pass would march the
   reprojected rays through the *virtual* space behind the portal surface, which is the world on
   this side of the blue portal, not around the twin. It would sample the shadows of the terrain
   there: in the seam scene 24 blocks away, and anywhere at all for a far or rotated pair.
3. **It needs a new render node.** The reprojected depth is farther than the wall the portal
   sits in, so a write in the main pass loses to that wall's faces in the depth test. It must
   overwrite the main depth after `MainPass`, only on the portal's pixels, which a pass cannot
   find from the depth attachment it writes (a stencil or a depth copy is needed). TAA's motion
   vectors would stay those of the surface either way.

**The simpler route (a′).** Bevy's `volumetric_fog.wgsl` uses the whole inverse projection
everywhere but in one line: the start depth of a ray, `-depth_ndc_to_view_z(frag_coord.z)`,
which is `near / z` and ignores an oblique near plane. Everything else (the depth of the opaque
surface, the start point, the steps, the cascades) is right under the oblique matrix, and the
interior plane is drawn at NDC depth 1, the oblique plane itself. So:

- replace that line by `-view_start_pos.z`, the depth of the start point the shader already
  computes, in Bevy's embedded copy of the shader, before it is loaded (`EmbeddedAssetRegistry`);
- under a plain projection it is the same depth: the player's camera is unchanged;
- portal cameras get `VolumetricFog` (same step count, no jitter: they have no TAA).

No Bevy code is copied; a Bevy upgrade that changes the line fails a unit test at once.

If the measures show (a′) is not enough, fall back to (a) proper.

## Steps

1. **The shader fix (TDD).** `world/rendering/volumetric_fog.rs`: the pure text change, and a
   plugin that changes Bevy's embedded shader.
   → verify: a unit test on a sample, and one that builds `VolumetricFogPlugin` and finds the
   real shader changed (red with the plugin stubbed).
2. **Portal cameras draw the haze (TDD).** `apply_camera_features` gives portal cameras
   `VolumetricFog`, jitter 0.
   → verify: `a_portal_camera_draws_the_picture_and_the_players_camera_finishes_it` updated
   first (red), then green.
3. **Measure F2.** `seam_close_high_{noon,dusk,night}` and `seam_close_ultra_{noon,dusk}`,
   before (at `4274fa30`) and after. The walk at noon goes through the oblique-to-plain switch
   of the portal camera (0.12 block), which shows whether the oblique haze matches.
4. **F3, measured after F2.** A variant that takes SSAO from the portal cameras only. Keep the
   smaller of: SSAO kept, SSAO off on the portal cameras.
5. **Cost.** `bench_portal_frame_time` on the GPU at High, before and after. Budget: about
   3 ms per active portal camera; above it, fewer steps on the portal cameras.
6. **Gates.** `seam_close_high_{noon,dusk}_is_seamless`, thresholds from the measures; run red
   at `4274fa30`.
7. **No regressions.** The Off/Low/Medium gates; the main view without portals: same shader
   result (a plain projection gives the same start depth).
8. **Docs, checks, screens.** ARCHITECTURE, the spec's as-built notes; clippy, fmt, the
   workspace tests; screens to `.superpowers/sdd/portals/lot-10-screens/`.
