"""Draws the 16x16 textures of the engraving table: a bench of hewn stone rimmed with copper, its
obsidian top carved with a ring of teal strokes round a diamond point.

Usage: python3 scripts/generate_engraving_table.py
"""

import struct
import zlib
from pathlib import Path

BLOCKS = Path(__file__).resolve().parent.parent / "data" / "graphics" / "base_textures" / "blocks"
COLOURS = {
    # copper, hewn stone, obsidian, teal, diamond, shade
    "C": "d98a56", "c": "b06a3c", "k": "7d4726", "S": "8b8d92", "s": "75777d", "r": "5c5e66", "o": "1c1d2b",
    "p": "2a2940", "T": "3fc4b4", "t": "1f7f7e", "D": "c8fbf2", "d": "7fe3d8", "x": "15161d",
}
TOP = """
kCCCCCCCCCCCCCCk
CckppppppppppkcC
CkpoooooooooopkC
CpooooTttToooopC
CpootToooooTtopC
CpoTopoooopoTopC
CpotooodDoootopC
CpotoodDDdootopC
CpotoodDDdootopC
CpotooodDoootopC
CpoTopoooopoTopC
CpootToooooTtopC
CpooooTttToooopC
CkpoooooooooopkC
CckppppppppppkcC
kCCCCCCCCCCCCCCk
"""
SIDE = """
CCCCCCCCCCCCCCCC
cCccccCccCccccCc
kkkkkkkkkkkkkkkk
popoopopopoopopo
oTootoTootoTooto
ototTotoTotoTtot
ootooootooootooo
pppopppopppopppo
rrrrrrrrrrrrrrrr
SSSSsxxxxxxsSSSr
SssssxxxxxxsSssr
SsrssxxxxxxsSssr
SssssxxxxxxsSrsr
SssrsxxxxxxsSssr
SssssxxxxxxsSssr
rrrrrxxxxxxrrrrr
"""


def png(art):
    rows = art.split()
    assert len(rows) == 16 and all(len(row) == 16 for row in rows), rows
    pixels = b"".join(b"\0" + b"".join(bytes.fromhex(COLOURS[c]) for c in row) for row in rows)

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    header = struct.pack(">IIBBBBB", 16, 16, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header)
            + chunk(b"IDAT", zlib.compress(pixels, 9)) + chunk(b"IEND", b""))


(BLOCKS / "EngravingTable.png").write_bytes(png(SIDE))
(BLOCKS / "EngravingTableTop.png").write_bytes(png(TOP))
