"""Draws the armour: the 16x16 icons of its pieces, the 64x64 layers worn on the model of a
player, the 9x9 shields of the armour bar and the outlines of the empty armour slots.

Every picture is drawn here from shapes and palettes of our own.

Usage: python3 scripts/generate_armour.py
"""

import struct
import zlib
from pathlib import Path

GRAPHICS = Path(__file__).resolve().parent.parent / "data" / "graphics"
ITEMS = GRAPHICS / "base_textures" / "items"
WORN = GRAPHICS / "armour"
HUD = GRAPHICS / "hud"

# Outline, dark, base, light, highlight, then the trim and its shade
PALETTES = {
    "Leather": ("#33200f", "#5d3a1e", "#80522b", "#99683a", "#b5834f", "#d8bb84", "#a88a55"),
    "Bronze": ("#3a2410", "#7a4b1e", "#a86d2c", "#c98d40", "#e9bb6e", "#4f8a72", "#376453"),
    "Iron": ("#2b2f36", "#6d747e", "#98a0aa", "#b9c0c8", "#e4e8ec", "#4a5b73", "#354356"),
    "Diamond": ("#123048", "#2c7f9e", "#48adc9", "#7fd3e4", "#d5f6fa", "#e3c56a", "#a9893a"),
}
KEYS = "odblhtT"

ICONS = {
    # A bowl with a crest, a brow band and cheek guards
    "Helmet": """
................
.......tt.......
.......tT.......
.....oottoo.....
....olhttbbo....
...olhlttbbdo...
...olhlttbbdo...
..olhllttbbbdo..
..ottttttttTTo..
..oTTTTTTTTTTo..
..obbo....obdo..
..obbo....obdo..
..obbdo..obbdo..
...oboo..oobo...
....o......o....
................
""",
    # Pauldrons, a collar, a breast with a ridge and a belt
    "Chestplate": """
................
..ooo......ooo..
.ohlbo.oo.obbdo.
olhlbboTToobbbdo
olllbbottobbbddo
obTbbblhllbbbTdo
.oToblhhlbbbdTo.
..o.olhllbbdo.o.
....olhllbbdo...
....olhllbbdo...
....oblllbbdo...
....obbbbbddo...
....ottttttTo...
....oTTooTTTo...
....oddbbddoo...
.....oooooo.....
""",
    # A belt with its buckle, two legs with knee plates
    "Leggings": """
................
...oooooooooo...
...ottthhtTTo...
...oTTThhTTTo...
...olhlbbbbdo...
...olhlbbbbdo...
...olhlbobbdo...
...olhbooobdo...
...olhbo.obdo...
...ohhlo.lhbo...
...otTTo.tTTo...
...olbbo.lbdo...
...olbdo.obdo...
...olbdo.obdo...
...oTTTo.TTTo...
....ooo..ooo....
""",
    # Two boots seen from the side, cuffed
    "Boots": """
................
................
................
.oooo....oooo...
.otTo....otTo...
.oTTo....oTTo...
.olbo....olbo...
.ohbo....ohbo...
.ohbdoo..ohbdoo.
.olbbbdo.olbbbdo
.olhlbdo.olhlbdo
.otttTTo.otttTTo
.ooooooo.ooooooo
................
................
................
""",
}

# The shield of the bar: its left half, with a glint, and its right one
BAR = """
ooooooooo
oLLLLmmmo
oLhLLmmmo
oLhLLmmmo
oLLLLmmmo
ooLLLmmoo
.ooLLmoo.
..ooLoo..
...ooo...
"""


def rgba(colour, alpha=255):
    return bytes.fromhex(colour[1:]) + bytes([alpha])


def write(path, width, pixels):
    height = len(pixels) // width
    raw = b"".join(
        b"\0" + b"".join(pixels[y * width + x] for x in range(width)) for y in range(height)
    )

    def chunk(kind, data):
        return (
            struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
        )

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def grid(art):
    rows = art.strip("\n").split("\n")
    assert all(len(row) == len(rows[0]) for row in rows), art
    return rows


CLEAR = bytes(4)


def icon(art, palette):
    colours = dict(zip(KEYS, map(rgba, palette)))
    return [colours.get(c, CLEAR) for row in grid(art) for c in row]


def outline(art):
    """The shape of a piece, faint, for its empty slot."""
    return [
        CLEAR if c == "." else rgba("#c8ccd4", 110 if c == "o" else 45)
        for row in grid(art)
        for c in row
    ]


def bar(filled):
    """The shield of the bar with its left, both its halves, or none filled."""
    empty, colours = rgba("#2a2d33"), {"o": rgba("#101216"), ".": CLEAR}
    full = {"L": rgba("#aab4c2"), "h": rgba("#f2f5f8"), "m": rgba("#7c889a")}
    shown = {"": "", "half": "Lh", "full": "Lhm"}[filled]
    return [
        colours[c] if c in colours else full[c] if c in shown else empty
        for row in grid(BAR)
        for c in row
    ]


# The worn layers are laid out as a skin (`client/src/player/model.rs`): the helmet, the
# chestplate and the boots where the skin has its first layer, the leggings where it has its
# second one. Each box is drawn face by face: (x, y, width, height).
def faces(x, y, w, h, d):
    return {
        "right": (x, y + d, d, h),
        "front": (x + d, y + d, w, h),
        "left": (x + d + w, y + d, d, h),
        "back": (x + 2 * d + w, y + d, w, h),
        "top": (x + d, y, w, d),
        "bottom": (x + d + w, y, w, d),
    }


HEAD, TORSO, WAIST = (0, 0, 8, 8, 8), (16, 16, 8, 12, 4), (16, 32, 8, 12, 4)
ARMS = [(40, 16, 4, 12, 4), (32, 48, 4, 12, 4)]
FEET = [(0, 16, 4, 12, 4), (16, 48, 4, 12, 4)]
LEGS = [(0, 32, 4, 12, 4), (0, 48, 4, 12, 4)]


def paint(canvas, box, colours, style):
    """Paints the pixels of the box for which `style(face, x, y, w, h)` names a colour."""
    for face, (left, top, w, h) in faces(*box).items():
        for y in range(h):
            for x in range(w):
                key = style(face, x, y, w, h)
                if key:
                    canvas[(top + y) * 64 + left + x] = colours[key]


def plate(x, y, w, h):
    """Metal lit from its top left, dark at its bottom and right edges."""
    if y == h - 1 or x == w - 1:
        return "d"
    if (x, y) in ((1, 1), (1, 2)):
        return "h"
    return "l" if y == 0 or x == 0 else "b"


def helmet(face, x, y, w, h):
    if face == "bottom":
        return None
    if face == "top":
        return "t" if x in (3, 4) else "l" if x < 3 else "b"
    if face == "front":
        if y == 3:
            return "t"
        if y < 3:
            return "t" if x in (3, 4) else plate(x, y, 8, 5)
        # Cheek guards and a nose guard
        if x in (0, 7) and y < 7:
            return "b" if y < 6 else "d"
        return "T" if x in (3, 4) and y == 4 else None
    if y == 3:
        return "t"
    if y > 6:
        return None
    if face == "back":
        return "t" if x in (3, 4) and y < 3 else plate(x, y, 8, 7)
    front_side = x if face == "right" else w - 1 - x
    if y > 4 and front_side < 2:
        return "b"
    return plate(x, y, w, 7) if y < 6 or front_side > 1 else None


def chest(face, x, y, w, h):
    if face == "bottom":
        return "d"
    if face == "top":
        return "T" if y in (1, 2) and x in (2, 3, 4, 5) else "l"
    if y in (10, 11):
        return "t" if y == 10 else "d"
    if face == "front":
        if y < 2 and x in (3, 4):
            return "T"
        # Two breast plates about a ridge, then bands over the belly
        if y < 6:
            return "d" if y == 5 else "h" if x in (1, 4) and y < 4 else "l" if x < 4 else "b"
        return "d" if y == 8 else "b" if x > 4 else "l"
    if face == "back":
        return "T" if x in (3, 4) else plate(x, y, w, 10)
    return "d" if y == 5 else "b"


def sleeve(face, x, y, w, h):
    # A pauldron, then the arm down to above the elbow
    if face == "top":
        return "l" if (x + y) % 3 else "h"
    if face == "bottom" or y > 6:
        return None
    if y == 3:
        return "t"
    if y < 3:
        return plate(x, y, w, 4)
    return "d" if y == 6 else "b" if x else "l"


def waist(face, x, y, w, h):
    if face in ("top", "bottom") or y < 8:
        return None
    if y == 8:
        return "h" if face == "front" and x in (3, 4) else "t"
    return "d" if y == 11 or x == w - 1 else "b"


def trousers(face, x, y, w, h):
    if face in ("top", "bottom") or y > 9:
        return None
    if y in (5, 6) and face == "front":
        return "T" if y == 6 else "t" if x in (0, 3) else "h"
    return "d" if y == 9 or x == w - 1 else "l" if x == 0 else "b"


def boot(face, x, y, w, h):
    if face == "top":
        return None
    if face == "bottom":
        return "o"
    if y < 7:
        return None
    if y == 7:
        return "t"
    if y == 11:
        return "T" if face == "front" else "d"
    return "h" if face == "front" and (x, y) == (1, 9) else "l" if x == 0 else "b"


def layer(palette):
    colours = dict(zip(KEYS, map(rgba, palette)))
    canvas = [CLEAR] * (64 * 64)
    paint(canvas, HEAD, colours, helmet)
    paint(canvas, TORSO, colours, chest)
    paint(canvas, WAIST, colours, waist)
    for boxes, style in ((ARMS, sleeve), (LEGS, trousers), (FEET, boot)):
        for box in boxes:
            paint(canvas, box, colours, style)
    return canvas


def main():
    for material, palette in PALETTES.items():
        for piece, art in ICONS.items():
            write(ITEMS / f"{material}{piece}.png", 16, icon(art, palette))
        write(WORN / f"{material}.png", 64, layer(palette))
    for piece, art in ICONS.items():
        write(HUD / f"slot_{piece.lower()}.png", 16, outline(art))
    for name, filled in (("empty", ""), ("half", "half"), ("full", "full")):
        write(HUD / f"armour_{name}.png", 9, bar(filled))


if __name__ == "__main__":
    main()
