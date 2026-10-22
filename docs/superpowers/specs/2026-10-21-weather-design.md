# Weather — design

Date: 2026-10-21. Branch `feat/weather`, worktree `feat-weather/`, from main `b2a3bc5c`.

The user picked direction D, "Météo", of the roadmap artifact
(https://claude.ai/artifact/JLHeUcdEQAJuCKxVzncwau), delegated every decision, and asked above
all for it to be **very aesthetic**. This file records the decisions; later rulings are appended
under **Decisions**.

## Goal

Make the sky change. Rain rolls in under darkening clouds, drops streak past and splash on the
ground and ring the water, the world turns wet and glossy, snow drifts down in the cold biomes,
and thunderstorms crack the sky open with lightning and rumbling thunder that is heard a moment
after the flash. Today the sky is always clear.

What makes it good is the look and the feel, not the mechanics. Every lot is judged on pictures
(see **Looking at it**), and a lot is not done until its pictures are beautiful.

## Scope

In:

1. A weather state held by the server (clear, rain, thunderstorm), changing on its own over time,
   kept in the save, sent to every client, set with `/weather`.
2. The sky reacting to it: thicker and darker clouds, an overcast sky, dimmer sun, hidden sun,
   moon and stars, closer fog, all eased in and out.
3. Precipitation: rain streaks, snow flakes where it is cold, nothing in the deserts, stopped by
   roofs; splashes on the ground and ripples on the water.
4. A wet world in the High and Ultra presets: darker and glossier exposed ground, puddles that
   reflect the sky.
5. Thunderstorms: lightning bolts, flashes that light the world and the clouds, thunder heard
   after the flash; rain and wind ambience muffled under a roof; a Weather volume slider.

Out, on purpose:

- **Gameplay effects.** Lightning does not hurt, set fires (there is no fire) or change mobs;
  rain does not grow crops (there are none) or fill anything. Combat (direction B) and farming
  (direction C) are where these belong.
- Snow piling up on the ground, ice forming, rainbows, hail, fog banks, seasons, wind moving the
  leaves harder.
- A per-player weather setting or a "no rain" option: the presets scale the cost instead.

## Lots

Each lot gets its own plan in `docs/superpowers/plans/` and is built test first. `just check`
green at the end of each lot. Commits follow the repo's Conventional Commits, in English, no
attribution trailer.

### Lot 1 — the weather state, shared and server

**Model.** `shared/src/world/weather.rs`:

```rust
pub enum WeatherKind { Clear, Rain, Thunder }
```

- The server holds `ServerWeather { kind, ticks_left: u32 }`, counted down in `Simulate`. When it
  reaches 0 the weather changes: after `Clear` comes `Rain` (2 in 3) or `Thunder` (1 in 3); after
  `Rain` or `Thunder` comes `Clear`. Durations, drawn uniformly: clear 12 000–120 000 ticks
  (10 min–100 min, half a day to five days), rain and thunder 6 000–18 000 ticks (5–15 min).
- A new world starts clear, with a clear spell drawn as above.
- The random choices go through pure helpers taking `rng: &mut impl Rng`, tested with a seeded
  `StdRng`, as the mobs do.
- `/weather <clear|rain|thunder>` sets the weather for a natural duration of that kind, answers
  "Weather set to rain" to its author only, and is added to `COMMANDS` for the chat's
  suggestions. No permissions, like the other commands.
- **Saving.** `world.ron` gets `#[serde(default)] weather: Option<SavedWeather>` (kind and ticks
  left); `FORMAT_VERSION` stays 5. A world without it starts clear.
- **Network.** `AuthRegisterResponse` gets the current `weather: WeatherKind`; a new
  `ServerToClientMessage::WeatherSet(WeatherKind)` (appended, pinned) is sent to every client when
  the weather changes. `PROTOCOL_ID` goes 24 → 25, once for the whole branch.
- **Lightning, server side.** During a thunderstorm the server draws a strike every 3–15 s
  (uniform, in ticks) near a random player: a column at 16–160 blocks, any direction, struck at
  its top non-air block. It is sent to every client as
  `ServerToClientMessage::LightningStrike { at: IVec3, seed: u32 }`; the seed shapes the bolt so
  that everyone sees the same one. No damage, no block change.

**Client.** `ClientWeather { kind, rain: f32, thunder: f32 }`:

- `rain` eases towards 1 for `Rain` and `Thunder`, and towards 0 for `Clear`; `thunder` eases
  towards 1 for `Thunder` only. Both move at most 1/20 per second (20 s from clear to a full
  storm and back), on a smoothstep so they start and stop gently.
- At join both are set at once, without easing, to the state the server sends.
- Every effect below reads these levels through its own curve, so that a storm rolls in: the
  clouds and the sky lead (they follow `smoothstep(0, 0.5, rain)`), the drops follow
  (`smoothstep(0.25, 1, rain)`).
- The debug HUD (F3) shows `Weather: rain 0.85`.

### Lot 2 — the sky

- **Clouds.** Coverage rises from today's ~45 % to ~80 % in rain and ~92 % in a thunderstorm,
  smoothly, with no hitch and no hole: the mesh can no longer be rebuilt at each step of the
  threshold, so cells carry their density and the shader decides which cells and faces show
  (a face between two cells shows when exactly one of them is above the threshold). Tint goes
  from white to a blue-grey (`#8C93A0`-ish at noon in rain, darker in a storm, `#5A606B`-ish),
  opacity from 0.8 to 0.95. The clouds are still lit by the time of day.
- **Overcast.** One overcast level and colour, shared by everything that shows the sky, so
  that no blue is left in a storm (see decision 17):
  - a dome around the camera, drawn in the transparent pass over the sky pixels (Bevy draws the
    sky after the opaque pass, so the dome cannot sit between them), a grey gradient lighter at
    the horizon and darker at the zenith;
  - `sky_radiance` (fog of the chunks, water's sky reflection), the horizon skirt and the
    clouds blend towards the same grey;
  - the environment-map light of the atmosphere is turned down with the overcast level and a
    grey ambient light takes over, so the world is lit from an even grey sky;
  - the grey's brightness follows the *clear* sky's brightness at that time of day (a function
    of the sun's height), not the dimmed physical sky's;
  - in the Off preset, the clear colour and the fog colour are blended to the same grey.
- **Sun, moon and stars** fade out with the overcast level (gone by 0.8).
- **Light.** Sun (and moon) illuminance goes down to 25 % in rain and 12 % in a storm, so shadows
  fade out; the ambient light keeps the world readable. The exposure follows so that a rainy noon
  is clearly darker than a clear one but not dim (about −1.3 EV in rain, −2 EV in a storm).
  Colour grading loses some saturation (1.12 → 0.9).
- **Fog.** Today the chunks fade between 60 % and 100 % of the render distance. In rain the fog
  closes in: from 30 % to 75 % in rain, from 15 % to 60 % in a storm, both eased with the
  overcast level, never ending closer than 48 blocks (low render distances, and `sky_fog`'s
  underwater test, which takes a fog ending within 16 blocks for water). The world fades into
  the overcast grey; the clouds' fade and the horizon skirt follow it.
- **God rays** (High, Ultra) fade out with the sun; the volumetric fog gets a little denser in
  rain for haze.

### Lot 3 — precipitation

- **Where.** Each column has a kind of precipitation, from the client's `WorldBiomes`: snow in
  `IcePlain`, in the snowy mountains and in any column whose surface is above the snow line (122),
  nothing in `Desert` and `Savanna`, rain elsewhere. It falls down to the column's highest block
  that stops it: any block but air (leaves and glass stop it; water stops it at its surface).
- **Occlusion map.** A texture of the columns around the camera (at least 96 × 96), holding for
  each the height that stops precipitation and its kind, kept up to date as chunks arrive and
  blocks change (a placed roof stops the rain within a frame or two). The GPU reads it; drops
  below it are not drawn.
- **Rain.** Thin streaks, drawn by the GPU from one static mesh (the vertex shader places,
  moves and wraps each drop around the camera, so that drops stay put in the world as the
  camera moves and the CPU does nothing per drop). Each drop has its own speed and length
  (falling ~ 14 blocks/s, streaks 0.6–1 block, a few pixels wide), slanted by a slow wind that
  follows the clouds' drift and gusts in storms. Soft-edged, translucent, lit by the sky (dim at
  night, grey by day, bright at a lightning flash). The number drawn follows the rain level, so
  that it starts with a few drops. Up to 8 000 drops in Ultra, fewer in lower presets
  (Off 2 500). Two depth bands: big near drops, finer far ones, out to ~40 blocks.
- **Snow.** Flakes: small soft round sprites, falling at ~1.5 blocks/s with a lazy sway and
  slow spin, varied in size, out to ~48 blocks; they show snow's quiet more than its density.
- **Splashes.** On the ground of rain columns near the camera (~16 blocks), tiny crowns of
  droplets that pop up and fall back in a quarter of a second, scattered in time and place,
  also GPU-driven from the occlusion map.
- **Ripples.** On water under rain, expanding rings drawn into `water_normal` (hashed cells,
  each with its own phase), with their number following the rain level. Every preset that draws
  water ripples (the `effects` setting) gets them.
- **Underwater**, no precipitation is drawn.

### Lot 4 — the wet world (High, Ultra)

- **Wetness** goes up while it rains (40 s to soaked) and down after it stops (2 min to dry);
  only the columns where rain falls and that are open to the sky (sky light 15, top faces and
  the sides near them) get wet: not under a roof, not in snow or desert columns (the climate map
  says which).
- **Wet surfaces** darken (albedo ×0.65 at full wetness) and turn glossy (lower roughness).
- **Puddles.** On top faces, a world-space noise lays out puddles that fill in as the wetness
  rises past half: near-mirror, slightly darker, with the rain's ripples in them, reflecting the
  overcast sky (through `sky_radiance`) and catching lightning flashes. They do not reflect the
  world (decision 16).
- These values go through a separate uniform binding, not `ChunkShading` (changing it rebuilds
  every chunk's bind group).

### Lot 5 — thunderstorms and sound

- **Bolt.** From each `LightningStrike` within the render distance: a jagged bolt from the cloud
  base down to the struck block, with branches, shaped from the strike's seed (the same for every
  player); a white-violet core with a glow, bright enough to bloom in the presets with HDR. It
  lives ~0.5 s and flickers (two or three pulses).
- **Flash.** Each pulse lights the world (ambient and exposure jump), the veil and the clouds
  (most near the strike). A strike beyond the render distance shows only as a flash inside the
  clouds in its direction, without a bolt.
- **Thunder.** Heard after the flash, delayed by distance at 340 blocks/s; a sharp crack for a
  strike within ~48 blocks, a long rumble further off, quieter with distance. Played without
  spatial attenuation (Bevy's is too steep for this), the volume set by hand, panned towards
  the strike.
- **Rain sound.** A seamless rain loop and a muffled one (as heard from indoors, low-passed),
  crossfaded after how open the player is: the share of the columns around them (radius ~6) whose
  precipitation stops above their head. Its volume follows the rain level and the share of rain
  columns around; snow columns play the existing `ambient/wind.ogg` instead, softly; thunderstorms
  add wind too. Underwater, every weather sound is muffled.
- **New sounds** are synthesised by `scripts/generate_sounds.py` like the others: the two rain
  loops, three cracks, three rumbles.
- **Weather volume.** A fourth audio channel, "Weather", with its slider in the Audio settings.

### Closing

README (features), `docs/ARCHITECTURE.md` (weather state, network messages, rendering),
`CHANGELOG.txt`, a final independent review, and an artifact for the user with the pictures.

## Looking at it

A lot that draws something adds an `#[ignore]` e2e snapshot test in the style of
`client/src/e2e/clouds.rs` (`UI_SNAPSHOTS=1 cargo test -p client e2e::weather -- --ignored`),
with fixed camera, time and weather levels, in the High preset unless said otherwise. The
orchestrator looks at every picture and asks for changes until they are beautiful; this is part
of the lot, not an afterthought. Pictures to cover, as each lot makes them possible:

- noon clear / rain / thunderstorm, from the ground, looking at a landscape with water;
- sunset in rain, midnight in rain;
- a snow biome while it snows;
- standing under a roof looking out at the rain;
- a lightning bolt (frozen at its brightest pulse);
- wet ground and puddles at noon in High;
- rain in the Off preset.

## Decisions

1. The user delegated every choice and asked for it to be very aesthetic: the look decides close
   calls, and each drawing lot is reviewed on pictures.
2. No gameplay effect in this branch (no lightning damage, no fire, no crops): the visuals and
   sounds are the feature; directions B and C will add effects that have something to act on.
3. Weather cycle: clear 10–100 min, rain/thunder 5–15 min, thunder one rain in three. Shorter
   clear spells than Minecraft's (up to 2.5 h) so that weather is actually seen in a session.
4. `/weather` takes no duration: a natural one is drawn; one fewer argument to learn.
5. Only the kind is sent to clients, never the time left: the client eases the levels itself, so
   a change costs one small message and a client joining mid-storm snaps straight to it.
6. 20 s to roll a storm in or out, with the clouds and sky leading the drops.
7. Lightning is drawn by the server and broadcast with a seed, so that every player sees the same
   bolt at the same place; up to 160 blocks from a player, so that most strikes are seen but some
   only heard and glimpsed in the clouds.
8. Rain or snow per column from the biome and the surface height, computed by the client from the
   seed: the server does not need to know.
9. Precipitation is entirely GPU-driven from static meshes and an occlusion texture: thousands of
   drops cost no CPU work per frame and no entity per drop.
10. Leaves stop the rain (as in Minecraft): forests give shelter, which is good to see.
11. Cloud coverage changes smoothly in the shader rather than by rebuilding the mesh at each
    step, to avoid hitches and pops during the 20 s transitions.
12. Wet ground and puddles only in High and Ultra (the roadmap's call): they need HDR and
    lighting headroom to look right; lower presets get every other effect.
13. Thunder is non-spatial with a hand-made volume and pan, because Bevy's spatial attenuation
    would silence a strike 100 blocks away.
14. A "Weather" audio channel rather than reusing Effects: rain is long and loud, and players
    want to turn it down without losing the block sounds.
15. No video toggle for precipitation: the presets scale the drop count, and Off still shows
    rain (fewer drops).
16. Puddles reflect the sky, not the world: the only SSR is the water's, which reads the frame
    copied for the transmissive pass; the ground is opaque and drawn before that copy. A
    separate puddle layer in the transmissive pass would cost a mesh path for little gain under
    a grey sky.
17. Overcast is one shared level and colour fed to the sky dome, `sky_radiance`, the horizon
    skirt, the clouds and the ambient light, rather than a dome alone: a dome would leave blue in
    the water's reflection, the fog, the horizon ring and the environment light.
18. Rain closes the fog in (the end moves, not only the start), with a 48-block floor: a storm
    should feel closed-in, and the floor keeps low render distances and the underwater test sane.
19. The client's rain and thunder levels are the smoothstep of ramps that move linearly at 1/20
    per second: a full change takes 20 s and starts and stops gently (the level itself moves
    at up to 1.5/20 per second midway), and a change turned back midway does not jump.
20. The lightning timer is not saved (a storm loaded draws its first strike 3–15 s later), and a
    struck column is searched for its top block from 128 blocks above its player down to 128
    below, the vertical reach of a render distance; the distance is drawn uniformly in
    16–160 blocks, so that close strikes are not rare.
21. `/weather` naming the weather already there draws a new duration for it and sends nothing.
22. The F3 line is `Weather: <kind> <rain>`, with `/<thunder>` while there is thunder:
    `Weather: rain 0.85`, `Weather: thunder 1.00/0.40`.
23. The overcast reaches the shaders through one storage buffer, `WeatherShading` (the horizon
    and zenith greys and the overcast level), at binding 107 of the chunk, horizon skirt and
    dome materials and declared once in `rustcraft::sky`. It is rewritten in place only when it
    changes, so `ChunkShading` stays free of per-frame values and the bind groups stay valid.
    The clouds, which have a uniform of their own, get the same greys there.
24. The overcast level is `smoothstep(0, 0.5, rain)` and the storm level
    `smoothstep(0, 0.5, thunder)`: the sky is fully grey once the rain is half in, so that a
    rain mostly in shows no blue. Every curve of the lot blends clear → rain by the first and
    rain → storm by the second.
25. The dome is an icosphere at the camera drawn at the far plane (depth 0) in the transparent
    pass with a large depth bias, blending the overcast grey over the sky by the overcast level,
    with no prepass or shadows. It exists only with the physical sky and is hidden while the
    level is 0, so clear weather draws exactly as before. `sky_radiance` became the sky as seen
    (without the environment-map intensity), blended to the same grey.
26. The greys are keyframes on the sun's height (day, golden hour, night), apart for the
    physical sky and for Off, since the two skies differ in brightness; the zenith is a little
    darker than the horizon, and a storm darkens both. They follow the hour, not the dimmed sky.
27. The light under the overcast is set for what the screen shows, not for physics: the sun
    falls to 12 % in rain and 5 % in a storm (25 %/12 % left shadows hard, as the sun weighs
    about five times the sky's light on screen), the environment-map light falls to `1 − level`,
    and an ambient light as bright on screen as the grey sky takes over (its brightness scaled
    by the exposure). The exposure does not change: lowering it as well made rain twice dark.
    A rainy noon is about 30 % darker than a clear one on the hills, a storm about 40 %.
28. Saturation is graded down in rain also in Off (× 0.9/1.12 in rain, × 0.78 in a storm, of
    whatever the preset grades).
29. The god rays fade by their density, not their light, and the volumetric fog does not get
    denser in rain: unlit, Bevy's volumetric fog only darkens the sky, and it has no ambient
    term worth using for a haze. The closer fog does the haze.
30. The fog never ends closer than 48 blocks but never beyond the render distance either; the
    clouds' fade follows the fog's end.
31. The clouds' coverage grows by a density threshold measured on the field (45 %, 80 %,
    92 %). Densities are banded so that faces of equal bands merge; a face shows when its
    threshold lies between the densities of its two cells, so cells appear one by one, never
    with a hole. Under the overcast the clouds take a blue grey darker than the overcast at that
    hour (68 % of the zenith in rain, 56 % in a storm), rather than a fixed tint, so that they
    stay darker than the sky at sunset and at night; far clouds fade into the horizon grey.
32. Under the overcast the water loses 60 % of its blue to a grey of the same brightness.
33. The pictures of `e2e::weather` hold the weather with a system after the easing, since
    pausing virtual time dropped the connection; High and Off are taken in two tests, since
    switching from High to Off in one session panics in Bevy's atmosphere.
34. Changing 31: under the overcast, a rain cloud seen from under is a deck darker than the
    sky, not a veil. The clouds are 85 % of the overcast at the zenith in rain and 70 % in a
    storm, slightly blue (the bottoms 70 % of that), the thicker cells darker (down to 72 %) and
    each cell ±8 % apart by a hash of the cell. They fade into the overcast sky behind them, in
    their own direction, from 35 % of their reach on: the horizon grey would lighten the deck
    overhead, and an earlier fade dissolved it.
35. Changing 26 and 27: a storm is clearly moodier than rain. The overcast turns to a blue slate,
    the horizon × (0.56, 0.6, 0.67), the zenith darker again × (0.74, 0.77, 0.82), and the storm
    lets 50 % of the light of the sky through. Without the shaders, that also dims the daylight
    ambient: otherwise a storm there was as bright as rain, its closer fog lightening the hills.
    At noon the hills are about 30 % darker than clear in rain, and about 48 % darker in a storm.
36. Adding to 26: at sunrise and sunset a dull orange pink glows low on the overcast toward the
    sun, (0.75, 0.36, 0.13) times the brightness of the overcast at the horizon. It shows with
    the sun from 0.15 under the horizon to 0.45 above it, falls off with the cube of the cosine
    from the sun's azimuth and above a height of 0.45, and is at 45 % in a storm. It sits in
    `overcast_sky`, so the dome, the fog of the chunks, the water and the far clouds share it.
    Without the shaders the sky is one colour, so it has no glow.
37. Adding to 26: the overcast at night is a faintly cool grey, not a neutral one.
38. Changing 9: the rain and the snow have no meshes. A pass of its own builds every drop,
    flake and splash from the vertex index alone (six vertices each, twelve for a splash), so
    nothing is uploaded but a small uniform. It draws after the temporal anti-aliasing: drawn
    before, thin blended streaks with no motion vectors were smeared away by the history, as
    Bevy's docs warn for particles. It tests the depth of the world by hand, the multisampled
    depth without the shaders, and blends premultiplied.
39. The climate map tells what falls by its blue channel, the column's own kind (rain, snow, or
    nothing in the desert and the savanna), not blended with its neighbours. Rain turns to snow
    above the snow line of the generation (122), each drop at a height of its own within ±4, so
    the line is a band, not a cut.
40. The occlusion map is 144 × 144 R32Float, the 9 × 9 chunk columns around the eye, holding
    the height at which what falls stops. A column not loaded stops everything; flowers, grass
    and other decorations let the rain through; water stops it at its surface. At most 12 chunk
    columns are scanned a frame, the nearest first; a changed block rescans its own column the
    same frame.
41. The counts per quality: rain and snow 2500 Off, 3500 Low, 5000 Medium, 6500 High, 8000 Ultra;
    splashes 160 to 500. The qualities that draw fewer drops draw them more opaque (× 1.6 Off down
    to × 0.9 Ultra), so that the rain reads as about as heavy everywhere. How many show follows
    the level of the drops, `smoothstep(0.25, 1, rain)`.
42. The rain falls in three bands around the eye: 15 % wide soft faint drops within 9 blocks,
    40 % finer ones to 24, the rest finest to 40; 12 to 16 blocks a second, 0.6 to 1 block
    long, never drawn thinner than 1.3 pixels (fainter instead, by the square root of how much
    thinner, so far drops haze). They take the grey of the overcast a little darker than the sky,
    with a faint floor so they glimmer at night. The snow falls in three bands too (to 8, 20 and
    40 blocks), 1 to 2 blocks a second, swaying and spinning, as soft lobed discs a little
    brighter than the sky.
43. The wind blows along +X, at 0.5, 1.6 and 4.8 blocks a second in clear, rain and a storm, and
    gusts in a storm; what falls drifts with it, so a storm slants.
44. Splashes pop up within 16 blocks where the rain stops on open ground or water, below the snow
    line: a soft ring thrown out and gone by half of their 0.26 s, and droplets thrown up; on the
    water, smaller and without the ring, as the water rings itself.
45. The rain rings the water open to the sky (sky light from 14 to 15) where the climate gives
    rain, below the snow line: in cells of 0.55 blocks one ring at a time, living about 0.8 s and
    spreading 0.3 blocks, in as many cells as the level of the rain says. They are part of the
    normal of the water, so they reflect, and fade out with the footprint of a pixel. They follow
    the water effect, as the waves.
46. Nothing falls while the eye is under water.
47. Adding to 38, changing 42: past the drops, the rain is drawn on sheets, three to five
    cylinders around the eye (three Off and Low, four Medium and High, five Ultra) from 12 to 58
    blocks, each further by the same ratio, from 40 blocks under the eye to 36 above it. Their
    fragments draw two layers of fine streaks that fall at 14 blocks a second, in whole columns
    around so that they close without a seam, slanted by the mean wind (by the gusts, they
    curled), thicker where the gusts blow, over a faint veil. Each fragment reads the occlusion
    map and the climate map at its own place, so a sheet never rains under a roof, in a cave or
    on a desert. The drops themselves keep two bands, 40 % within 9 blocks and the rest to 24;
    the band to 40 is gone. On lavapipe the sheets cost nothing measurable next to the fog.
48. Changing 41: rain shows 70 % of the drops of the quality and a storm all of them, the sheets
    as much more opaque: a storm is a downpour.
49. Changing 43: the wind of a storm blows at 6.5 blocks a second, so a storm slants clearly.
50. Changing 44: half of the splashes fall within 8 blocks, the others within 16; the crown is
    larger, with three to five droplets, and a brief bright tick where the drop hits.
51. Changing 42: the drops and the sheets keep a cool glimmer, (0.032, 0.038, 0.052), so that
    the rain is seen at night, still dark.
52. The pictures of `e2e::weather` hide the outline of the block aimed at.
53. The wetness is the client's own, never sent: it rises by the level of the drops over 40 s
    (`precipitation() / 40` a second) and falls by 1/120 a second once nothing falls. A player
    joining in rain or in a storm finds the world soaked.
54. The wet world is `ShaderFeatures::wet`, High and Ultra only: elsewhere the wetness reaches
    the shaders as 0 and every wet term is skipped, so clear weather in every preset, and rain
    in Off, Low and Medium, draw as before.
55. What the rain falls on comes from the occlusion map of lot 3, bound to the chunks at 108: a
    top face is wet where its column's stop height is not above it, a side face where the column
    in front of it lets the rain fall past it at its height (so the walls under an overhang stay
    dry and the cliffs get wet), in streaks of columns of texels, wettest within 2 blocks under
    the top; a bottom face never. Beyond 40 blocks along x or z, where the map may not hold the
    column, a full light of the sky stands in for it, all of it from 48. Only the columns whose
    climate gives rain, faded across the band of the snow line as the drops.
56. Each block texture has a surface (`WetSurface`, in the bits of the table over the fading of
    the frames): hard (stone, cobblestone, gravel, wood, ores, and any texture of a pack unknown
    here) × 0.62, glossy, puddles; dirt × 0.68, less glossy, puddles at 60 %; grass × 0.72, a
    faint sheen, no puddle (a puddle on a lawn read as a grey film); sand × 0.55, hardly glossy,
    no puddle; leaves and plants × 0.8; glass, ice, snow, torches and the cracks as they are.
57. The sheen is drawn by hand after the light, not by Bevy's specular: the chunks keep a
    reflectance of 0, as the environment map would add the blue sky a second time in a clearing
    sky. A Fresnel term with the roughness mixes the face toward the sky it reflects
    (`sky_radiance`, blurred by the roughness, none under the horizon) and adds a lobe of the
    sun, shadowed; it follows the pixels, more in the darker texels (the water in the cracks)
    than on the lighter ones. An even sheen washed the cobblestone into a grey sheet at a
    glance.
58. The puddles lie in the hollows of a world-space noise (two octaves, 2.8 blocks and a third
    of that, turned, spread evenly from 0 to 1) lowered by the relief of the texels, the darker
    texels of the pixel art taken as lower: their edges follow the pixels of the blocks without
    snapping the noise to the 16 × 16 grid, and the relief fades out where a texel is smaller
    than a pixel. They cover up to 42 % of hard ground, filling from a wetness of 0.5 while it
    rains and lingering down to 0.1 as it dries, and keep away from the edges where the ground
    drops. Inside, the ground is darker (× 0.55, less at the shallow edge), under a mirror of
    the sky by the Fresnel of water (0.02), rippled by the rings of lot 3, with a glint of the
    sun; a flash of lightning brightens the sky they reflect by `1 + flash`.
59. While drops fall the ground is wet all over, soaked at a wetness of 0.4; once they stop it
    dries from the high spots of the same noise, so the hollows and the puddles dry last.
60. The pictures of the wet world look at a terrace built on the shore (a dirt path, a plaza of
    cobblestone and stone, sand, a low wall, a pier of planks a step up), in High, and in Ultra
    and Medium in tests of their own. Run all at once with the others, ten clients on one
    machine, some lost their camera: they pass with `--test-threads=4`.
61. The bolt is grown from the seed of its strike by a small PRNG of our own (SplitMix64), not
    `rand`, so that every build and every client draws the same bolt. Its main channel leaves
    the cloud base (193) up to 28 % of its height aside, and turns at every scale (a roughness
    of 0.34 at the scale of 3.5 blocks, 0.17 at 36, with a few sharp kinks); four to nine
    branches leave its upper part going down, forking up to three deep, thinner and fading to
    their tips.
62. A strike flashes in two or three return strokes over half a second, the first the
    brightest. Only the first lights the branches, which go dark faster than the main channel
    (a decay of 26 ms against 40); the later strokes re-light the main channel alone, as a real
    channel re-strikes along its ionised path.
63. The bolt is a pass of its own after the TAA (as the rain's, decision 38): the points of the
    channels in a storage buffer, each piece a ribbon facing the camera, mitred to the next. Its
    hot core is drawn at least 0.75 px wide (fainter when thinner), its violet glow 11 to 18
    times wider with the distance, at least 6 px; they are added to the frame, the core 16 times
    brighter than white and the glow 2.6 with the shaders, for the bloom, and 1.6 and 0.75
    without. A bolt beyond the render distance is not drawn: only its flash in the clouds.
64. The flash reaches the shaders through `WeatherShading`: `strike` (the struck block, and the
    level of the channel) and `flash_dir` (from the eye to the flash in the clouds). The
    overcast sky brightens by 0.05 all over and up to 0.55 more toward it; the clouds glow within
    56 blocks of the cloud point (1.1), and faintly all over (0.06); the chunks get a light from
    the bolt itself, a vertical line from the struck block up (3.0, within 9 blocks), and a
    light from the flash in the clouds on the faces turned to it (0.45), both through the light
    of the sky, so caves stay dark. The puddles reflect the sky, which flashes already: their
    own `1 + flash` of decision 58 is gone.
65. The even light of a flash is added to the ambient light, a quarter of the overcast at noon
    on screen, and with the physical sky the exposure opens by a quarter of a stop. A first
    flash as bright as the overcast at noon washed the world into a grey sheet by night and
    drowned the bolt by day; softer, the bolt is what shines. Without the shaders, the colour
    behind the world and the fog brighten (`FLASH_SKY`).
66. The strikes are aged in `PreUpdate`, before every system that reads the flash, so the
    pictures can hold a strike at a stroke by setting its age after them.
67. The thunder is delayed by its distance at 340 blocks a second. Within 48 blocks it cracks
    and booms (`thunder_near`), farther it rumbles (`thunder_far`); its volume is
    `(48 / d)^0.75`, at least 0.15, and none beyond 1,500 blocks. It is muffled to 55 % closed
    in and 30 % under water.
68. The thunder is not attenuated by Bevy's spatial sound, which would make a strike 100 blocks
    away inaudible: its emitter is kept 2 blocks from the ears toward the strike, moved every
    frame, inside the radius where rodio does not attenuate (`audio::SPATIAL_SCALE`), and only
    its balance between the ears follows the strike; its volume is raised by 4/3, as rodio
    plays a sound right ahead at three quarters in each ear.
69. How open the player stands is found by 16 rays at the height of the eye, 6 blocks long,
    stopped by any block but the decorations and the liquids, each open when it reaches a column
    on which the rain falls below the eye (the occlusion map). A column of its own open to the
    rain is the open; else the roof is the height of the rain's stop above the eye. The rain in
    the open plays at the share of open rays; the drumming on the roof at full volume under a
    roof up to 4 blocks above, none from 24 (a cave). Under a pavilion both play.
70. The share of rain and snow around is read on the climate map at nine columns 8 blocks apart;
    above the snow line, rain is snow. The snow and the storms bring the wind (0.6 of the snow,
    0.5 of the storm), quieter sheltered. Under water only the drumming plays, at half.
71. The three loops play all along the game, paused while silent, their volumes eased over
    0.35 s, so walking through a door crossfades the open rain into the drumming.
72. The sounds are synthesised by `scripts/generate_sounds.py`. The rain is thousands of drops
    of a few sizes layered over a soft hiss, a 16 s loop crossfaded at its seam; indoors, the
    same rain low-passed at 650 Hz under a drumming on planks. The thunder is heard from a
    channel modelled as the bolt is (a tortuous line of segments with branches and strands in
    the cloud), each segment an N-wave arriving after its distance, filtered by the air an
    octave lower per doubling of the distance, with echoes and a roll; its time starts with the
    sound of the foot of the channel. `--only PREFIX` writes only the files under it, so the
    other sounds stay byte for byte as they were.
73. Changing 43: what falls drifts by the wind summed frame after frame on the CPU
    (`WindDrift`), sent in `WeatherShading` and wrapped at 3,600 blocks, a whole number of the
    boxes of every band (the snow's at the 0.8 of the wind it drifts with); the shader adds only
    the gusts' bounded sweep to and fro. The wind of now times the time since the start, as
    before, made the drops race sideways as the wind eased in: their pace was the wind plus
    the time times its change. `WeatherShading` is now written every frame.
