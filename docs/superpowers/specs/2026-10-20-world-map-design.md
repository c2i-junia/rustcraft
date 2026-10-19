# World map — design

Asked on 2026-10-20: "an option to show the map large: pressing M would show the map over the
whole screen". The user left every choice to Claude, favouring the UI and the UX; the choices
below are Claude's.

## What the player sees

- `M` opens the world map during gameplay, over the whole window; `M` or Escape closes it. It is
  a dialog like the inventory: the cursor is free, the game keys wait, the server goes on.
- The whole loaded world, north up, on the `SLOT` background, which is also the colour of what is
  not loaded, so the edge of the world melts into it. Same colours, relief and water as the
  minimap.
- At the top-left, a panel with the title "Map" and the coordinates of the player; "N" at the
  top; the key hints at the bottom: Drag Move, Wheel Zoom, Space Centre, Esc Close.
- The arrow of the player, turned by the camera, the other players as light dots and the mobs as
  orange dots, all at a fixed size whatever the zoom.
- It opens centred on the player and follows them. A drag moves it (and stops the following,
  within the loaded world); Space centres it again. The wheel zooms around the cursor, `=` and
  `-` around the centre: 1, 2, 4 or 8 px a block, 2 when first opened, kept for the session.
- The HUD (hotbar, minimap, reticle, chat lines) hides under it, as under the pause menu.

## Decisions

1. `M` opens the world map; the minimap is shown or hidden from the Interface tab only. The
   action `ToggleMinimap` becomes `OpenMap` (with a serde alias, so a saved binding carries
   over). The zoom actions are named "Zoom in / out the map" and serve both maps.
2. No panning beyond the loaded world, and nothing kept of where the player has been (as for the
   minimap): the view is clamped to the image of the loaded world.
3. One image at one block a pixel, scaled by the UI: dragging and zooming are free. It is drawn
   tile by tile (`compose_world`), not pixel by pixel, and at most every 250 ms while chunks come
   in, only while the map is open.
4. The players beyond the loaded world are not held at the edge, unlike the minimap: the map
   already shows all that is loaded.

## Out of scope

Waypoints, a saved explored map, a cave mode, clicking the map to do anything.
