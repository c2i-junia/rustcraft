"""Draws the textures of the furniture that the texture pack lacks, from the textures of the
game:

- the bedrolls of fifteen colours, the red one recoloured with the shades of each wool;
- the lanterns of sixteen colours, the light of the lantern tinted with each dye;
- the banners of sixteen colours, of their wool, a darker fringe at the bottom;
- the flower pot (terracotta round dirt) and the item frame (oak round leather);
- the canvases of the paintings, original pictures of one block and of two blocks;
- the icons of the tables, the chairs, the carpets, the banners and the wide painting.

The bookshelf, the book, and the icons of the flower pot, the item frame and the painting are
Pixel Perfection CE's, put in place as they are.

Usage: python3 scripts/generate_furniture.py (needs Pillow)
"""

import random
from pathlib import Path

from PIL import Image

TEXTURES = Path(__file__).resolve().parent.parent / "data" / "graphics" / "base_textures"
BLOCKS = TEXTURES / "blocks"
ITEMS = TEXTURES / "items"
SIZE = 16

WOODS = ["Oak", "Spruce", "Jungle", "Willow", "Acacia"]
COLORS = ["White", "Orange", "Magenta", "LightBlue", "Yellow", "Lime", "Pink", "Gray",
          "LightGray", "Cyan", "Purple", "Blue", "Brown", "Green", "Red", "Black"]
DYE_RGB = {
    "White": (0xF9, 0xFF, 0xFE), "Orange": (0xF9, 0x80, 0x1D), "Magenta": (0xC7, 0x4E, 0xBD),
    "LightBlue": (0x3A, 0xB3, 0xDA), "Yellow": (0xFE, 0xD8, 0x3D), "Lime": (0x80, 0xC7, 0x1F),
    "Pink": (0xF3, 0x8B, 0xAA), "Gray": (0x47, 0x4F, 0x52), "LightGray": (0x9D, 0x9D, 0x97),
    "Cyan": (0x16, 0x9C, 0x9C), "Purple": (0x89, 0x32, 0xB8), "Blue": (0x3C, 0x44, 0xAA),
    "Brown": (0x83, 0x54, 0x32), "Green": (0x5E, 0x7C, 0x16), "Red": (0xB0, 0x2E, 0x26),
    "Black": (0x1D, 0x1D, 0x21),
}


def load(path):
    return Image.open(path).convert("RGBA")


def wool(color):
    return load(BLOCKS / ("Wool.png" if color == "White" else f"{color}Wool.png"))


def luminance(pixel):
    r, g, b = pixel[:3]
    return 0.299 * r + 0.587 * g + 0.114 * b


def shade(pixel, by):
    return tuple(max(0, min(255, round(c * by))) for c in pixel[:3]) + (pixel[3],)


def recolour_where(image, keep, shades_of):
    """`image` with each pixel that `keep` takes replaced by the shade of `shades_of` of the same
    rank of luminance among them."""
    shades = sorted({p[:3] for p in shades_of.getdata() if p[3] > 0}, key=luminance)
    chosen = [p for p in image.getdata() if p[3] > 0 and keep(p)]
    levels = sorted({luminance(p) for p in chosen})
    out = image.copy()
    for y in range(image.height):
        for x in range(image.width):
            p = image.getpixel((x, y))
            if p[3] == 0 or not keep(p):
                continue
            rank = levels.index(luminance(p)) / max(1, len(levels) - 1)
            out.putpixel((x, y), shades[round(rank * (len(shades) - 1))] + (p[3],))
    return out


def is_red(p):
    return p[0] > p[1] + 40 and p[0] > p[2] + 40


def is_light(p):
    return p[0] > 200 and p[1] > 150 and p[2] < 160


def tint_light(image, rgb):
    """The light of a lantern in the colour `rgb`, kept bright."""
    out = image.copy()
    for y in range(image.height):
        for x in range(image.width):
            p = image.getpixel((x, y))
            if p[3] and is_light(p):
                bright = luminance(p) / 255
                mixed = [round((0.45 * c + 0.55 * 255) * bright) for c in rgb]
                out.putpixel((x, y), tuple(mixed) + (p[3],))
    return out


def banner_block(color):
    """The wool of the colour, its two lowest rows darker, as a fringe."""
    out = wool(color)
    for y in (14, 15):
        for x in range(SIZE):
            if y == 15 and x % 2:
                continue
            out.putpixel((x, y), shade(out.getpixel((x, y)), 0.72))
    return out


def banner_icon(color):
    """A banner on its pole: the bar across the top, the cloth below it."""
    cloth = banner_block(color)
    stick = load(BLOCKS / "OakPlanks.png")
    out = Image.new("RGBA", (SIZE, SIZE))
    for x in range(2, 14):
        out.putpixel((x, 1), shade(stick.getpixel((x, 1)), 0.8))
    for y in range(2, 15):
        for x in range(4, 12):
            out.putpixel((x, y), cloth.getpixel((x, y + 1)))
    return out


def carpet_icon(color):
    """A thin rug seen from the side and a little above."""
    texture = wool(color)
    out = Image.new("RGBA", (SIZE, SIZE))
    for y in range(9, 13):
        for x in range(SIZE):
            p = texture.getpixel((x, y))
            out.putpixel((x, y), shade(p, 1.0 if y < 11 else 0.75))
    return out


def table_icon(planks):
    out = Image.new("RGBA", (SIZE, SIZE))
    for y in range(SIZE):
        for x in range(SIZE):
            top = 3 <= y < 6
            leg = y >= 6 and (1 <= x < 3 or 13 <= x < 15)
            if top or leg:
                out.putpixel((x, y), shade(planks.getpixel((x, y)), 1.0 if top else 0.8))
    return out


def chair_icon(planks):
    out = Image.new("RGBA", (SIZE, SIZE))
    for y in range(SIZE):
        for x in range(SIZE):
            back = 10 <= x < 12 and 1 <= y < 9
            seat = 4 <= x < 12 and 8 <= y < 10
            leg = y >= 10 and (x in (4, 5) or x in (10, 11))
            if back or seat or leg:
                out.putpixel((x, y), shade(planks.getpixel((x, y)), 1.0 if seat else 0.85))
    return out


def flower_pot():
    """Terracotta on the sides (rows 10 to 15, columns 5 to 10, as the pot's box shows them),
    dirt in a rim of terracotta on top (columns and rows 5 to 10)."""
    clay = load(BLOCKS / "Terracotta.png")
    dirt = load(BLOCKS / "Dirt.png")
    out = clay.copy()
    for y in range(5, 11):
        for x in range(5, 11):
            rim = x in (5, 10) or y in (5, 10)
            out.putpixel((x, y), shade(clay.getpixel((x, y)), 1.1) if rim else shade(dirt.getpixel((x, y)), 0.8))
    for x in range(5, 11):
        out.putpixel((x, 10), shade(clay.getpixel((x, 10)), 1.15))
    return out


def item_frame():
    """Leather in a frame of oak, in the twelve pixels the frame's box shows (2 to 13)."""
    planks = load(BLOCKS / "OakPlanks.png")
    leather = load(ITEMS / "Leather.png")
    leather_shades = sorted({p[:3] for p in leather.getdata() if p[3] > 0}, key=luminance)
    rng = random.Random(7)
    out = Image.new("RGBA", (SIZE, SIZE))
    for y in range(SIZE):
        for x in range(SIZE):
            border = x <= 2 or x >= 13 or y <= 2 or y >= 13
            if border:
                out.putpixel((x, y), shade(planks.getpixel((x, y)), 0.9 if x + y > 15 else 1.05))
            else:
                mid = leather_shades[len(leather_shades) // 3:2 * len(leather_shades) // 3] or leather_shades
                out.putpixel((x, y), rng.choice(mid) + (255,))
    return out


# The paintings: each a picture of `width` blocks, drawn by a function of the pixel.
FRAME_DARK = (74, 48, 28)
FRAME_LIGHT = (122, 84, 50)


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def dawn(x, y, w, h, rng):
    if (x - 7) ** 2 + (y - 10) ** 2 <= 9 and y <= 10:
        return (255, 214, 120)
    hill = 10 + round(1.5 * __import__("math").sin(x * 0.7))
    if y > hill:
        return lerp((70, 52, 80), (40, 30, 50), (y - hill) / 4)
    return lerp((250, 150, 110), (120, 90, 160), 1 - y / 11)


def meadow(x, y, w, h, rng):
    if y >= 9:
        if rng.random() < 0.12:
            return rng.choice([(230, 60, 60), (250, 220, 70), (240, 240, 240), (200, 110, 220)])
        return lerp((110, 170, 70), (70, 120, 45), (y - 9) / 6)
    if 2 <= y <= 3 and 3 <= x <= 7 or y == 4 and 4 <= x <= 6:
        return (245, 245, 250)
    return lerp((140, 190, 240), (190, 220, 250), y / 9)


def harbour(x, y, w, h, rng):
    if 2 <= x <= 4 and 3 <= y <= 9:
        return (235, 235, 225) if y % 3 else (200, 60, 50)
    if 2 <= x <= 4 and y == 2:
        return (255, 230, 120)
    if 8 <= y <= 9 and 8 <= x <= 13 - (y - 8):
        return (110, 70, 40)
    if 3 <= y <= 7 and 10 <= x <= 10 + (y - 3) // 2:
        return (245, 245, 240)
    if y >= 9:
        return (40, 80, 140) if (x + y) % 4 else (90, 140, 200)
    return lerp((30, 40, 90), (90, 110, 170), y / 9)


def still_life(x, y, w, h, rng):
    if y >= 11:
        return lerp((120, 70, 40), (90, 50, 30), (y - 11) / 4)
    if 6 <= x <= 9 and 7 <= y <= 10:
        return (70, 110, 160) if x in (6, 9) else (90, 140, 190)
    flowers = {(5, 4): (230, 70, 70), (7, 3): (250, 220, 80), (9, 4): (230, 120, 200), (8, 5): (230, 70, 70)}
    if (x, y) in flowers:
        return flowers[(x, y)]
    if 7 <= x <= 8 and 4 <= y <= 6:
        return (60, 120, 50)
    if 11 <= x <= 12 and 9 <= y <= 10:
        return (220, 180, 60)
    return (60, 45, 50) if (x + 2 * y) % 7 else (70, 52, 58)


def valley(x, y, w, h, rng):
    import math
    peak = min(abs(x - 9), abs(x - 21) * 0.8) * 0.9 + 2
    if y >= 11:
        river = abs(x - 15 - (y - 11) * 2) <= 1
        return (80, 140, 200) if river else lerp((100, 160, 70), (60, 110, 40), (y - 11) / 4)
    if y >= peak:
        snow = y <= peak + 1.5
        return (240, 245, 250) if snow else lerp((110, 110, 125), (80, 85, 95), (y - peak) / 8)
    return lerp((120, 170, 230), (180, 210, 240), y / 10)


def seascape(x, y, w, h, rng):
    if (x - 22) ** 2 + (y - 5) ** 2 <= 5:
        return (255, 236, 150)
    if 7 <= y <= 8 and 5 <= x <= 10:
        return (70, 110, 60)
    if y >= 8:
        crest = (x + 3 * y) % 7 == 0
        return (220, 235, 245) if crest else lerp((50, 110, 170), (20, 60, 110), (y - 8) / 7)
    return lerp((250, 190, 140), (150, 190, 230), 1 - y / 8)


def forest(x, y, w, h, rng):
    for cx, top in [(3, 3), (8, 1), (13, 4), (18, 2), (23, 3), (27, 1)]:
        if y >= top and abs(x - cx) <= (y - top) // 2 and y <= 12:
            return lerp((40, 90, 50), (25, 60, 35), (y - top) / 10)
        if abs(x - cx) == 0 and 12 < y <= 13:
            return (90, 60, 35)
    if y >= 13:
        return (80, 120, 50)
    return lerp((170, 210, 190), (220, 235, 220), y / 13)


def painting(draw, width, seed):
    """The picture drawn by `draw` on a canvas of `width` blocks in a frame of dark wood: a
    texture per block, from the left."""
    rng = random.Random(seed)
    w, h = SIZE * width, SIZE
    image = Image.new("RGBA", (w, h))
    for y in range(h):
        for x in range(w):
            if x == 0 or y == 0 or x == w - 1 or y == h - 1:
                image.putpixel((x, y), (FRAME_LIGHT if x + y < w else FRAME_DARK) + (255,))
            else:
                image.putpixel((x, y), tuple(draw(x - 1, y - 1, w - 2, h - 2, rng)) + (255,))
    return [image.crop((SIZE * i, 0, SIZE * (i + 1), SIZE)) for i in range(width)]


SMALL = {"DawnPainting": dawn, "MeadowPainting": meadow, "HarbourPainting": harbour,
         "StillLifePainting": still_life}
WIDE = {"ValleyPainting": valley, "SeascapePainting": seascape, "ForestPainting": forest}


def main():
    bedroll, bedroll_icon = load(BLOCKS / "Bedroll.png"), load(ITEMS / "Bedroll.png")
    lantern = load(BLOCKS / "Lantern.png")
    for color in COLORS:
        if color != "Red":
            recolour_where(bedroll, is_red, wool(color)).save(BLOCKS / f"{color}Bedroll.png")
            recolour_where(bedroll_icon, is_red, wool(color)).save(ITEMS / f"{color}Bedroll.png")
        tint_light(lantern, DYE_RGB[color]).save(BLOCKS / f"{color}Lantern.png")
        banner_block(color).save(BLOCKS / f"{color}Banner.png")
        banner_icon(color).save(ITEMS / f"{color}Banner.png")
        carpet_icon(color).save(ITEMS / f"{color}Carpet.png")
    for wood in WOODS:
        planks = load(BLOCKS / f"{wood}Planks.png")
        table_icon(planks).save(ITEMS / f"{wood}Table.png")
        chair_icon(planks).save(ITEMS / f"{wood}Chair.png")
    flower_pot().save(BLOCKS / "FlowerPot.png")
    item_frame().save(BLOCKS / "ItemFrame.png")
    for seed, (name, draw) in enumerate(SMALL.items()):
        painting(draw, 1, seed)[0].save(BLOCKS / f"{name}.png")
    for seed, (name, draw) in enumerate(WIDE.items()):
        left, right = painting(draw, 2, 10 + seed)
        left.save(BLOCKS / f"{name}.png")
        right.save(BLOCKS / f"{name}Right.png")
        if name == "ValleyPainting":
            # the icon: the whole picture, squeezed into the middle rows
            whole = Image.new("RGBA", (2 * SIZE, SIZE))
            whole.paste(left, (0, 0))
            whole.paste(right, (SIZE, 0))
            icon = Image.new("RGBA", (SIZE, SIZE))
            icon.paste(whole.resize((SIZE, SIZE // 2), Image.NEAREST), (0, 4))
            icon.save(ITEMS / "WidePainting.png")


if __name__ == "__main__":
    main()
