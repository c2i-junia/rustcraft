"""Builds the villagers, data/models/animated/{Farmer,Smith,Cartographer}.glb, voxel by voxel in
Blender, with the voxels, the palettes and the exporter of generate_hostiles.py: short round people,
all of one build, told apart by what they wear. Their clips are Idle and Walk.

Usage:
  blender --background --factory-startup --python scripts/generate_villagers.py -- [Name ...]
  blender --background --factory-startup --python scripts/generate_villagers.py -- --render DIR
"""

import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import generate_hostiles as gh  # noqa: E402
from generate_hostiles import PI, ROOT, bob, box, keys, mirror, swing  # noqa: E402


def rounded(cells, xr, yr, zr, colour, cut=1, caps=True):
    """A box whose upright edges are cut, and its lowest and highest layers drawn in."""
    for i in xr:
        for j in yr:
            for k in zr:
                x = min(i - xr[0], xr[-1] - i)
                y = min(j - yr[0], yr[-1] - j)
                end = caps and k in (zr[0], zr[-1])
                if x + y < cut + end:
                    continue
                cells[(i, j, k)] = colour(i, j, k) if callable(colour) else colour


def person(skin, shirt, trousers, boots, hair, eyes, sleeve):
    """The body under the outfit, on short legs: a belly, a round head wider than it is tall and
    large hands. `sleeve` is the lowest voxel of the sleeves."""
    rng = random.Random(skin[0])
    pick = lambda colours: lambda i, j, k: rng.choice(colours)
    parts = {}
    leg = {}
    box(leg, range(-4, -1), range(-1, 2), range(2, 7), pick(trousers))
    box(leg, range(-5, 0), range(-4, 2), range(0, 2), pick(boots))
    for c in ((-5, -4, 1), (-1, -4, 1), (-5, -4, 0)):
        leg.pop(c)
    parts["leg_L"], parts["leg_R"] = leg, mirror(leg)

    body = {}
    rounded(body, range(-5, 5), range(-3, 3), range(6, 12), pick(shirt))
    rounded(body, range(-4, 4), range(-3, 3), range(11, 17), pick(shirt))
    parts["body"] = body

    head = {}
    rounded(head, range(-5, 5), range(-4, 5), range(17, 25), pick(skin), cut=2)
    for (i, j, k) in list(head):
        if (k >= 20 and j >= 1) or k >= 23 or (k >= 21 and j >= -1 and i in (-5, 4)):
            head[(i, j, k)] = rng.choice(hair)
    for i in (-2, 1):  # eyes of one dark voxel, set wide, under the brows
        head[(i, -4, 20)] = head[(i, -4, 21)] = eyes
    for i in (-3, 2):
        head[(i, -4, 22)] = head[(i + (i < 0), -4, 22)] = hair[0]
    box(head, range(-1, 1), range(-5, -4), range(19, 20), skin[1])
    for i in (-3, 2):
        head[(i, -4, 19)] = skin[2]
    for i in (-6, 5):  # ears
        for k in (19, 20):
            head[(i, 0, k)] = skin[1]
    parts["head"] = head

    arm = {}
    box(arm, range(-7, -5), range(-1, 2), range(9, 16), pick(skin))
    box(arm, range(-8, -5), range(-2, 2), range(6, 9), pick(skin))  # the hand
    box(arm, range(-7, -5), range(-1, 2), range(sleeve, 16), pick(shirt))
    box(arm, range(-5, -4), range(-1, 2), range(13, 16), pick(shirt))
    arm.pop((-8, -2, 6)), arm.pop((-8, 1, 6))
    parts["arm_L"], parts["arm_R"] = arm, mirror(arm)
    pivots = {"leg_L": (-2.5, 0.5, 7), "leg_R": (2.5, 0.5, 7), "body": (0, 0, 7), "head": (0, 0.5, 17),
              "arm_L": (-5, 0.5, 15), "arm_R": (5, 0.5, 15)}
    pose = {"arm_L": (-22, 0, 0), "arm_R": (16, 0, 0), "leg_L": (12, 0, 0), "leg_R": (-12, 0, 0),
            "head": (0, 0, -8)}
    return parts, pivots, pose


def farmer():
    """Rolled sleeves, ochre dungarees, a leaf-green kerchief tied behind the head and a basket
    of vegetables on the back."""
    parts, pivots, pose = person(
        ["#d9a27a", "#c98f68", "#d58b70"], ["#efe6cd", "#e6dabc"], ["#b8792f", "#ab6f2a"],
        ["#5b4028", "#513822"], ["#a4532a", "#964a25"], "#2c2014", 13)
    body, head = parts["body"], parts["head"]
    ochre = ["#b8792f", "#ab6f2a"]
    rng = random.Random(1)
    for (i, j, k) in list(body):
        if k < 11 or (j == -3 and k < 14 and -4 < i < 3):
            body[(i, j, k)] = rng.choice(ochre)
    for i in (-3, 2):  # the straps
        for k in range(13, 17):
            body[(i, -3, k)] = body[(i, 2, k)] = ochre[1]
        body[(i, -4, 13)] = "#e8d9a0"
    box(body, range(-2, 2), range(-4, -3), range(8, 11), "#94602a")  # a pocket of rye
    body[(-1, -4, 11)] = body[(-1, -4, 12)] = "#e2be4e"
    body[(1, -4, 11)] = "#e2be4e"
    for j in range(3, 7):  # the basket
        for i in range(-4, 4):
            for k in range(7, 13):
                if j in (3, 6) or i in (-4, 3) or k == 7:
                    body[(i, j, k)] = ["#9a7343", "#82603a"][(i + j + k) % 2]
    for i, j, c in ((-3, 4, "#e07a2a"), (-1, 5, "#c9a56a"), (1, 4, "#e07a2a"), (2, 5, "#c9a56a"), (0, 5, "#e07a2a")):
        body[(i, j, 13)] = c
        body[(i, j, 14)] = "#5f9a3a" if c == "#e07a2a" else c
    leaf = ["#5c9a45", "#538d3e"]
    for (i, j, k) in list(head):  # the kerchief, white-dotted
        if k >= 23 or (k >= 20 and j >= 1):
            head[(i, j, k)] = "#eef2dc" if (i * 2 + j + k * 3) % 11 == 0 else rng.choice(leaf)
    for i, k in ((-1, 20), (0, 20), (-2, 19), (1, 19), (-3, 18), (2, 18)):
        head[(i, 5, k)] = leaf[1]
    box(head, range(-1, 1), range(-5, -4), range(23, 24), leaf[1])
    for i in (-1, 0):
        head[(i, -4, 18)] = "#a85a48"
    return parts, pivots, pose


def smith():
    """A leather apron over bare arms, long gloves, a red headband round a bald head, a braided
    beard and a hammer at the belt."""
    parts, pivots, pose = person(
        ["#8d5f41", "#7e5338", "#95573f"], ["#6d6a66", "#64615d"], ["#3a3632", "#34302d"],
        ["#241f1c", "#2b2521"], ["#8d5f41", "#86593c"], "#140d08", 14)
    body, head = parts["body"], parts["head"]
    rng = random.Random(2)
    leather = ["#7a4a28", "#6f4324", "#835230"]
    box(body, range(-4, 4), range(-4, -3), range(5, 15), lambda *_: rng.choice(leather))
    for i in (-4, 3):
        body.pop((i, -4, 14)), body.pop((i, -4, 5))
    for (i, j, k) in list(body):
        if k == 9:
            body[(i, j, k)] = "#4a2c18"
    box(body, range(-1, 1), range(-5, -4), range(9, 10), "#c9a24a")
    for c in ((-3, -4, 8), (2, -4, 11), (0, -4, 7)):
        body[c] = "#33231a"  # burns
    box(body, range(5, 6), range(-1, 1), range(4, 9), "#8f6a3e")  # the hammer
    box(body, range(5, 7), range(-2, 2), range(9, 11), "#8d939b")
    for arm in ("arm_L", "arm_R"):
        for c in list(parts[arm]):
            if c[2] < 11:
                parts[arm][c] = "#5a3820" if c[2] < 10 else "#9c6638"
    BEARD = ["#c9c2b6", "#b8b0a3"]
    for (i, j, k) in list(head):
        if k in (18, 17) and j <= -1 or (k in (19, 20) and i in (-5, 4) and j <= 0):
            head[(i, j, k)] = rng.choice(BEARD)
    box(head, range(-3, 3), range(-5, -4), range(16, 19), lambda *_: rng.choice(BEARD))
    box(head, range(-1, 1), range(-5, -4), range(13, 16), lambda *_: rng.choice(BEARD))
    box(head, range(-1, 1), range(-5, -4), range(14, 15), "#b5352c")  # the band of the braid
    for i in (-1, 0):
        head[(i, -5, 18)] = "#7a3f33"
    for i in (-3, -2, 1, 2):
        head[(i, -4, 22)] = BEARD[1]
    for (i, j, k) in list(head):
        if k == 23:
            head[(i, j, k)] = "#b5352c"
    for c in ((3, 5, 23), (3, 5, 22), (4, 5, 21)):
        head[c] = "#9a2b24"
    return parts, pivots, pose


def cartographer():
    """A teal travelling cape over a pale tunic, a wide-brimmed felt hat turned up on one side,
    with a plume, rolled maps on the back and a compass on the chest."""
    parts, pivots, pose = person(
        ["#e8bf9c", "#d8a885", "#e09a86"], ["#d9cfb4", "#cfc4a6"], ["#5d5146", "#54493f"],
        ["#3a2618", "#432d1d"], ["#3c2a20", "#46322a"], "#1c2a44", 8)
    body, head = parts["body"], parts["head"]
    rng = random.Random(3)
    teal = ["#1f6f73", "#1b6367", "#237a7d"]
    for i in range(-6, 6):  # the cape, open in front
        for k in range(4, 16):
            if -5 <= i < 5:
                body[(i, 3, k)] = rng.choice(teal)
    box(body, range(-5, 5), range(-4, 4), range(14, 17), lambda *_: rng.choice(teal))
    for c in [(i, j, 16) for i in (-5, 4) for j in (-4, 3)]:
        body.pop(c)
    for i in range(-5, 5):
        body[(i, 3, 4)] = "#d8a83c"
        if i not in (-1, 0):
            body[(i, -4, 14)] = "#d8a83c"
    for (i, j, k) in list(body):
        if k == 9 and j < 3:
            body[(i, j, k)] = "#6b4424"
    box(body, range(-1, 1), range(-4, -3), range(11, 13), "#d8b24a")  # the compass
    body[(0, -5, 12)] = "#c23b2e"
    body[(-1, -5, 11)] = "#f2ead6"
    for x, top in ((-3, 19), (0, 21), (2, 18)):  # the rolled maps
        for i in (x, x + 1):
            for j in (4, 5):
                for k in range(9, top):
                    body[(i, j, k)] = "#b23a30" if k == 13 else "#ece0bd"
                body[(i, j, top - 1)] = "#c9b88a"
    felt = ["#5a4636", "#524030", "#634d3c"]
    rounded(head, range(-8, 8), range(-7, 8), range(23, 24), lambda *_: rng.choice(felt), cut=3, caps=False)
    for j in range(-4, 5):  # the brim turned up
        head.pop((7, j, 23), None)
        for k in (24, 25):
            head[(6, j, k)] = felt[1]
    rounded(head, range(-4, 4), range(-3, 4), range(24, 28), lambda *_: rng.choice(felt), cut=1, caps=False)
    for (i, j, k) in list(head):
        if k == 24 and -5 < i < 5:
            head[(i, j, k)] = "#d8a83c"
    for j, k in ((-2, 25), (-1, 26), (0, 27), (1, 28), (2, 28), (3, 28), (4, 27), (5, 26)):
        head[(5, j, k)] = "#e9f0ee"
        head[(5, j, k - 1)] = "#b9d4d2"
    for i in (-1, 0):
        head[(i, -4, 17 + 1)] = "#a85a4c"
    return parts, pivots, pose


def clips():
    walk = {"leg_L": swing(26), "leg_R": swing(26, phase=PI), "arm_L": swing(20, phase=PI),
            "arm_R": swing(20), "body": bob(0.3, period=12), "head": bob(0.3, period=12)}
    idle = {"head": keys((0, (0, 0, 0)), (18, (0, 0, 16)), (36, (4, 0, 10)), (54, (0, 0, -14)), (72, (0, 0, 0))),
            "arm_L": swing(3, period=72, frames=72), "arm_R": swing(3, period=72, frames=72, phase=PI)}
    return {"Idle": idle, "Walk": walk}


VILLAGERS = {"Farmer": farmer, "Smith": smith, "Cartographer": cartographer}
gh.MOBS.update({name: (name, builder, clips) for name, builder in VILLAGERS.items()})


def preview(out):
    out.mkdir(parents=True, exist_ok=True)
    for view, turn in (("front", 0.35), ("back", 3.5)):
        scene = gh.set_scene()
        scene.cycles.samples = 24
        for name, x in (("Farmer", -1.4), ("Smith", 0), ("Cartographer", 1.4)):
            root, _ = gh.build(name, scene.collection, posed=True)
            root.location.x = x
            root.rotation_euler.z = turn
        gh.ground(scene.collection)
        gh.lights(scene.collection, warm=True)
        gh.camera(scene, scene.collection, (0, 0, 0.95), 5.4, 1.7, angle=0)
        gh.render(scene, out / f"villagers_{view}.png", 1100, 700)


if __name__ == "__main__":
    if "--render" in gh.ARGS:
        preview(Path(gh.ARGS[gh.ARGS.index("--render") + 1]))
    else:
        for villager in gh.ARGS or VILLAGERS:
            gh.export(villager, ROOT / "data" / "models" / "animated" / f"{villager}.glb")
