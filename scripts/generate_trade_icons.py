"""Draws the 16x16 icons of the trades: the coin, a square bronze piece with a hole stamped with
four rays, and the directions a cartographer sells, a strip of parchment with a dotted way.

Usage: python3 scripts/generate_trade_icons.py
"""

import struct
import zlib
from pathlib import Path

GRAPHICS = Path(__file__).resolve().parent.parent / "data" / "graphics"
COLOURS = {
    ".": "00000000", "o": "5a2f14ff", "d": "8f5423ff", "m": "c0782fff", "l": "e09c48ff", "h": "f6cf86ff",
    "p": "d9c496ff", "q": "b89b66ff", "w": "efe2bdff", "e": "6b5230ff", "r": "a8342aff", "b": "2f5f7aff",
}
COIN = """
................
................
....oooooooo....
...ohhhhhhhlo...
..ohlllllllmdo..
..ohlmmhhmmmdo..
..ohlmooooomdo..
..ohlho..odhdo..
..ohlho..odhdo..
..ohlmoooodmdo..
..ohlmmddmmmdo..
..olmmmmmmmmdo..
...oddddddddo...
....oooooooo....
................
................
"""
DIRECTIONS = """
................
..eeee..........
.eqpwweeeeeeee..
.eqpwwwwwwwwwpe.
.eqpwwwwwwwrwre.
..eewwwwwwwwrwe.
...ewwwwwwwrwre.
...ewwwwwbwwwwe.
...ewwwwwwwwwpe.
...ewwwbbwwwwpe.
...ewwwwwwwwwpe.
...ewbwwwwwwwpe.
...ewwwwwwwwppe.
...eppppppppqqee
....eeeeeeeeeqqe
............eee.
"""


def png(art):
    rows = art.split()
    assert len(rows) == 16 and all(len(row) == 16 for row in rows), rows
    pixels = b"".join(b"\0" + b"".join(bytes.fromhex(COLOURS[c]) for c in row) for row in rows)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    header = struct.pack(">IIBBBBB", 16, 16, 8, 6, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(pixels, 9)) + chunk(b"IEND", b""))


(GRAPHICS / "base_textures" / "items" / "Coin.png").write_bytes(png(COIN))
(GRAPHICS / "hud" / "directions.png").write_bytes(png(DIRECTIONS))
