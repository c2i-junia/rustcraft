"""Builds the boat, data/models/animated/Boat.glb, voxel by voxel in Blender, with the voxels, the
palettes and the exporter of generate_hostiles.py: a hull of oak planks 2 m long, 1.25 m wide and
0.5 m high, its bow narrowed, two benches (the driver's at the back, the passenger's at the front,
`shared::players::riding::seat_offset`) and a pair of oars. Its three clips (Attack, Idle, Walk)
stand still, as the arrow's.

Usage:
  blender --background --factory-startup --python scripts/generate_boat.py
"""

import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_hostiles as gh  # noqa: E402
from generate_hostiles import ROOT, box  # noqa: E402

PLANKS = ["#a0824f", "#8f7343", "#b08f58", "#9a7b4a"]
TRIM, DARK = "#6e5631", "#5a4527"
OAR, BLADE = "#7d6139", "#8e7044"


def boat():
    """The hull faces -Y in Blender, its bow at y = -16, its keel on the origin."""
    rng = random.Random(23)
    plank = lambda *_: rng.choice(PLANKS)
    hull = {}

    def half_width(j):
        # 10 voxels a side amidships, narrowed over the last five rows of the bow
        return 10 - max(0, -11 - j)

    for j in range(-16, 16):
        w = half_width(j)
        box(hull, range(-w, w), range(j, j + 1), range(0, 1), plank)  # the floor
        for i in (-w, w - 1):  # the sides
            box(hull, range(i, i + 1), range(j, j + 1), range(1, 8), plank)
    for j in (-16, 15):  # the bow and the stern
        w = half_width(j)
        box(hull, range(-w, w), range(j, j + 1), range(1, 8), plank)
    for (i, j, k) in list(hull):  # the gunwale
        if k == 7:
            hull[(i, j, k)] = TRIM
    for j in (-7, -6, 5, 6):  # the benches, their tops 0.25 above the keel
        box(hull, range(-9, 9), range(j, j + 1), range(3, 4), DARK if j in (-7, 5) else TRIM)
    for i in (-11, 10):  # the oars, resting along the sides
        box(hull, range(i, i + 1), range(-2, 10), range(6, 7), OAR)
        box(hull, range(i, i + 1), range(10, 14), range(5, 7), BLADE)
    return {"hull": hull}, {"hull": (0, 0, 0)}, {}


def clips():
    still = {"hull": [(0, None, None), (24, None, None)]}
    return {"Attack": still, "Idle": still, "Walk": still}


gh.MOBS["Boat"] = ("Boat", boat, clips)

if __name__ == "__main__":
    gh.export("Boat", ROOT / "data" / "models" / "animated" / "Boat.glb")
