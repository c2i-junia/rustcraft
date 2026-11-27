"""Builds the splash potion in flight, data/models/animated/ThrownPotion.glb, voxel by voxel in
Blender, with the voxels, the palettes and the exporter of generate_hostiles.py: a round flask of
glass 4 voxels wide, its liquid a deep violet, with a neck and a cork (`MobKind::ThrownPotion`,
0.25 wide). Its clips: Walk (its flight) tumbles it end over end, Idle and Attack stand still.

Usage:
  blender --background --factory-startup --python scripts/generate_thrown_potion.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_hostiles as gh  # noqa: E402
from generate_hostiles import ROOT, box  # noqa: E402

GLASS, LIQUID, DARK, CORK = "#cfe3ea", "#7a3fa8", "#5a2b80", "#9a6b3f"


def potion():
    """Its bottom on the origin."""
    body = {}
    box(body, range(-2, 2), range(-2, 2), range(0, 1), GLASS)
    box(body, range(-2, 2), range(-2, 2), range(1, 3), lambda i, j, k: DARK if (i + j) % 2 else LIQUID)
    box(body, range(-2, 2), range(-2, 2), range(3, 4), GLASS)
    box(body, range(-1, 1), range(-1, 1), range(4, 5), GLASS)
    box(body, range(-1, 1), range(-1, 1), range(5, 6), CORK)
    return {"body": body}, {"body": (0, 0, 2)}, {}


def clips():
    still = {"body": [(0, None, None), (24, None, None)]}
    tumble = {"body": [(0, None, (0, 0, 0)), (6, None, (90, 0, 0)), (12, None, (180, 0, 0)),
                       (18, None, (270, 0, 0)), (24, None, (360, 0, 0))]}
    return {"Attack": still, "Idle": still, "Walk": tumble}


gh.MOBS["ThrownPotion"] = ("ThrownPotion", potion, clips)

if __name__ == "__main__":
    gh.export("ThrownPotion", ROOT / "data" / "models" / "animated" / "ThrownPotion.glb")
