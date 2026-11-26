# Where the assets come from

An audit before the first release, of 25.11.2026. "Verified" is what was read or compared; anything else
is marked as uncertain. It is not legal advice, and `LICENSE.txt` remains the license of the project.

Names are those under `data/graphics/`, without `base_textures/` and `.png`. Of the 495 PNG files:

| Origin | Files | State |
|---|---|---|
| A. Identical to Pixel Perfection CE | 170 | Verified: CC BY-SA 4.0 by that repository |
| B. Identical to Nova_Wostra's pack only | 71 | **Uncertain: to be cleared** |
| C. Derived from a pack (`blocks/Cactus`, `colormap/grass`, ...) | 8 | **Uncertain: as their pack** |
| D. Were identical to a file of Minecraft | 3 | Replaced, drawn by `scripts/generate_default.py`, MIT |
| E. Drawn by scripts of this repository | 149 | Claimed by scripts and commits, MIT |
| F. Said to be drawn for Rustcraft, no script | 77 | Claimed by commits, MIT |
| G. Lettered faces (`Back`, `Front`, ...) | 12 | No source given; plain letters |
| H. Without any source | 5 | **Uncertain: to be cleared or replaced** |

## How it was checked

The decoded pixels of every file were compared with the `block`, `item`, `gui`, `colormap` and
`environment` textures (the first frame of the animated ones included) of:

- [Pixel Perfection, Chorus Edit](https://github.com/NovaWostra/Pixel-Perfection-Chorus-Eddit), commit
  `3ce5c70`: **no license file**, GitHub detects none, and its `pack.txt` holds only credits: XSSheep,
  Nova_Wostra, freejusticehere, HexaBlu and others.
- [Pixel Perfection CE](https://github.com/Athemis/PixelPerfectionCE): `LICENSE.md` is CC BY-SA 4.0,
  added on 15.05.2020. Its README says that XSSheep published the original under a Creative Commons
  license that allows alteration and redistribution.
- the textures of Minecraft 1.21.1, to find copies of them. Minecraft's textures are Mojang's and
  may not be redistributed here.

XSSheep's own page ([Planet Minecraft](https://www.planetminecraft.com/texture-pack/131pixel-perfection/))
refused automated reading: a search result quotes its license as CC BY-SA 4.0, as does
[ContentDB](https://content.luanti.org/packages/sofar/pixelperfection). **The owner should read it.**

Nova_Wostra's [Pixel Perfection Legacy](https://modrinth.com/resourcepack/pixel-perfection-legacy) says
"Pixel Perfection by XSSheep is licensed under a Creative Commons Attribution-Share Alike 4.0
International License", and its license field reads `CC-BY-4.0` with the URL of CC BY-SA 4.0.
[CurseForge](https://www.curseforge.com/minecraft/texture-packs/pixel-perfection-legacy) shows "Custom
License" and quotes "Feel free to use these textures in any mixpack, project, game or whatever you
want (paid or unpaid)". These pages are about the same author's other edition, not about the
repository, whose textures of their own are therefore not cleared by any license read here.

The textures of a pack came with the commits that name Pixel Perfection, from `989f3c44` to
`6a4f3422` (`git log -i --grep='pixel perfection'`), or with `4f123aa8`, `9637839a`, `df6cccf5` and
`c1c2fa58`, which name no source: for these, the comparison is the only evidence.

## A. Identical to Pixel Perfection CE (170)

blocks/CarrotSprout, blocks/Cocoa, blocks/Dandelion, blocks/DestroyStage0, blocks/DestroyStage1,
blocks/DestroyStage2, blocks/DestroyStage3, blocks/DestroyStage4, blocks/DestroyStage5,
blocks/DestroyStage6, blocks/DestroyStage7, blocks/DestroyStage8, blocks/DestroyStage9,
blocks/DiamondBlock, blocks/GrassSideOverlay, blocks/GrassTop, blocks/Gravel, blocks/Ice,
blocks/JungleLeaves, blocks/JungleLog, blocks/JungleLogTop, blocks/OakLeaves, blocks/OakLog,
blocks/OakLogTop, blocks/OakPlanks, blocks/Poppy, blocks/PotatoSprout, blocks/RipeCarrots,
blocks/RipePotatoes, blocks/Sand, blocks/Snow, blocks/SpruceLeaves, blocks/SpruceLog,
blocks/SpruceLogTop, blocks/TallCarrots, blocks/TallGrass, blocks/TallPotatoes, blocks/Vine,
blocks/YoungCarrots, blocks/YoungPotatoes, items/BakedPotato, items/Charcoal, items/CocoaBeans,
items/DiamondAxe, items/Feather, items/FlintAndSteel, items/Grass, items/IronAxe, items/IronHoe,
items/IronShovel, items/RawBeef, items/RawPork, items/RawRabbit, items/RoastBeef,
items/RoastRabbit, items/StoneAxe, items/StoneHoe, items/StoneShovel, items/WoodenAxe,
items/WoodenHoe, items/WoodenShovel

The palette (wool, stained glass, terracotta, concrete and its powder of each colour, the dyes,
the doors and trapdoors), taken from the `master` branch of Pixel Perfection CE (CC BY-SA 4.0): where the Chorus Edit differs, its CE file was preferred, its license being the one verified.

blocks/AcaciaDoor, blocks/AcaciaDoorTop, blocks/AcaciaTrapdoor, blocks/BlackConcrete,
blocks/BlackConcretePowder, blocks/BlackStainedGlass, blocks/BlackTerracotta, blocks/BlackWool,
blocks/BlueConcrete, blocks/BlueConcretePowder, blocks/BlueStainedGlass, blocks/BlueTerracotta,
blocks/BlueWool, blocks/BrownConcrete, blocks/BrownConcretePowder, blocks/BrownStainedGlass,
blocks/BrownTerracotta, blocks/BrownWool, blocks/CyanConcrete, blocks/CyanConcretePowder,
blocks/CyanStainedGlass, blocks/CyanTerracotta, blocks/CyanWool, blocks/GrayConcrete,
blocks/GrayConcretePowder, blocks/GrayStainedGlass, blocks/GrayTerracotta, blocks/GrayWool,
blocks/GreenConcrete, blocks/GreenConcretePowder, blocks/GreenStainedGlass, blocks/GreenTerracotta,
blocks/GreenWool, blocks/JungleDoor, blocks/JungleDoorTop, blocks/JungleTrapdoor,
blocks/LightBlueConcrete, blocks/LightBlueConcretePowder, blocks/LightBlueStainedGlass,
blocks/LightBlueTerracotta, blocks/LightBlueWool, blocks/LightGrayConcrete,
blocks/LightGrayConcretePowder, blocks/LightGrayStainedGlass, blocks/LightGrayTerracotta,
blocks/LightGrayWool, blocks/LimeConcrete, blocks/LimeConcretePowder, blocks/LimeStainedGlass,
blocks/LimeTerracotta, blocks/LimeWool, blocks/MagentaConcrete, blocks/MagentaConcretePowder,
blocks/MagentaStainedGlass, blocks/MagentaTerracotta, blocks/MagentaWool, blocks/OakTrapdoor,
blocks/OrangeConcrete, blocks/OrangeConcretePowder, blocks/OrangeStainedGlass,
blocks/OrangeTerracotta, blocks/OrangeWool, blocks/PinkConcrete, blocks/PinkConcretePowder,
blocks/PinkStainedGlass, blocks/PinkTerracotta, blocks/PinkWool, blocks/PurpleConcrete,
blocks/PurpleConcretePowder, blocks/PurpleStainedGlass, blocks/PurpleTerracotta, blocks/PurpleWool,
blocks/RedConcrete, blocks/RedConcretePowder, blocks/RedStainedGlass, blocks/RedTerracotta,
blocks/RedWool, blocks/SpruceDoor, blocks/SpruceDoorTop, blocks/SpruceTrapdoor, blocks/Terracotta,
blocks/WhiteConcrete, blocks/WhiteConcretePowder, blocks/WhiteStainedGlass, blocks/WhiteTerracotta,
blocks/YellowConcrete, blocks/YellowConcretePowder, blocks/YellowStainedGlass,
blocks/YellowTerracotta, blocks/YellowWool, items/AcaciaDoor, items/BlackDye, items/BlueDye,
items/BrownDye, items/CyanDye, items/GrayDye, items/GreenDye, items/JungleDoor, items/LightBlueDye,
items/LightGrayDye, items/LimeDye, items/MagentaDye, items/OrangeDye, items/PinkDye,
items/PurpleDye, items/RedDye, items/SpruceDoor, items/WhiteDye, items/YellowDye

## B. Identical to Nova_Wostra's pack, not to CE (71)

Either XSSheep's, changed by CE, or Nova_Wostra's: not told apart. To clear them, ask Nova_Wostra for
a license on the repository, or take XSSheep's original file where there is one.

blocks/Bedrock, blocks/CoalOre, blocks/Cobblestone, blocks/CraftingTable,
blocks/CraftingTableBottom, blocks/CraftingTableSide, blocks/CraftingTableSide1,
blocks/CraftingTableSide2, blocks/CraftingTableTop, blocks/DiamondOre, blocks/Dirt, blocks/Fern,
blocks/Fire, blocks/Furnace, blocks/FurnaceBottom, blocks/FurnaceSide, blocks/FurnaceTop,
blocks/Glass, blocks/GoldBlock, blocks/GoldOre, blocks/Grass, blocks/IronBlock, blocks/IronOre,
blocks/JunglePlanks, blocks/Lava, blocks/LitFurnace, blocks/Obsidian, blocks/Stone, blocks/Torch,
blocks/Water, items/Bucket, items/Carrot, items/Coal, items/CraftingTable, items/Diamond,
items/DiamondHoe, items/DiamondPickaxe, items/DiamondShovel, items/DiamondSword, items/GoldIngot,
items/GoldenAxe, items/GoldenHoe, items/GoldenPickaxe, items/GoldenShovel, items/GoldenSword,
items/IronIngot, items/IronPickaxe, items/IronSword, items/LavaBucket, items/Leather,
items/Potato, items/RawChicken, items/RoastChicken, items/RoastPork, items/Snowball,
items/StonePickaxe, items/StoneSword, items/Torch, items/WaterBucket, items/WoodenPickaxe,
items/WoodenSword, hud/air, hud/air_bursting, hud/food_empty, hud/food_full, hud/food_half,
hud/furnace_arrow, hud/furnace_flame, hud/heart_container, hud/heart_full, hud/heart_half

## C. Derived (8)

`blocks/Cactus` and `blocks/CactusTop` share 224 and 196 of their 256 pixels with Nova_Wostra's
`cactus_side` and `cactus_top` (commit `c7dada88` gives no source). The others are said to be from that
pack by `docs/ressources.txt`: the chest cut from its sheet, the colour maps smoothed
(`scripts/smooth_colormap.py`), and the silhouettes of the furnace's flame and arrow.

blocks/Cactus, blocks/CactusTop, blocks/Chest, blocks/ChestTop, colormap/foliage, colormap/grass,
hud/furnace_arrow_empty, hud/furnace_flame_empty

## D. Were identical to Minecraft's `debug.png` (3)

Shown when a texture is missing; drawn again by `scripts/generate_default.py`.

blocks/_Default, custom_textures/blocks/_Default, custom_textures/items/_Default

## E. Drawn by scripts of this repository

Not run again to compare their output.

- `scripts/generate_armour.py` (27): armour/Bronze, armour/Diamond, armour/Iron, armour/Leather,
  items/BronzeBoots, items/BronzeChestplate, items/BronzeHelmet, items/BronzeLeggings,
  items/DiamondBoots, items/DiamondChestplate, items/DiamondHelmet, items/DiamondLeggings,
  items/IronBoots, items/IronChestplate, items/IronHelmet, items/IronLeggings, items/LeatherBoots,
  items/LeatherChestplate, items/LeatherHelmet, items/LeatherLeggings, hud/armour_empty,
  hud/armour_full, hud/armour_half, hud/slot_boots, hud/slot_chestplate, hud/slot_helmet,
  hud/slot_leggings
- `scripts/generate_boss.py` (18): blocks/EmberHeart, items/DepthsCompass,
  items/DepthsCompass_Draw1, items/DepthsCompass_Draw10, items/DepthsCompass_Draw11,
  items/DepthsCompass_Draw12, items/DepthsCompass_Draw13, items/DepthsCompass_Draw14,
  items/DepthsCompass_Draw15, items/DepthsCompass_Draw16, items/DepthsCompass_Draw2,
  items/DepthsCompass_Draw3, items/DepthsCompass_Draw4, items/DepthsCompass_Draw5,
  items/DepthsCompass_Draw6, items/DepthsCompass_Draw7, items/DepthsCompass_Draw8,
  items/DepthsCompass_Draw9
- `scripts/generate_engraving_table.py` (2): blocks/EngravingTable, blocks/EngravingTableTop
- `scripts/generate_bow.py` (5): items/Arrow, items/Bow, items/Bow_Draw1, items/Bow_Draw2,
  items/Bow_Draw3
- `scripts/generate_combat_gear.py` (4, after the audit): items/Bolt, items/Mace, items/Shield,
  items/ThrowingAxe
- `scripts/generate_spawn_eggs.py` (20): items/BrambleSpawnEgg, items/CartographerSpawnEgg,
  items/CatSpawnEgg, items/ChickenSpawnEgg, items/CinderlingSpawnEgg, items/CowSpawnEgg,
  items/DrownerSpawnEgg, items/FarmerSpawnEgg, items/FoxSpawnEgg, items/GnawerSpawnEgg,
  items/GoatSpawnEgg, items/GustOwlSpawnEgg, items/HorseSpawnEgg, items/PigSpawnEgg,
  items/RabbitSpawnEgg, items/RollerSpawnEgg, items/SheepSpawnEgg, items/SmithSpawnEgg,
  items/StumplingSpawnEgg, items/WolfSpawnEgg
- `scripts/generate_palette.py` (63): the willow door and trapdoor, recoloured from the oak ones
  (blocks/OakTrapdoor is from CE, the change is told here), the polished and chiseled rocks, and
  the icons of the slabs, stairs, low walls, fence gates and stained glass panes, cut from the
  textures of their blocks: blocks/ChiseledGloomrock, blocks/ChiseledLimestone, blocks/ChiseledSlate, blocks/ChiseledStone,
  blocks/PolishedGloomrock, blocks/PolishedLimestone, blocks/PolishedSlate, blocks/PolishedStone,
  blocks/WillowDoor, blocks/WillowDoorTop, blocks/WillowTrapdoor, items/AcaciaFenceGate,
  items/AcaciaSlab, items/AcaciaStairs, items/BlackStainedGlassPane, items/BlueStainedGlassPane,
  items/BrickLowWall, items/BrownStainedGlassPane, items/CobbledGloomrockLowWall,
  items/CobbledGloomrockSlab, items/CobbledGloomrockStairs, items/CobblestoneLowWall,
  items/CobblestoneSlab, items/CobblestoneStairs, items/CyanStainedGlassPane, items/GloomrockLowWall,
  items/GloomrockSlab, items/GloomrockStairs, items/GrayStainedGlassPane, items/GreenStainedGlassPane,
  items/HewnStoneLowWall, items/JungleFenceGate, items/JungleSlab, items/JungleStairs,
  items/LightBlueStainedGlassPane, items/LightGrayStainedGlassPane, items/LimeStainedGlassPane,
  items/LimestoneLowWall, items/LimestoneSlab, items/LimestoneStairs, items/MagentaStainedGlassPane,
  items/OakFenceGate, items/OakSlab, items/OakStairs, items/OrangeStainedGlassPane,
  items/PinkStainedGlassPane, items/PurpleStainedGlassPane, items/RedStainedGlassPane,
  items/SlateLowWall, items/SlateSlab, items/SlateStairs, items/SpruceFenceGate, items/SpruceSlab,
  items/SpruceStairs, items/StoneLowWall, items/StoneSlab, items/StoneStairs,
  items/WhiteStainedGlassPane, items/WillowDoor, items/WillowFenceGate, items/WillowSlab,
  items/WillowStairs, items/YellowStainedGlassPane
- `scripts/generate_trade_icons.py` (2): items/Coin, hud/directions
- `scripts/generate_skins.py` (12): skins/Baker, skins/Courier, skins/Fisher, skins/Gardener,
  skins/Guard, skins/Healer, skins/Miner, skins/Ranger, skins/Sailor, skins/Scholar, skins/Smith,
  skins/Wanderer

## F. Said to be drawn for Rustcraft, without a script (77)

Commit `da9220ee` and `docs/ressources.txt` say so; `items/PortalGun` by commit `07581651`. None is
identical to a texture compared, or shares 30 % of its pixels with one.

blocks/AcaciaLeaves, blocks/AcaciaLog, blocks/AcaciaLogTop, blocks/AcaciaPlanks,
blocks/AcaciaSapling, blocks/Barrel, blocks/BarrelTop, blocks/Bedroll, blocks/BerryBush,
blocks/BrickWall, blocks/BronzeBlock, blocks/CampfireFlame, blocks/Clay,
blocks/CoalBlock, blocks/CopperBlock, blocks/CopperOre, blocks/Door, blocks/DoorTop,
blocks/FullRainCisternTop, blocks/Heather, blocks/HewnStone,
blocks/JungleSapling, blocks/Ladder, blocks/Lantern, blocks/Limestone, blocks/Mushroom,
blocks/OakSapling, blocks/RainCistern, blocks/RainCisternTop, blocks/RammedEarth, blocks/Reed,
blocks/RipeBerryBush, blocks/RipeRye, blocks/RyeSprout, blocks/Slate, blocks/SpruceSapling,
blocks/Thatch, blocks/TilledSoil, blocks/TinBlock, blocks/TinOre, blocks/WillowLeaves,
blocks/WillowLog, blocks/WillowLogTop, blocks/WillowPlanks, blocks/WillowSapling,
blocks/Wool, blocks/YoungRye, items/Bedroll, items/Berries, items/BrickSlab,
items/BrickStairs, items/BronzeAxe, items/BronzeHoe, items/BronzeIngot, items/BronzePickaxe,
items/BronzeShovel, items/BronzeSword, items/Campfire, items/CopperIngot, items/Door, items/Fence,
items/Fiber, items/FiredBrick, items/Flint, items/GlassPane, items/HewnStoneSlab,
items/HewnStoneStairs, items/RawMutton, items/RoastMushroom, items/RoastMutton, items/Rope,
items/Rye, items/RyeBread, items/Sign, items/Spear, items/TinIngot,
items/PortalGun

## G. Lettered faces (12)

From `4f123aa8`: a letter on a flat colour. No source was given.

blocks/Back, blocks/Down, blocks/Front, blocks/Left, blocks/Right, blocks/Top,
custom_textures/blocks/Back, custom_textures/blocks/Down, custom_textures/blocks/Front,
custom_textures/blocks/Left, custom_textures/blocks/Right, custom_textures/blocks/Top

## H. Without any source (5)

- `title`: `docs/ressources.txt` names https://textcraft.net for the logo. Its usage terms say its
  images may be used in personal or commercial projects without permission. Which font and texture
  it used, and whether the lettering is too near Minecraft's logo, is unknown.
- `background`: commit `04038a7e`, no source; not compared (160x96).
- `blocks/SprucePlanks`, `items/Stick`: commit `40efc081`, no source; unlike any texture compared.
- `blocks/Moss`: commit `4f123aa8`, no source; unlike any texture compared.

## Fonts

| File | Source | State |
|---|---|---|
| `fonts/RustCraftRegular-Bmg3.otf` | "Minecraft Font" by JDGraphics, [FontSpace](https://www.fontspace.com/minecraft-font-f28180) | FontSpace says "Public Domain"; the file itself says "Copyright 2017 by Jacob Debono" and "Free for Commercial Use". **The two differ: uncertain.** |
| `fonts/FiraMono-Medium.ttf` | Fira Mono 3.206, by Carrois and Edenspiekermann for Mozilla | Verified in the file: SIL Open Font License 1.1 or later. **`LICENSE.txt` is wrong here: it says that the fonts are in the public domain.** The text of the license is not shipped. |

## Sounds, music and models

- The 223 files of `data/sounds/` and the 8 of `data/music/`: synthesised by `scripts/generate_sounds.py`,
  `scripts/generate_feature_sounds.py` (the 52 of the armour, the bow, the experience, the engraving
  table, the anvil, the trade, the villagers, the breeding, the progress, the melee, the mounts, the
  shield and the crossbow) and `scripts/generate_music.py`, as commits `86973646`, `52050487`, `2396d533`, `75bd05c9` and others
  and `docs/ressources.txt` say. Not run again to compare their output.
- The 29 models of `data/models/animated/`: built by `scripts/generate_{animals,sheep,hostiles,villagers,boat}.py`.
  Not run again either.

## Added after the audit, from Nova_Wostra's pack (10)

As B: taken from the repository at the time of the shield and the crossbow, not cleared by a license.
items/Crossbow (`item/crossbow_standby`), items/Crossbow_Draw1 (`crossbow_pulling_0`),
items/Crossbow_Draw2 (`crossbow_pulling_2`), items/Crossbow_Draw3 (`crossbow_arrow`),
items/MilkBucket (`milk_bucket`), hud/effect_poison, hud/effect_slowness, hud/effect_weakness,
hud/effect_regeneration (`mob_effect/`), hud/effect_burn (`mob_effect/fire_resistance`).

## Added after the audit, from Pixel Perfection CE (21)

The maps, from the `master` branch of Pixel Perfection CE (CC BY-SA 4.0): the paper, the empty
map, the compass and its sixteen pictures (every other one of the pack's 32, `compass_16` ahead).
Two are put together from two of its files, and so are under its license: the map drawn
(`filled_map` under `filled_map_markings`, tinted brown) and the front of the frame of a map
(`item_frame`, the item, over our `blocks/OakPlanks`).

blocks/MapFrame, items/Compass, items/Compass_Draw1, items/Compass_Draw10, items/Compass_Draw11,
items/Compass_Draw12, items/Compass_Draw13, items/Compass_Draw14, items/Compass_Draw15,
items/Compass_Draw16, items/Compass_Draw2, items/Compass_Draw3, items/Compass_Draw4,
items/Compass_Draw5, items/Compass_Draw6, items/Compass_Draw7, items/Compass_Draw8,
items/Compass_Draw9, items/EmptyMap, items/FilledMap, items/Paper

## To do before the release

1. Read XSSheep's page, and settle the 79 files of B and C with Nova_Wostra or with XSSheep's originals.
2. Replace the 3 copies of `debug.png`.
3. Find the author of, or replace, the 5 files of H, and have the authors confirm F and G.
4. Correct the fonts in `LICENSE.txt`, and ship the text of the Open Font License.
5. CC BY-SA 4.0 asks for the changes to be told: section C does it.
