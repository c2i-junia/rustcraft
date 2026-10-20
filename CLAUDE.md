# Notes for Claude

## Textures

Before drawing a texture for a new block or item, look for it in the Pixel Perfection resource
pack (XSSheep's, kept up by Nova_Wostra): https://github.com/NovaWostra/Pixel-Perfection-Chorus-Eddit

- Item textures are in `assets/minecraft/textures/item/`, block textures in
  `assets/minecraft/textures/block/`, named like Minecraft's ids: `wooden_pickaxe.png`,
  `oak_planks.png`. A file is fetched from
  `https://raw.githubusercontent.com/NovaWostra/Pixel-Perfection-Chorus-Eddit/main/<path>`.
- Here they go in `data/graphics/base_textures/items/` and `.../blocks/`, named after the
  `ItemId` or `BlockId` variant: `wooden_pickaxe.png` becomes `WoodenPickaxe.png`. An item
  with no texture in `items/` shows the block texture of the same name, so a block item needs
  one only when its icon differs.
- Check that the texture is 16x16, like ours; an animated one is a vertical strip of frames, of
  which we take the first.
- A block Minecraft draws as a model of its own, like the chest, has no block texture: its
  faces are cut from its sheet in `assets/minecraft/textures/entity/` (64x64 for the chest,
  each part upside down since Minecraft 1.15) and grown to 16x16 by repeating their edges.
- A block with a texture per face follows the pack's model in `assets/minecraft/models/block/`:
  north is our front (-Z), then south, west and east are our back, left and right.
- The repository has no license file, only the credits in its `pack.txt`. Say which textures
  came from it in the commit message, and credit the pack in `docs/ressources.txt`.
- Draw a texture only when the pack has none for it.
