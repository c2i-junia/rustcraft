# Weather Lot 5 (Thunderstorms and sound) Implementation Plan

**Goal:** A thunderstorm cracks the sky open. A jagged, branching bolt tears down from the
cloud base to the block it strikes, white-violet and blinding, blooming in the presets with
HDR; it flickers in two or three strokes over half a second, and each stroke lights the world,
the grey sky, the clouds (most around the strike), the puddles and the drops. A strike beyond
the render distance lights the clouds in its direction only. The thunder comes after the flash,
at the speed of sound: a tearing crack and a boom close by, a long low roll far off. The rain
is heard as a soft, seamless hiss outdoors, and as a muffled drumming on the roof indoors, the
two crossfaded after how open the player stands; snow and storms bring wind; under water every
weather sound is muffled. A fourth volume, "Weather", sets them all. To see and hear:
`/weather thunder`, out in the open, then under a roof.

**Spec:** `docs/superpowers/specs/2026-10-21-weather-design.md`, "Lot 5 — thunderstorms and
sound", and its "Decisions" (7, 13 and 14 above all, 23, 38, 52 and 58, up to 60).

## Global Constraints

- English for code, comments, docs and commits; Conventional Commits, no trailer.
- Doc comments in full sentences, tests named as sentences, sparse comments, as around.
- TDD for what can be tested without a GPU or a sound card: the bolt from its seed (the same
  seed, the same bolt; from the cloud base to the struck block; branches thinner and dimmer),
  the strokes of a strike over time, the flash by distance, the thunder's delay, kind and
  volume by distance, how open the player stands, the crossfade and the volumes of the rain and
  the wind, the channel and its setting, the files of the sounds. The look is judged on the
  snapshots of `e2e::weather`, the sounds on their spectrograms and waveforms.
- Clear weather draws and sounds exactly as before: no flash is 0 everywhere, and the loops are
  silent and paused without rain or wind.
- `WeatherShading` stays the one buffer of the weather; it changes only while a flash lasts.
- The existing sounds stay byte-identical: the script writes the new files only.
- `timeout 600 cargo test -p client` while working; `timeout 900 just check` at the end.

## Design

- **Strikes.** `LightningStrike { at, seed }` becomes a `Strike` message in the client
  (`world/lightning`), read by the lightning (bolt and flash) and by the sound (thunder).
  `Lightning` holds the strikes alive, their age, their distance and their bolt, if within the
  render distance.
- **The bolt** (`lightning/bolt.rs`, pure): from the seed (a small PRNG of our own, so that
  every build draws the same bolt), a main channel from a point of the cloud base (192), offset
  sideways, down to the top of the struck block, by midpoint displacement (fractal, sharp
  turns at every scale), then branches leaving it downward and outward, more from its upper
  part, each with sub-branches, thinner, dimmer and fading to their tips. Points carry a width
  and a brightness.
- **Strokes** (pure): two or three return strokes in ~0.5 s, from the seed: each a sharp rise
  and a quick decay, the first the brightest, a faint glow of the channel between them.
- **Drawing the bolt** (`lightning/render.rs`, `lightning.wgsl`): a pass of its own after the
  TAA and before the bloom, as the rain's (decision 38): the points in a storage buffer, each
  segment a camera-facing ribbon joined to the next by a miter, so the glow has no seam; the
  fragment draws a hot white-violet core and a soft violet glow, additive, in HDR bright enough
  to bloom; tested by hand against the depth of the view. Without HDR, the core saturates white
  and the glow carries the shape.
- **Flash.** `WeatherShading` grows the flash and where it comes from: the struck point, and
  the direction from the eye to the flash in the clouds. The overcast sky (dome, fog of the
  chunks, water, puddles, the horizon skirt, far clouds) brightens, most toward the strike; the
  clouds glow from within around it; the chunks get a light from the strike, strongest near it,
  through the light of the sky (so caves stay dark); the ambient light jumps, and with HDR the
  exposure; without the shaders, the colour behind the world and the fog brighten.
- **Thunder** (`audio/weather.rs`): delayed by distance / 340 s; a crack within 48 blocks, a
  rumble further; louder near; non-spatial in volume, panned by a spatial emitter kept 2 blocks
  from the ears toward the strike, inside the radius where Bevy does not attenuate.
- **Rain and wind.** Two loops of rain (outdoors, muffled indoors) and the wind loop, always
  playing while weather is heard, their volumes eased each frame: the rain by the level of the
  drops and the share of rain columns around, crossfaded by how open the player stands (rays at
  the height of the head toward open columns, from the occlusion map and the blocks), the
  muffled one quieter the higher the roof; the wind by the snow around and the storm.
- **Sounds** (`scripts/generate_sounds.py`): `weather/rain.ogg` and `weather/rain_indoor.ogg`
  (seamless loops), `weather/thunder_near1..3.ogg`, `weather/thunder_far1..3.ogg`.
- **Weather volume.** `Channel::Weather`, saved in `audio.ron` with a default, its slider in the
  Audio tab.

## Tasks

### Task 1: The Weather channel (`audio/settings.rs`)
- [x] Tests: the channel is scaled by the master volume, set and clamped, and an old file
  loads it at full volume. See them fail.
- [x] `Channel::Weather`, `AudioSettings::weather`; regenerate `menu_audio.png`. Commit
  `feat(client): a volume for the weather`.

### Task 2: The sounds of the weather (`scripts/generate_sounds.py`, `audio/sounds.rs`)
- [x] Rain loops, cracks and rumbles synthesised; spectrograms and seams checked; only the new
  files written.
- [x] Tests: every file of the new sounds exists; they play on the Weather channel. Commit
  `feat(client): the sounds of the rain and the thunder`.

### Task 3: The bolt and its strokes (`world/lightning/bolt.rs`, `mod.rs`)
- [x] Tests: the same seed gives the same bolt, another seed another; the main channel goes from
  the cloud base to the top of the struck block, never far sideways; branches leave the main
  channel and go down, thinner and dimmer; the strokes: none before or after, two or three
  peaks, the first at 1; a strike far off has no bolt and a weaker flash. See them fail.
- [x] The bolt, the strokes, `Lightning`, the message from the network. Commit
  `feat(client): lightning strikes from the clouds`.

### Task 4: The flash (`overcast.rs`, `sky.wgsl`, `clouds.wgsl`, `chunk.wgsl`, `shaders.rs`)
- [x] Tests: the shaders are told the flash and the strike; the ambient light and the exposure
  jump with it; the colour behind the world brightens without the shaders. See them fail.
- [x] The flash everywhere. Commit `feat(client): the flash lights the world, the sky and the
  clouds`.

### Task 5: The bolt drawn (`lightning/render.rs`, `lightning.wgsl`)
- [x] Tests: the points sent to the pass, their segments and vertices. Commit
  `feat(client): the bolt is drawn, blinding`.

### Task 6: Thunder, rain and wind (`audio/weather.rs`)
- [x] Tests: delay by distance, crack or rumble, volume falling with distance; how open the
  player stands (open field, closed room, under a pavilion, by a wall, in a cave); the
  crossfade and the volumes; underwater. See them fail.
- [x] The systems. Commit `feat(client): the thunder rolls after the flash and the rain is
  heard`.

### Task 7: Pictures (`e2e/weather.rs`)
- [x] A bolt 40–60 blocks off at its brightest stroke in High, Ultra and Off; a far strike; a
  strike over the lake with the wet ground; a bolt at night. Commit
  `test(client): pictures of the lightning`.
- [x] Look at every picture, boards, tune until beautiful; commit each tuning.

### Task 8: Closing
- [x] Decisions appended to the spec; `just check`; commit `docs: the thunderstorms and the
  sound of the weather (lot 5)`.
