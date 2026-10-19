# Minimap — design

The proposal the user accepted on 2026-10-19 ("je te laisse trancher"): a top-down map of the
loaded world in the top-right corner of the game. The open questions were settled with the
recommendations of the proposal; they are recorded under "Decisions".

## What the player sees

- A square map, 128 × 128 logical px plus the bevelled frame of the theme, in the top-right
  corner, under which the FPS counter and then the toasts are placed.
- North (-Z) is up, east (+X) right. The map does not turn: an arrow at its centre, in `ACCENT`,
  turns with the camera.
- Each pixel is the top visible block of a column of the world. Glass, flowers and tall grass
  are seen through; leaves are shown. A block higher than the one to its north is lighter, a
  lower one darker. Water is darkened by its depth, up to 16 blocks.
- What is not loaded is `SLOT`.
- Three zoom levels: 1, 2 and 4 blocks per pixel (`=` and `-`). `M` shows or hides the map.
  The Interface tab of the settings has Minimap: Off / On, saved in `interface.ron`.
- The coordinates of the player, as whole blocks, under the map.
- The other players are light dots, held at the edge of the map when they are beyond it; the
  mobs are orange dots, shown only within the map.
- The map hides under the pause menu, the in-game settings and the quitting card, like the
  hotbar. Its room stays taken, so the FPS counter does not jump.

## How it works

All on the client, from `ClientWorldMap`: no message, no protocol change.

- `colors.rs`: `MinimapColors`, the colour of each `BlockId`: the mean of the opaque pixels of
  the texture of its top face (`VoxelShape::create_from_block`), times the tint of that face.
  Computed when the game is entered, the textures being loaded by then.
- `surface.rs`: `MinimapSurface`, one `ColumnTile` of 16 × 16 cells per column of chunks
  `(x, z)`, each cell holding the top visible block, its height and the depth of the water over
  it. A tile keeps the heights of its loaded chunks, scanned from the top.
  `ChunkLoaded` / `ChunkUnloaded` (written by `network/world.rs`) mark a column dirty; at most
  `COLUMNS_PER_FRAME` dirty columns are scanned per frame, the nearest first. A `BlockChanged`
  rescans its own cell only.
- `compose.rs`: a pure function from the surface, the colours, the centre and the zoom to the
  RGBA pixels, written into the `Image` of the map's `ImageNode` when the player's block, the
  zoom or the surface changed.
- `markers.rs`: the arrow (a small pixel-art image turned by `UiTransform`), the dots and the
  coordinates are UI nodes over the image, so turning the arrow never redraws the map.
- The top-right corner is one column (`TopRightColumn`): the minimap, then the FPS counter. The
  toast stack keeps below it in the game (`ToastsBelow`).

## Decisions

1. Square, bevelled like the rest of the HUD (round fits neither the pixel style nor the slots).
2. North up and a turning arrow; a turning map would need a round frame and blurs the pixels.
3. Underground, the map shows the top loaded block of each column: no cave mode.
4. Only what is loaded: no explored map kept in memory or on disk.
5. The other players are shown: the client already knows and draws them.

## Out of scope

A full-screen map, an explored map saved per world, waypoints, a cave mode, a turning map.
