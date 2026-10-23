# Portals, lot 14: the shadows through a far twin

Branch `feat/portals`, base `9aa45024`. Spec: `docs/superpowers/specs/2026-10-20-portals-design.md`.

## Goal

Through a far twin, the portal cameras draw the shadows the player will see after crossing, at
every hour: no seam at dusk, when the shadows are long. The player's own view does not change,
neither its picture nor its cost; with a far twin in view the cost is measured and acceptable,
and with no portal in view it does not move.

## Root cause

- The groups beyond the player's own reach, sent only for a portal, are on
  `PORTAL_CHUNKS_LAYER` (3), drawn by the portal cameras only. The sky light's `RenderLayers` are
  `[0, BODY_LAYER]`.
- Bevy 0.19 filters shadow casters by the **light's** layers only, not by the view's:
  `check_dir_light_mesh_visibility` (CPU-culled meshes) tests `light_mask.intersects(entity_mask)`,
  and `collect_gpu_culled_meshes` hands a `NoCpuCulling` mesh to every shadow subview whose
  light's layers meet its own. So a layer-3 group casts in no view at all.
- The cascades themselves are per camera already: `build_directional_light_cascades` fits a set
  to the frustum of every active camera, the portal cameras included, and
  `CascadesVisibleEntities` lists the CPU-culled casters per camera and per cascade. The portal
  cameras' shadows of the layer-0 groups around a close twin work through that
  (`ShadowRole::ForTwinsOnly`, `keep_twin_shadows_to_portal_cameras`, final fix round 2).
- So through a far twin, the groups within `SHADOW_CASTER_DISTANCE` of the twin are already
  marked `ForTwinsOnly` (the twin is approached), culled on the CPU, but their layer keeps them
  out of every cascade. After crossing they are on layer 0 and cast: the shadows pop in.

## Approach (the simplest correct one)

1. The sky light shadows layer 3 too: `[0, BODY_LAYER, PORTAL_CHUNKS_LAYER]`.
2. Then no group on layer 3 may cast in every view (GPU culled), or it would reach the player's
   cascades from beyond its fog. `shadow_role` gets the render distance and makes the reach
   explicit, as the layer used to make it implicit:
   - `Own` (every view, GPU culled): within 12 chunks of the player **and** within its own reach
     (`drawn_by_player`, the same test as the layers);
   - `ForTwinsOnly` (portal cameras only, CPU culled, filtered out of the other cameras'
     cascades): within 12 chunks of an approached twin and within the reach the player will
     have there;
   - `None` otherwise.
   With a render distance under 12, a group 10–12 chunks from the player was `Own` but on
   layer 3, so it cast nowhere: it is now `None`, the same. A group near a twin but past the
   reach the player will have there cast for the portal cameras although it will not after
   crossing: it no longer does.
3. `shadow_casters_system` recomputes the roles when the render distance changes too.

Rejected: a second directional light for layer 3 (its own cascades for every camera, the
player's included, and a second light in every shader: a cost and a look change for the
player); per-camera layers on the light (Bevy has no such thing).

## Tests first

- Unit: `the_sky_light_shadows_the_world_and_the_body_seen_only_through_the_portals` asks for
  layer 3; `a_group_casts_only_within_the_reach_of_where_the_player_stands_or_will_stand`. Red,
  then green.
- E2E gates `seam_far_medium_dusk_is_seamless` and `seam_far_high_dusk_is_seamless`
  (`approach_seam`, twin 21 chunks away, render distance 8). The existing gates use medians,
  which the shadows do not move (most pixels are lit on both sides): at base, Medium passed and
  High failed only on the screen mean (2.60 for 2.5). A dusk check on the mean of the 64–96
  class (`SHADOWS_SEAMLESS`), with an in-test control (the twin's casters taken off through the
  portal must fail it), and a red control (the light without layer 3 must fail it).
- Bench `bench_far_twin_shadows` (release, GPU): far twin in view and no portal in view, as built
  and with the twin's layer-3 casters off, High and Ultra, dusk.

## Also (re-review nits)

- N1: haze comments in `volumetric_fog.rs` said two coincident cameras always look different
  ways; a vertical look is kept by a turn about the vertical, and those share a volume.
- N2: `drawn_in` loosened to a cosine of 0.9997 (shader 0.9998).
- N3: the seam set takes about 107 minutes, not an hour.

## Checks

`cargo clippy --all-targets -q -- -D warnings && cargo fmt --all -- --check && cargo test
--workspace --no-fail-fast`; the pre-merge subset and the two new gates; the full seam set once,
two at a time. Before/after JPEGs in `.superpowers/sdd/portals/lot-14-screens/`.
