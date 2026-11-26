"""Builds the float of the fishing rod, data/models/animated/Bobber.glb, voxel by voxel in Blender,
with the voxels, the palettes and the exporter of generate_hostiles.py: a body 4 voxels wide, red
above and white below, with a quill on top (`MobKind::Bobber`, 0.25 wide and 0.375 tall). Its
clips: Idle bobs gently on the water, Walk (its flight) stands still, and Attack, played once at a
bite, plunges it and lets it bob back up.

Usage:
  blender --background --factory-startup --python scripts/generate_bobber.py
"""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_hostiles as gh  # noqa: E402
from generate_hostiles import ROOT, box  # noqa: E402

RED, DARK_RED, WHITE, GREY, QUILL = "#c62a2a", "#8f1c1c", "#efeae0", "#bdb6a8", "#3a2a1c"


def bobber():
    """Its bottom on the origin."""
    body = {}
    box(body, range(-2, 2), range(-2, 2), range(0, 2), lambda i, j, k: GREY if (i + j) % 2 else WHITE)
    box(body, range(-2, 2), range(-2, 2), range(2, 4), lambda i, j, k: DARK_RED if (i + j) % 2 else RED)
    box(body, range(-1, 1), range(-1, 1), range(4, 5), RED)
    box(body, range(0, 1), range(0, 1), range(5, 6), QUILL)
    return {"body": body}, {"body": (0, 0, 0)}, {}


def clips():
    still = {"body": [(0, None, None), (24, None, None)]}
    idle = {"body": [(0, (0, 0, 0), None), (12, (0, 0, -0.6), (4, 0, 0)), (24, (0, 0, 0), None),
                     (36, (0, 0, -0.6), (-4, 0, 0)), (48, (0, 0, 0), None)]}
    plunge = {"body": [(0, (0, 0, 0), None), (2, (0, 0, -4), (12, 0, 0)), (8, (0, 0, -3), (-6, 0, 0)),
                       (14, (0, 0, -1), None), (20, (0, 0, 0), None)]}
    return {"Attack": plunge, "Idle": idle, "Walk": still}


gh.MOBS["Bobber"] = ("Bobber", bobber, clips)

if __name__ == "__main__":
    gh.export("Bobber", ROOT / "data" / "models" / "animated" / "Bobber.glb")
