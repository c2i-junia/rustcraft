#!/usr/bin/env python3
"""Makes the pictures of the fishing, the cooking and the farm, 16x16, from Pixel Perfection CE
(CC BY-SA 4.0, https://github.com/Athemis/PixelPerfectionCE): taken as they are where the pack
has the thing (the rod, the egg, the apple, the bowl, the soups, the stew), recoloured where ours
is a thing of its own (the five fish from its cod, salmon, tropical fish and pufferfish, the cake
and the pies, the compost from its bone meal, the apple tree from its oak), and drawn here, from
pixel art of our own, where it has nothing near (the fried egg).

Requirements: curl and ImageMagick (`magick`).
Usage: python3 scripts/generate_farm_items.py
"""

import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ITEMS = ROOT / "data" / "graphics" / "base_textures" / "items"
BLOCKS = ROOT / "data" / "graphics" / "base_textures" / "blocks"
PACK = "https://raw.githubusercontent.com/Athemis/PixelPerfectionCE/master/assets/minecraft/textures/"

# (our picture, the pack's picture, ImageMagick's recolouring)
TAKEN = [
    ("FishingRod", "item/fishing_rod", []),
    ("Egg", "item/egg", []),
    ("Apple", "item/apple", []),
    ("Bowl", "item/bowl", []),
    ("MushroomSoup", "item/mushroom_stew", []),
    ("Stew", "item/rabbit_stew", []),
    ("BerrySalad", "item/beetroot_soup", []),
    ("FishChowder", "item/suspicious_stew", []),
    # The fish: a silvery roach, a green and gold perch, a long green pike, a dark eel, a
    # glowing cyan glowfin
    ("RawRoach", "item/cod", ["-modulate", "112,35,100"]),
    ("RawPerch", "item/tropical_fish", ["-modulate", "95,85,125"]),
    ("RawPike", "item/salmon", ["-modulate", "95,70,150"]),
    ("RawEel", "item/cod", ["-modulate", "62,70,85"]),
    ("RawGlowfin", "item/pufferfish", ["-modulate", "115,140,155"]),
    ("GrilledRoach", "item/cooked_cod", []),
    ("GrilledPerch", "item/cooked_cod", ["-modulate", "95,120,96"]),
    ("GrilledPike", "item/cooked_salmon", []),
    ("GrilledEel", "item/cooked_salmon", ["-modulate", "72,90,100"]),
    ("GrilledGlowfin", "item/cooked_cod", ["-modulate", "118,85,100"]),
    # The cakes and the pies, raw and baked
    ("CakeBatter", "item/mushroom_stew", ["-modulate", "135,35,108"]),
    ("CocoaCake", "item/cake", ["-channel", "RGB", "-colorspace", "gray", "+level-colors", "#2b160a,#c08a5a", "+channel"]),
    ("RawPie", "item/pumpkin_pie", ["-modulate", "118,40,105"]),
    ("ApplePie", "item/pumpkin_pie", ["-modulate", "104,115,92"]),
    ("Compost", "item/bone_meal", ["-channel", "RGB", "-colorspace", "gray", "+level-colors", "#24170c,#8a6a43", "+channel"]),
]

# A fried egg: the white spread, the yolk in the middle
FRIED_EGG = """
................
................
.....wwwww......
...wwwwwwwww....
..wwwwwwwwwwwo..
..wwwwwyywwwwo..
.wwwwwyYYywwwo..
.wwwwwyYYyywwwo.
.wwwwwwyyywwwwo.
..wwwwwwwwwwwwo.
..owwwwwwwwwwo..
...oowwwwwwoo...
.....oooooo.....
................
................
................
"""
FRIED_PALETTE = {"w": "#f4f1e6", "o": "#cfc7b0", "y": "#f2a81d", "Y": "#ffd24a"}

# The apples on the leaves and the sapling: (x, y, colour)
APPLES_ON_LEAVES = [(3, 2), (11, 4), (6, 9), (13, 12), (1, 13), (9, 14)]
APPLES_ON_SAPLING = [(5, 4), (10, 6)]


def magick(*args):
    subprocess.run(["magick", *map(str, args)], check=True)


def fetch(name, into):
    path = into / (name.replace("/", "_") + ".png")
    if not path.exists():
        subprocess.run(["curl", "-sfL", "-o", str(path), PACK + name + ".png"], check=True)
    return path


def draw(art, palette, path):
    rows = [row for row in art.strip("\n").split("\n")]
    args = ["-size", "16x16", "xc:none"]
    for y, row in enumerate(rows):
        for x, c in enumerate(row):
            if c in palette:
                args += ["-fill", palette[c], "-draw", f"point {x},{y}"]
    magick(*args, path)


def apples(source, spots, path):
    """`source` with a red apple of two pixels, lit on its top, on each of `spots`."""
    args = [source]
    for x, y in spots:
        args += ["-fill", "#c0262d", "-draw", f"point {x},{y + 1}", "-fill", "#e8484a",
                 "-draw", f"point {x},{y}", "-fill", "#8e1a20", "-draw", f"point {x + 1},{y + 1}"]
    magick(*args, path)


def main():
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        for ours, theirs, recolour in TAKEN:
            magick(fetch(theirs, tmp), *recolour, "-strip", ITEMS / f"{ours}.png")
        draw(FRIED_EGG, FRIED_PALETTE, ITEMS / "FriedEgg.png")
        # The leaves of the pack are grey, tinted by the biome: these keep their own green, as
        # the apples on them would take the tint
        green = tmp / "apple_leaves.png"
        magick(fetch("block/oak_leaves", tmp), "-channel", "RGB", "+level-colors",
               "#1f4a16,#7dbf4a", "+channel", green)
        apples(green, APPLES_ON_LEAVES, BLOCKS / "AppleLeaves.png")
        apples(fetch("block/oak_sapling", tmp), APPLES_ON_SAPLING, BLOCKS / "AppleSapling.png")
    print("wrote", len(TAKEN) + 3, "pictures")


if __name__ == "__main__":
    main()
