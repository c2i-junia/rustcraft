"""Draws RustCraft Accents, the pixel font of the letters French needs that the RustCraft pixel
font (data/fonts/RustCraftRegular-Bmg3.otf) lacks: é è ï œ, the accented capitals, the curly
quotes, the ellipsis, the degree sign and the no-break spaces. The client gives it as the
fallback of the pixel font, glyph by glyph (client/src/ui/fonts.rs).

Every glyph is drawn here, pixel by pixel, on the grid of the pixel font: a pixel is 100 units of
an em of 1000, the baseline under row 0, capitals 7 pixels high, small letters 5, a column of
space after each glyph. Small letters take their accent on row 6; capitals keep their full height
and take it on rows 8 and 9, inside the ascent of the line (1000). The font is ours, under the
MIT license of the project, and does not copy the outlines of the pixel font.

Needs fontTools (pip install fonttools).
Usage: python3 -I scripts/generate_accent_font.py
"""

from pathlib import Path

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen

OUT = Path(__file__).resolve().parent.parent / "data" / "fonts" / "RustCraftAccents.ttf"
PIXEL = 100

# The bodies, top row first; the bottom row of the body sits on the baseline unless a glyph says
# otherwise. `#` is ink.
E_SMALL = [".###.", "#...#", "#####", "#....", ".####"]
I_SMALL = [".#", ".#", ".#", ".#", ".#"]
A_CAP = [".###.", "#...#", "#####", "#...#", "#...#", "#...#", "#...#"]
E_CAP = ["#####", "#....", "###..", "#....", "#....", "#....", "#####"]
I_CAP = ["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"]
O_CAP = [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]
U_CAP = ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."]
Y_CAP = ["#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..", "..#.."]

ACUTE = ["...##"]
GRAVE = ["##..."]
CIRCUMFLEX = [".###.", "#...#"]
DIAERESIS = ["#...#"]


def small(accent, body):
    """A small letter of five rows with `accent` on row 6, or on rows 5 and 6 when it has two."""
    gap = [] if len(accent) == 2 else ["." * len(body[0])]
    return accent + gap + body


def capital(accent, body):
    """A capital of seven rows with `accent` on row 8, or on rows 8 and 9, and row 7 empty."""
    width = len(body[0])
    accent = [row[:width] if len(row) >= width else row.center(width, ".") for row in accent]
    return accent + ["." * width] + body


def narrow(accent):
    """An accent five pixels wide over a glyph three wide: its middle."""
    return [row[1:4] for row in accent]


# char: (rows, the row of the bottom one, advance in pixels or None for the width + 1)
GLYPHS = {
    "é": (small(ACUTE, E_SMALL), 0, None),
    "è": (small(GRAVE, E_SMALL), 0, None),
    "ï": (small(["#.#"], [".#.", ".#.", ".#.", ".#.", ".#."]), 0, None),
    "œ": ([".##.##.", "#..#..#", "#..####", "#..#...", ".##.###"], 0, None),
    "Œ": ([".######", "#..#...", "#..#...", "#..####", "#..#...", "#..#...", ".######"], 0, None),
    "À": (capital(GRAVE, A_CAP), 0, None),
    "Â": (capital(CIRCUMFLEX, A_CAP), 0, None),
    "É": (capital(ACUTE, E_CAP), 0, None),
    "È": (capital(GRAVE, E_CAP), 0, None),
    "Ê": (capital(CIRCUMFLEX, E_CAP), 0, None),
    "Ë": (capital(DIAERESIS, E_CAP), 0, None),
    "Î": (capital([".#.", "#.#"], I_CAP), 0, None),
    "Ï": (capital(["#.#"], I_CAP), 0, None),
    "Ô": (capital(CIRCUMFLEX, O_CAP), 0, None),
    "Ù": (capital(GRAVE, U_CAP), 0, None),
    "Û": (capital(CIRCUMFLEX, U_CAP), 0, None),
    "Ÿ": (capital(DIAERESIS, Y_CAP), 0, None),
    "’": ([".#", ".#", "#."], 4, None),
    "‘": ([".#", "#.", "#."], 4, None),
    "”": ([".#.#", ".#.#", "#.#."], 4, None),
    "“": ([".#.#", "#.#.", "#.#."], 4, None),
    "…": (["#.#.#"], 0, None),
    "°": ([".#.", "#.#", ".#."], 4, None),
    " ": ([], 0, 4),
    " ": ([], 0, 2),
}


def draw(rows, bottom):
    """The outline of `rows`: a rectangle per run of ink in a row."""
    pen = TTGlyphPen(None)
    for index, row in enumerate(rows):
        y = (bottom + len(rows) - 1 - index) * PIXEL
        x = 0
        while x < len(row):
            if row[x] != "#":
                x += 1
                continue
            end = x
            while end < len(row) and row[end] == "#":
                end += 1
            pen.moveTo((x * PIXEL, y))
            pen.lineTo((x * PIXEL, y + PIXEL))
            pen.lineTo((end * PIXEL, y + PIXEL))
            pen.lineTo((end * PIXEL, y))
            pen.closePath()
            x = end
    return pen.glyph()


def main():
    order = [".notdef"] + [f"uni{ord(c):04X}" for c in GLYPHS]
    builder = FontBuilder(1000, isTTF=True)
    builder.setupGlyphOrder(order)
    builder.setupCharacterMap({ord(c): f"uni{ord(c):04X}" for c in GLYPHS})
    glyphs = {".notdef": draw([], 0)}
    metrics = {".notdef": (400, 0)}
    for char, (rows, bottom, advance) in GLYPHS.items():
        name = f"uni{ord(char):04X}"
        glyphs[name] = draw(rows, bottom)
        width = advance if advance is not None else max(len(r) for r in rows) + 1
        metrics[name] = (width * PIXEL, 0)
    builder.setupGlyf(glyphs)
    builder.setupHorizontalMetrics(metrics)
    builder.setupHorizontalHeader(ascent=1000, descent=-200)
    builder.setupNameTable(
        {
            "familyName": "RustCraft Accents",
            "styleName": "Regular",
            "uniqueFontIdentifier": "RustCraftAccents-Regular",
            "fullName": "RustCraft Accents Regular",
            "psName": "RustCraftAccents-Regular",
            "version": "Version 1.000",
            "licenseDescription": "MIT License, as the code of Rustcraft",
        }
    )
    builder.setupOS2(
        sTypoAscender=800,
        sTypoDescender=-200,
        sTypoLineGap=0,
        usWinAscent=1000,
        usWinDescent=200,
        sxHeight=500,
        sCapHeight=700,
    )
    builder.setupPost(underlinePosition=-75, underlineThickness=50)
    builder.save(OUT)
    print(f"Wrote {OUT} ({len(GLYPHS)} glyphs)")


if __name__ == "__main__":
    main()
