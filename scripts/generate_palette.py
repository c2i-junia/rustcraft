"""Draws the textures of the palette that the texture pack lacks, from the textures of the game:

- the door and the trapdoor of willow, the oak ones recoloured with the shades of willow planks;
- the polished and the chiseled blocks of stone, limestone, slate and gloomrock;
- the icons of the slabs, stairs, low walls and fence gates, cut from the texture of what they
  are made of, and of the stained glass panes, cut from their glass.

Usage: python3 scripts/generate_palette.py (needs Pillow)
"""

from pathlib import Path

from PIL import Image

TEXTURES = Path(__file__).resolve().parent.parent / "data" / "graphics" / "base_textures"
BLOCKS = TEXTURES / "blocks"
ITEMS = TEXTURES / "items"
SIZE = 16

WOODS = ["Oak", "Spruce", "Jungle", "Willow", "Acacia"]
# the name of each rock in the variants, and its block
ROCKS = {
    "Stone": "Stone",
    "Cobblestone": "Cobblestone",
    "HewnStone": "HewnStone",
    "Limestone": "Limestone",
    "Slate": "Slate",
    "Brick": "BrickWall",
    "Gloomrock": "Gloomrock",
    "CobbledGloomrock": "CobbledGloomrock",
}
# their slabs and stairs had icons already
DRAWN_BEFORE = {"HewnStone", "Brick"}
POLISHABLE = ["Stone", "Limestone", "Slate", "Gloomrock"]
COLORS = ["White", "Orange", "Magenta", "LightBlue", "Yellow", "Lime", "Pink", "Gray",
          "LightGray", "Cyan", "Purple", "Blue", "Brown", "Green", "Red", "Black"]


def load(path):
    return Image.open(path).convert("RGBA")


def luminance(pixel):
    r, g, b = pixel[:3]
    return 0.299 * r + 0.587 * g + 0.114 * b


def recolour(image, shades_of):
    """`image` with each opaque pixel replaced by the shade of `shades_of` of the same rank of
    luminance: the drawing of one wood in the colours of another."""
    shades = sorted({p[:3] for p in shades_of.getdata() if p[3] > 0}, key=luminance)
    pixels = [p for p in image.getdata() if p[3] > 0]
    levels = sorted({luminance(p) for p in pixels})
    out = Image.new("RGBA", image.size)
    for y in range(image.height):
        for x in range(image.width):
            p = image.getpixel((x, y))
            if p[3] == 0:
                continue
            rank = levels.index(luminance(p)) / max(1, len(levels) - 1)
            out.putpixel((x, y), shades[round(rank * (len(shades) - 1))] + (p[3],))
    return out


def shade(pixel, by):
    return tuple(max(0, min(255, round(c * by))) for c in pixel[:3]) + (pixel[3],)


def polished(rock):
    """The rock smoothed: each pixel the mean of its row of three, a light edge at the top and
    the left and a dark one at the bottom and the right."""
    image = load(BLOCKS / f"{rock}.png")
    out = Image.new("RGBA", (SIZE, SIZE))
    for y in range(SIZE):
        for x in range(SIZE):
            row = [image.getpixel(((x + dx) % SIZE, y)) for dx in (-1, 0, 1)]
            mean = tuple(round(sum(p[c] for p in row) / 3) for c in range(3)) + (255,)
            if x == 0 or y == 0:
                mean = shade(mean, 1.18)
            elif x == SIZE - 1 or y == SIZE - 1:
                mean = shade(mean, 0.72)
            out.putpixel((x, y), mean)
    return out


def chiseled(rock):
    """The polished rock with a frame cut two pixels in, and a diamond carved in its middle."""
    out = polished(rock)
    for y in range(SIZE):
        for x in range(SIZE):
            p = out.getpixel((x, y))
            on_frame = (x in (2, 13) and 2 <= y <= 13) or (y in (2, 13) and 2 <= x <= 13)
            in_diamond = abs(x - 7.5) + abs(y - 7.5) <= 4.5
            edge = abs(x - 7.5) + abs(y - 7.5) > 3.5
            if on_frame:
                out.putpixel((x, y), shade(p, 0.68))
            elif in_diamond and edge:
                out.putpixel((x, y), shade(p, 0.78 if x + y > 15 else 1.15))
    return out


def silhouette(texture, cells):
    """An icon showing `texture` where `cells` (rows of `#` and `.`) is filled."""
    out = Image.new("RGBA", (SIZE, SIZE))
    for y, row in enumerate(cells):
        for x, cell in enumerate(row):
            if cell == "#":
                out.putpixel((x, y), texture.getpixel((x, y)))
    return out


def rows(fill):
    return ["".join("#" if fill(x, y) else "." for x in range(SIZE)) for y in range(SIZE)]


SLAB = rows(lambda x, y: y >= 8)
STAIRS = rows(lambda x, y: y >= 8 or x >= 8)
LOW_WALL = rows(lambda x, y: 4 <= x < 12 or 5 <= y < 14)
FENCE_GATE = rows(lambda x, y: x < 2 or x >= 14 or (2 <= y < 5) or (9 <= y < 12))
PANE = rows(lambda x, y: 4 <= x < 12)


def main():
    planks = load(BLOCKS / "WillowPlanks.png")
    recolour(load(BLOCKS / "Door.png"), planks).save(BLOCKS / "WillowDoor.png")
    recolour(load(BLOCKS / "DoorTop.png"), planks).save(BLOCKS / "WillowDoorTop.png")
    recolour(load(BLOCKS / "OakTrapdoor.png"), planks).save(BLOCKS / "WillowTrapdoor.png")
    recolour(load(ITEMS / "Door.png"), planks).save(ITEMS / "WillowDoor.png")
    for rock in POLISHABLE:
        polished(rock).save(BLOCKS / f"Polished{rock}.png")
        chiseled(rock).save(BLOCKS / f"Chiseled{rock}.png")
    for wood in WOODS:
        texture = load(BLOCKS / f"{wood}Planks.png")
        silhouette(texture, SLAB).save(ITEMS / f"{wood}Slab.png")
        silhouette(texture, STAIRS).save(ITEMS / f"{wood}Stairs.png")
        silhouette(texture, FENCE_GATE).save(ITEMS / f"{wood}FenceGate.png")
    for rock, block in ROCKS.items():
        texture = load(BLOCKS / f"{block}.png")
        if rock not in DRAWN_BEFORE:
            silhouette(texture, SLAB).save(ITEMS / f"{rock}Slab.png")
            silhouette(texture, STAIRS).save(ITEMS / f"{rock}Stairs.png")
        silhouette(texture, LOW_WALL).save(ITEMS / f"{rock}LowWall.png")
    for color in COLORS:
        glass = load(BLOCKS / f"{color}StainedGlass.png")
        silhouette(glass, PANE).save(ITEMS / f"{color}StainedGlassPane.png")


if __name__ == "__main__":
    main()
