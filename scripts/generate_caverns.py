"""Builds the creatures of the seas and the deep caves, data/models/animated/<Name>.glb, voxel by
voxel in Blender, with the parts, palettes and exporter of `generate_hostiles.py`: the two fish
of the shoals, Glintfin and Shoalback; the Mossback of the damp caves; the Shardling of the
crystal caverns; and the Slagcrawler of the basalt galleries. Their clips are Attack, Idle and
Walk, as the hostiles' (glTF animations 0, 1 and 2); colours marked `*` glow.

Usage:
  blender --background --factory-startup --python scripts/generate_caverns.py -- [Name ...]
  blender --background --factory-startup --python scripts/generate_caverns.py -- --render DIR
"""

import math
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import generate_hostiles as h  # noqa: E402
from generate_hostiles import bob, box, keys, mirror, swing  # noqa: E402

PI = math.pi


# ------------------------------------------------------------------ the fish

def fish(body_colours, belly, stripe, fin_colour, length, height, seed):
    """A fish lying along y, its head toward -y, its belly a voxel over the origin."""
    rng = random.Random(seed)
    half = length // 2
    body = {}
    for j in range(-half, half):
        # thickest a third of the way from the head
        along = (j + half) / length
        girth = max(2, round(height * math.sin(math.pi * min(0.92, along * 0.9 + 0.12))))
        for k in range(1, 1 + girth):
            for i in (-1, 0):
                colour = rng.choice(body_colours)
                if k == 1:
                    colour = belly
                elif stripe and j % 3 == 0:
                    colour = stripe
                body[(i, j, k)] = colour
    eye = "#101418"
    body[(-2, -half + 1, 3)] = eye
    body[(1, -half + 1, 3)] = eye
    for k in range(height, height + 2):  # the fin on the back
        body[(0, -1 + (k - height), k + 1)] = fin_colour
        body[(-1, -1 + (k - height), k + 1)] = fin_colour
    tail = {}
    for k in range(0, height + 2):
        spread = abs(k - (height + 1) / 2)
        if spread <= (height + 2) / 2:
            tail[(-1, half, k)] = fin_colour
            tail[(0, half, k)] = fin_colour
            if spread > 1:
                tail[(-1, half + 1, k)] = fin_colour
                tail[(0, half + 1, k)] = fin_colour
    parts = {"body": body, "tail": tail}
    pivots = {"body": (0, 0, 2), "tail": (0, half, 2)}
    return parts, pivots, {"tail": (0, 0, 20)}


def glintfin():
    return fish(["#f08a24", "#f59a35", "#e67a1c"], "#fbe3b8", "*#3fb6e0", "#3fb6e0", 8, 4, 21)


def shoalback():
    return fish(["#9fb0bd", "#aebfca", "#8fa1ae"], "#e6edf1", "#4d6a85", "#6f8799", 10, 3, 22)


def clips_fish():
    walk = {"tail": swing(30, period=6, axis=2), "body": swing(4, period=6, axis=2, phase=PI)}
    idle = {"tail": swing(12, period=24, frames=48, axis=2), "body": bob(0.4, period=24, frames=48)}
    attack = {"tail": swing(40, period=4, frames=12, axis=2)}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


# ------------------------------------------------------------------ Mossback

def mossback():
    rng = random.Random(31)
    SHELL = ["#6b5a44", "#5d4d3a", "#7a6850"]
    MOSS = ["#4c7029", "#5d8a33", "#3f5f22", "#6f9a44"]
    SKIN = ["#7d8a6a", "#6f7c5d"]
    BUD, EYE = "*#d8e8a0", "#151a10"
    shell = {}
    for i in range(-6, 6):
        for j in range(-6, 7):
            r = math.hypot(i + 0.5, j * 0.9)
            top = 4 + round(5 * max(0.0, 1 - (r / 7.2) ** 2))
            if r > 7.0:
                continue
            for k in range(3, top + 1):
                shell[(i, j, k)] = rng.choice(SHELL)
            shell[(i, j, top)] = rng.choice(MOSS)
            if rng.random() < 0.08:
                shell[(i, j, top + 1)] = BUD if rng.random() < 0.4 else rng.choice(MOSS)
    head = {}
    box(head, range(-2, 2), range(-10, -6), range(4, 8), lambda *_: rng.choice(SKIN))
    head[(-2, -10, 6)] = EYE
    head[(1, -10, 6)] = EYE
    leg = {}
    box(leg, range(-6, -3), range(-5, -2), range(0, 4), lambda *_: rng.choice(SKIN))
    back = {(i, j + 7, k): c for (i, j, k), c in leg.items()}
    parts = {"shell": shell, "head": head, "leg_FL": leg, "leg_FR": mirror(leg),
             "leg_BL": back, "leg_BR": mirror(back)}
    pivots = {"shell": (0, 0, 3), "head": (0, -6, 5), "leg_FL": (-4.5, -3.5, 4),
              "leg_FR": (4.5, -3.5, 4), "leg_BL": (-4.5, 3.5, 4), "leg_BR": (4.5, 3.5, 4)}
    return parts, pivots, {"head": (10, 0, 15)}


def clips_mossback():
    walk = {"leg_FL": swing(25, period=24), "leg_BR": swing(25, period=24),
            "leg_FR": swing(25, period=24, phase=PI), "leg_BL": swing(25, period=24, phase=PI),
            "shell": bob(0.3, period=12), "head": swing(5, period=24, axis=2)}
    idle = {"head": keys((0, (0, 0, 0)), (24, (12, 0, 0)), (48, (12, 0, 20)), (72, (0, 0, 0))),
            "shell": bob(0.2, period=72, frames=72)}
    attack = {"head": keys((0, (0, 0, 0)), (4, (-15, 0, 0)), (8, (10, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


# ------------------------------------------------------------------ Shardling

def shardling():
    rng = random.Random(41)
    CARAPACE = ["#8f8796", "#7d7684", "#a69eae"]
    CRYSTAL = ["*#b99af0", "*#9b77e0", "*#dcc8fb"]
    EYE = "*#f2eaff"
    body = {}
    box(body, range(-5, 5), range(-4, 4), range(3, 7), lambda *_: rng.choice(CARAPACE))
    for (i, j, height) in ((-3, -1, 4), (0, 1, 5), (2, -2, 3), (-1, 2, 3), (3, 2, 2)):
        for k in range(7, 7 + height):
            body[(i, j, k)] = rng.choice(CRYSTAL)
    body[(-2, -5, 6)] = EYE
    body[(1, -5, 6)] = EYE
    claw = {}
    box(claw, range(-8, -5), range(-8, -4), range(3, 6), lambda *_: rng.choice(CARAPACE))
    claw[(-8, -9, 5)] = rng.choice(CRYSTAL)
    claw[(-6, -9, 5)] = rng.choice(CRYSTAL)
    legs = {}
    for j in (-2, 0, 2):
        for d in range(3):
            legs[(-6 - d, j, 3 - d)] = rng.choice(CARAPACE)
    parts = {"body": body, "claw_L": claw, "claw_R": mirror(claw), "legs_L": legs,
             "legs_R": mirror(legs)}
    pivots = {"body": (0, 0, 3), "claw_L": (-6, -4, 4), "claw_R": (6, -4, 4),
              "legs_L": (-5, 0, 3), "legs_R": (5, 0, 3)}
    return parts, pivots, {"claw_L": (-20, 0, 10), "claw_R": (-10, 0, -10)}


def clips_shardling():
    walk = {"legs_L": swing(20, period=6, axis=1), "legs_R": swing(20, period=6, axis=1, phase=PI),
            "body": bob(0.4, period=6), "claw_L": swing(8, period=12), "claw_R": swing(8, period=12, phase=PI)}
    idle = {"claw_L": keys((0, (0, 0, 0)), (18, (-15, 0, 0)), (36, (0, 0, 0)), (72, (0, 0, 0))),
            "claw_R": keys((0, (0, 0, 0)), (36, (0, 0, 0)), (54, (-15, 0, 0)), (72, (0, 0, 0))),
            "body": bob(0.2, period=36, frames=72)}
    attack = {"claw_L": keys((0, (0, 0, 0)), (4, (-50, 0, 20)), (8, (15, 0, -10)), (16, (0, 0, 0))),
              "claw_R": keys((0, (0, 0, 0)), (4, (-50, 0, -20)), (8, (15, 0, 10)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


# ------------------------------------------------------------------ Slagcrawler

def slagcrawler():
    rng = random.Random(51)
    SLAG = ["#2e2a28", "#3a3431", "#24201e"]
    CRACK = ["*#e8611c", "*#ff8c1a", "*#ffb347"]
    EYE = "*#fff0a0"

    def slag(i, j, k):
        return rng.choice(CRACK) if rng.random() < 0.12 else rng.choice(SLAG)

    parts, pivots = {}, {}
    for n, (j0, width) in enumerate(((-6, 4), (-2, 5), (2, 4), (6, 3))):
        seg = {}
        box(seg, range(-width, width), range(j0, j0 + 4), range(1, 6 - (n == 3)), slag)
        for i in (-width, width - 1):  # stubby legs
            seg[(i - 1 if i < 0 else i + 1, j0 + 1, 1)] = rng.choice(SLAG)
            seg[(i - 1 if i < 0 else i + 1, j0 + 1, 0)] = rng.choice(SLAG)
        parts[f"seg{n}"] = seg
        pivots[f"seg{n}"] = (0, j0 + 2, 3)
    head = {}
    box(head, range(-3, 3), range(-10, -6), range(1, 6), slag)
    head[(-2, -11, 4)] = EYE
    head[(1, -11, 4)] = EYE
    jaw = {}
    for i in (-3, 2):
        for j in (-12, -11):
            jaw[(i, j, 1)] = rng.choice(CRACK)
    parts["head"] = head
    parts["jaw"] = jaw
    pivots["head"] = (0, -6, 3)
    pivots["jaw"] = (0, -10, 1)
    return parts, pivots, {"head": (-10, 0, 10), "seg1": (0, 0, 6), "seg3": (0, 0, -8)}


def clips_slagcrawler():
    def wave(amp, period, frames):
        return {f"seg{n}": swing(amp, period=period, frames=frames, axis=2, phase=n * PI / 2) for n in range(4)}
    walk = {**wave(10, 12, 24), "head": swing(8, period=12, axis=2, phase=-PI / 2)}
    idle = {**wave(3, 72, 72), "jaw": keys((0, (0, 0, 0)), (30, (20, 0, 0)), (40, (0, 0, 0)), (72, (0, 0, 0)))}
    attack = {"head": keys((0, (0, 0, 0)), (4, (-25, 0, 0)), (8, (15, 0, 0)), (16, (0, 0, 0))),
              "jaw": keys((0, (0, 0, 0)), (4, (35, 0, 0)), (8, (0, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


CREATURES = {
    "Glintfin": ("Lueurine", glintfin, clips_fish),
    "Shoalback": ("DosDArgent", shoalback, clips_fish),
    "Mossback": ("Moussecaille", mossback, clips_mossback),
    "Shardling": ("Eclatin", shardling, clips_shardling),
    "Slagcrawler": ("RampeScorie", slagcrawler, clips_slagcrawler),
}
FRAMING = {"Glintfin": (0.15, 0.3), "Shoalback": (0.15, 0.3), "Mossback": (0.35, 0),
           "Shardling": (0.3, 0), "Slagcrawler": (0.25, 0)}


def main():
    # The builder of the hostiles makes these too
    h.MOBS.clear()
    h.MOBS.update(CREATURES)
    h.FRAMING.update(FRAMING)
    args = h.ARGS
    if "--render" in args:
        out = Path(args[args.index("--render") + 1])
        out.mkdir(parents=True, exist_ok=True)
        for name in args[args.index("--render") + 2:] or list(CREATURES):
            scene = h.set_scene()
            root, _ = h.build(name, scene.collection, posed=True)
            centre, lift = FRAMING[name]
            root.location.z = lift
            h.ground(scene.collection)
            h.lights(scene.collection)
            h.camera(scene, scene.collection, (0, 0, centre), 2.4, centre + 0.5)
            h.render(scene, out / f"{name}.png", 600, 600)
        return
    for name in args or CREATURES:
        h.export(name, h.ROOT / "data" / "models" / "animated" / f"{name}.glb")


if __name__ == "__main__":
    main()
