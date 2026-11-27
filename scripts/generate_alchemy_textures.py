"""Makes the textures of alchemy from those of Pixel Perfection CE (CC BY-SA 4.0,
https://github.com/Athemis/PixelPerfectionCE, `master`), and draws the ones it has none of.

Taken as they are: items/GlassVial (`item/glass_bottle`), items/Alembic (`item/brewing_stand`), and
the icons of the effects hud/effect_healing (`mob_effect/instant_health`), effect_nightvision,
effect_waterbreathing, effect_swiftness (`speed`) and effect_strength.

Put together from its files, so under its license: each vial and potion is `item/potion` (the
splash ones `item/splash_potion`) with the inside of its flask filled with the colour of its
potion (`ItemId::potion_color` in `shared/src/world/alchemy.rs`, copied in `COLOURS`), shaded;
blocks/Alembic is `block/cauldron_side` with its gaps closed by `block/cauldron_inner`, and
blocks/AlembicTop `block/cauldron_top` over a violet brew; hud/effect_fireresistance is
`mob_effect/fire_resistance` turned golden, as hud/effect_burn is that icon as it is.

Drawn here (MIT): items/GnawerFang.

Requirements: Pillow.
Usage: python3 scripts/generate_alchemy_textures.py <the textures folder of Pixel Perfection CE>
"""

import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent / "data" / "graphics"
ITEMS = ROOT / "base_textures" / "items"
BLOCKS = ROOT / "base_textures" / "blocks"
HUD = ROOT / "hud"

# As `ItemId::potion_color`
COLOURS = {
    "WaterVial": (56, 92, 214),
    "NeutralBase": (150, 170, 210),
    "HealingPotion": (248, 36, 35),
    "NightVisionPotion": (31, 31, 161),
    "WaterBreathingPotion": (46, 82, 153),
    "FireResistancePotion": (228, 154, 58),
    "SwiftnessPotion": (124, 175, 198),
    "StrengthPotion": (147, 36, 35),
    "RegenerationPotion": (205, 92, 171),
    "SplashPoisonPotion": (78, 147, 49),
    "SplashSlownessPotion": (90, 108, 129),
    "SplashWeaknessPotion": (72, 77, 72),
}


def shade(colour, factor):
    return tuple(max(0, min(255, round(c * factor))) for c in colour) + (255,)


def inside(flask):
    """The clear pixels the outline of `flask` closes in: those no clear path joins to an edge."""
    size = flask.size[0]
    clear = {(x, y) for x in range(size) for y in range(size) if flask.getpixel((x, y))[3] < 128}
    outside = set()
    todo = [p for p in clear if p[0] in (0, size - 1) or p[1] in (0, size - 1)]
    while todo:
        p = todo.pop()
        if p in outside or p not in clear:
            continue
        outside.add(p)
        x, y = p
        todo += [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]
    return clear - outside


def filled(flask, colour):
    """`flask` with the inside of its body filled with `colour`, under the glass: darker at the
    bottom and on the right, lighter at the top left."""
    out = flask.copy()
    cells = inside(flask)
    # The body: below its neck, where the flask is at its widest
    widths = {}
    for x, y in cells:
        widths[y] = widths.get(y, 0) + 1
    widest = max(widths.values())
    top = min(y for y, w in widths.items() if w >= widest * 0.6)
    bottom = max(widths)
    left = min(x for x, _ in cells)
    right = max(x for x, _ in cells)
    liquid = Image.new("RGBA", flask.size, (0, 0, 0, 0))
    for x, y in cells:
        if y < top:
            continue
        depth = (y - top) / max(1, bottom - top)
        across = (x - left) / max(1, right - left)
        factor = 1.15 - 0.35 * depth - 0.2 * across
        liquid.putpixel((x, y), shade(colour, factor))
    liquid.alpha_composite(out)
    return liquid


def closed(side, inner):
    """`side` with its clear pixels taken from `inner`."""
    out = inner.copy()
    out.alpha_composite(side)
    return out


def golden(icon):
    """`icon` with its reds turned to gold."""
    out = icon.copy()
    for x in range(icon.size[0]):
        for y in range(icon.size[1]):
            r, g, b, a = icon.getpixel((x, y))
            if r > g + 30 and r > b + 30:
                out.putpixel((x, y), (r, min(255, round(g + (r - g) * 0.75)), b // 2, a))
    return out


FANG = [
    "................",
    "................",
    "...........##...",
    "..........#ww#..",
    ".........#wwl#..",
    "........#wwl#...",
    ".......#wwl#....",
    "......#wwwl#....",
    ".....#wwwl#.....",
    "....#wwwl#......",
    "...#rwwl#.......",
    "...#rrw#........",
    "..#rrr#.........",
    "...###..........",
    "................",
    "................",
]
FANG_COLOURS = {"#": (74, 62, 48, 255), "w": (236, 228, 204, 255), "l": (196, 184, 156, 255),
                "r": (150, 64, 58, 255)}


def fang():
    image = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    for y, row in enumerate(FANG):
        for x, cell in enumerate(row):
            if cell in FANG_COLOURS:
                image.putpixel((x, y), FANG_COLOURS[cell])
    return image


def main():
    pack = Path(sys.argv[1])
    load = lambda name: Image.open(pack / f"{name}.png").convert("RGBA")
    load("item/glass_bottle").save(ITEMS / "GlassVial.png")
    load("item/brewing_stand").save(ITEMS / "Alembic.png")
    for name, colour in COLOURS.items():
        flask = load("item/splash_potion" if name.startswith("Splash") else "item/potion")
        filled(flask, colour).save(ITEMS / f"{name}.png")
    inner = load("block/cauldron_inner")
    closed(load("block/cauldron_side"), inner).save(BLOCKS / "Alembic.png")
    brew = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    for x in range(16):
        for y in range(16):
            brew.putpixel((x, y), shade((122, 63, 168), 0.8 + 0.25 * (((x * 7 + y * 3) % 5) / 4)))
    closed(load("block/cauldron_top"), brew).save(BLOCKS / "AlembicTop.png")
    fang().save(ITEMS / "GnawerFang.png")
    for effect, icon in [("healing", "instant_health"), ("nightvision", "night_vision"),
                         ("waterbreathing", "water_breathing"), ("swiftness", "speed"),
                         ("strength", "strength")]:
        load(f"mob_effect/{icon}").save(HUD / f"effect_{effect}.png")
    golden(load("mob_effect/fire_resistance")).save(HUD / "effect_fireresistance.png")


if __name__ == "__main__":
    main()
