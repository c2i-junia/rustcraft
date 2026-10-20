#!/usr/bin/env python3
"""Draws the files dumped by `server/examples/worldgen_dump.rs`, to judge the world generation.

    worldgen_preview.py map   <dump> <out.png>   top view: shaded relief coloured by top block
    worldgen_preview.py biome <dump> <out.png>   top view coloured by biome
    worldgen_preview.py height <dump> <out.png>  top view coloured by height, sea in blue
    worldgen_preview.py view  <dump> <out.png>   oblique view of the map, from the south
    worldgen_preview.py slice <dump> <out.png>   vertical slice, one pixel per block
    worldgen_preview.py layer <dump> <out.png>   horizontal layer, one pixel per block

`map` and `biome` also print the height statistics and the share of each biome.
Needs Pillow and NumPy.
"""

import sys

import numpy as np
from PIL import Image

# In the order of the variants of BlockId (shared/src/world/blocks.rs)
BLOCKS = [
    ("Dirt", (134, 96, 67)),
    ("Debug", (255, 0, 255)),
    ("Grass", (95, 159, 53)),
    ("Stone", (125, 125, 125)),
    ("OakLog", (102, 81, 50)),
    ("OakPlanks", (162, 130, 78)),
    ("OakLeaves", (48, 105, 30)),
    ("Sand", (219, 207, 163)),
    ("Cactus", (88, 140, 45)),
    ("Ice", (160, 190, 245)),
    ("Glass", (200, 230, 240)),
    ("Bedrock", (40, 40, 40)),
    ("Dandelion", (95, 159, 53)),
    ("Poppy", (95, 159, 53)),
    ("TallGrass", (95, 159, 53)),
    ("Cobblestone", (110, 110, 110)),
    ("Snow", (245, 250, 252)),
    ("SpruceLeaves", (40, 80, 50)),
    ("SpruceLog", (70, 50, 30)),
    ("Water", (52, 92, 196)),
    ("CraftingTable", (150, 100, 60)),
    ("SprucePlanks", (110, 80, 50)),
    ("Furnace", (90, 90, 90)),
    ("Chest", (160, 110, 40)),
    ("CoalOre", (20, 20, 20)),
    ("IronOre", (216, 160, 120)),
    ("GoldOre", (250, 210, 40)),
    ("Gravel", (176, 146, 120)),
]
AIR = 255
WATER = 19
ICE = 9

# In the order of the variants of BiomeType (shared/src/world/data.rs)
BIOMES = [
    ("Plains", (140, 190, 90)),
    ("Forest", (40, 120, 50)),
    ("MediumMountain", (150, 140, 120)),
    ("HighMountainGrass", (200, 200, 200)),
    ("Desert", (230, 210, 140)),
    ("IcePlain", (200, 230, 250)),
    ("FlowerPlains", (200, 180, 90)),
    ("ShallowOcean", (90, 160, 220)),
    ("Ocean", (50, 100, 200)),
    ("DeepOcean", (20, 50, 140)),
]


def palette(table):
    colours = np.zeros((256, 3), np.float32)
    for i, (_, colour) in enumerate(table):
        colours[i] = colour
    return colours


def read(path):
    raw = np.fromfile(path, np.uint8)
    width, height = raw[:8].view("<u4")
    return int(width), int(height), raw[8:]


def read_map(path):
    width, height, data = read(path)
    record = np.dtype([("top", "<i2"), ("block", "u1"), ("floor", "<i2"), ("biome", "u1")])
    columns = data.view(record).reshape(height, width)
    return columns["top"].astype(np.float32), columns["block"], columns["floor"].astype(np.float32), columns["biome"]


def relief(heights):
    """Light from the north-west: slopes facing it are brighter."""
    dz, dx = np.gradient(heights)
    shade = 1.0 + 0.12 * (-dx - dz)
    return np.clip(shade, 0.45, 1.35)


# Height bands of the `height` view: (from height, colour)
HEIGHT_BANDS = [
    (0, (60, 110, 60)), (70, (110, 150, 70)), (84, (170, 170, 90)), (100, (160, 120, 80)),
    (120, (130, 110, 110)), (140, (235, 235, 240)),
]


def height_map(block, floor):
    colours = np.zeros(floor.shape + (3,), np.float32)
    for start, colour in HEIGHT_BANDS:
        colours[floor >= start] = colour
    colours[block == WATER] = (52, 92, 196)
    colours *= relief(floor)[..., None]
    return np.clip(colours, 0, 255).astype(np.uint8)


def coloured_map(top, block, floor):
    colours = palette(BLOCKS)[block]
    water = block == WATER
    depth = np.clip((top - floor) / 12.0, 0, 1)[..., None]
    colours = np.where(water[..., None], colours * (1.15 - 0.55 * depth), colours)
    colours *= relief(np.where(water, top, floor))[..., None]
    # brighter with altitude, so that the plains and the peaks read apart
    colours *= (0.85 + 0.3 * np.clip((floor - 40) / 120, 0, 1))[..., None]
    return np.clip(colours, 0, 255).astype(np.uint8)


def stats(top, block, floor, biome):
    land = (block != WATER) & (block != ICE)
    print(f"floor height: min {floor.min():.0f}, p10 {np.percentile(floor, 10):.0f}, "
          f"median {np.median(floor):.0f}, p90 {np.percentile(floor, 90):.0f}, "
          f"p99 {np.percentile(floor, 99):.0f}, max {floor.max():.0f}")
    print(f"water {100 * (~land).mean():.1f}% (frozen {100 * (block == ICE).mean():.1f}%), snow {100 * (block == 16).mean():.1f}%, "
          f"bare stone {100 * (block == 3).mean():.1f}%")
    steps = np.abs(np.diff(floor, axis=1))
    print(f"steps between neighbours: >2 {100 * (steps > 2).mean():.2f}%, "
          f">4 {100 * (steps > 4).mean():.2f}%, max {steps.max():.0f}")
    shares = ", ".join(f"{name} {100 * (biome == i).mean():.1f}%"
                       for i, (name, _) in enumerate(BIOMES) if (biome == i).any())
    print(f"biomes: {shares}")
    # the width of the biome a column stands in, along x, averaged over the columns: thin
    # bands between two biomes weigh as little as the area they cover
    runs = []
    for row in biome:
        edges = np.flatnonzero(np.diff(row.astype(np.int16))) + 1
        runs.extend(np.diff(np.concatenate(([0], edges, [row.size]))))
    runs = np.array(runs, np.float64)
    print(f"biome width seen from a column, along x: {(runs ** 2).sum() / runs.sum():.0f} blocks")


def oblique(top, block, floor, scale=1.0):
    """The map seen from the south, 30 degrees up: one column of pixels per x, painted from
    the nearest row of the map to the farthest."""
    colours = coloured_map(top, block, floor).astype(np.float32)
    height, width = top.shape
    lift = 1.2 * scale
    depth = 0.5
    image_height = int(height * depth + 200 * lift) + 10
    image = np.zeros((image_height, width, 3), np.uint8)
    image[:] = (180, 205, 230)
    ybuffer = np.full(width, image_height, np.int32)
    base = image_height - 5
    for z in range(height - 1, -1, -1):
        screen = (base - (height - 1 - z) * depth - top[z] * lift).astype(np.int32)
        screen = np.clip(screen, 0, image_height)
        for x in np.nonzero(screen < ybuffer)[0]:
            image[screen[x]:ybuffer[x], x] = colours[z, x]
            ybuffer[x] = screen[x]
    return image


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    mode, path, out = sys.argv[1:]
    if mode in ("map", "biome", "view", "height"):
        top, block, floor, biome = read_map(path)
        if mode == "map":
            image = coloured_map(top, block, floor)
        elif mode == "height":
            image = height_map(block, floor)
        elif mode == "biome":
            image = (palette(BIOMES)[biome] * relief(floor)[..., None]).clip(0, 255).astype(np.uint8)
        else:
            image = oblique(top, block, floor)
        if mode != "view":
            stats(top, block, floor, biome)
    elif mode in ("slice", "layer"):
        width, height, data = read(path)
        cells = data.reshape(height, width)
        colours = palette(BLOCKS)
        colours[AIR] = (15, 15, 22) if mode == "slice" else (70, 110, 170)
        image = colours[cells].astype(np.uint8)
        found = {BLOCKS[i][0]: int((cells == i).sum()) for i in (24, 25, 26)}
        print(f"air {100 * (cells == AIR).mean():.1f}%, ores {found}")
    else:
        sys.exit(__doc__)
    Image.fromarray(image).save(out)


if __name__ == "__main__":
    main()
