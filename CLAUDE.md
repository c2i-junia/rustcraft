# Notes for Claude

## Textures

Before drawing a texture for a new block or item, look for it in the Pixel Perfection resource
pack (XSSheep's, kept up by Nova_Wostra): https://github.com/NovaWostra/Pixel-Perfection-Chorus-Eddit

- Item textures are in `assets/minecraft/textures/item/`, block textures in
  `assets/minecraft/textures/block/`, named like Minecraft's ids: `wooden_pickaxe.png`,
  `oak_planks.png`. A file is fetched from
  `https://raw.githubusercontent.com/NovaWostra/Pixel-Perfection-Chorus-Eddit/main/<path>`.
- Here they go in `data/graphics/base_textures/items/` and `.../blocks/`, named after the
  `ItemId` or `BlockId` variant: `wooden_pickaxe.png` becomes `WoodenPickaxe.png`.
- Check that the texture is 16x16, like ours; an animated one is a vertical strip of frames, of
  which we take the first.
- The repository has no license file, only the credits in its `pack.txt`. Say which textures
  came from it in the commit message, and credit the pack in `docs/ressources.txt`.
- Draw a texture only when the pack has none for it.
