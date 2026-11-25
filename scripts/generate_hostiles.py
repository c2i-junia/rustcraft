"""Builds the hostile mobs, data/models/animated/<Name>.glb, voxel by voxel in Blender.

Seven mobs on the 1/16 m grid of the sheep: Stumpling, Drowner, Cinderling and GustOwl, the
mobs that lead to a death of their own (blows, drowning, fire, falls), and Gnawer, Bramble and
Roller, simpler ones; the boss EmberMother; and the Ember a Cinderling throws, whose three clips are one spin, and
the Arrow of the players' bow, which stands still. Each part is a rigid object whose voxels take their colour from a 16x16
palette drawn with nearest filtering; colours marked `*` glow. The clips move whole parts and
are Attack, Idle and Walk, which the exporter sorts by name into glTF animations 0, 1 and 2,
at 24 fps. The models are in metres with their feet on the origin and face -Y in Blender,
+Z in glTF. Fixed seeds make the output reproducible.

Usage:
  blender --background --factory-startup --python scripts/generate_hostiles.py -- [Name ...]
  blender --background --factory-startup --python scripts/generate_hostiles.py -- --render DIR
"""

import math
import random
import sys
import tempfile
from pathlib import Path

import bmesh
import bpy
from mathutils import Vector

ROOT = Path(__file__).resolve().parent.parent
ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
VOX = 1 / 16
FPS = 24
P = 16  # side of the palette texture

FACES = (
    ((1, 0, 0), ((1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1))),
    ((-1, 0, 0), ((0, 1, 0), (0, 0, 0), (0, 0, 1), (0, 1, 1))),
    ((0, 1, 0), ((1, 1, 0), (0, 1, 0), (0, 1, 1), (1, 1, 1))),
    ((0, -1, 0), ((0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1))),
    ((0, 0, 1), ((0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1))),
    ((0, 0, -1), ((0, 1, 0), (1, 1, 0), (1, 0, 0), (0, 0, 0))),
)


def box(cells, xr, yr, zr, colour):
    for i in xr:
        for j in yr:
            for k in zr:
                cells[(i, j, k)] = colour(i, j, k) if callable(colour) else colour


def mirror(cells):
    """Mirror across x = 0 (cell i maps to -1 - i)."""
    return {(-1 - i, j, k): c for (i, j, k), c in cells.items()}


# ------------------------------------------------------------------ Souchard (night melee)

def souchard():
    rng = random.Random(3)
    BARK = ["#4a3524", "#3e2c1f", "#56402c"]
    GROOVE = "#2a1d14"
    MOSS = ["#5d7a2e", "#6f8f37", "#4b6526"]
    RING = ["#b48a5a", "#9c7448", "#c49a66"]
    HOLLOW, EYE, EMBER = "#120c08", "*#ffb347", "*#ff7a1a"
    SHROOM, SHROOM_D = "#b0452f", "#e9d6b8"

    def bark(i, j, k):
        return GROOVE if (i + 7) % 3 == 0 and rng.random() < 0.7 else rng.choice(BARK)

    parts = {}
    leg = {}
    box(leg, range(-4, -1), range(-1, 2), range(2, 10), bark)
    box(leg, range(-5, 0), range(-2, 3), range(0, 2), bark)  # roots spreading at the foot
    for c in [(-5, -2, 1), (-1, 2, 1), (-5, 2, 1), (-1, -2, 1)]:
        leg.pop(c, None)
    parts["leg_L"] = leg
    parts["leg_R"] = mirror(leg)

    body = {}
    box(body, range(-5, 5), range(-2, 3), range(10, 22), bark)
    for (i, j, k) in list(body):
        if k >= 20 and (j == -2 or k == 21) and rng.random() < 0.8:
            body[(i, j, k)] = rng.choice(MOSS)
    # A crack in the chest with an ember glowing inside
    for k, i in ((13, 0), (14, -1), (15, 0), (16, 0), (17, -1), (18, -1)):
        body[(i, -2, k)] = EMBER if k in (15, 16) else HOLLOW
    for i in range(-6, 6):  # moss drapes over the shoulders
        if rng.random() < 0.6:
            body[(i, rng.choice((-3, 3)), 21)] = rng.choice(MOSS)
    parts["body"] = body

    head = {}
    box(head, range(-4, 4), range(-3, 4), range(22, 29), bark)
    box(head, range(-3, 3), range(-2, 3), range(29, 30), lambda i, j, k: RING[(max(abs(i + 0.5), abs(j)) > 1.5) + (max(abs(i + 0.5), abs(j)) > 2.5)])
    for i in range(-3, 3):  # the hollow face, set back by one voxel
        for k in range(23, 28):
            head.pop((i, -3, k), None)
            head[(i, -2, k)] = HOLLOW
    head[(-2, -2, 26)] = EYE
    head[(1, -2, 26)] = EYE
    head[(-2, -3, 23)] = rng.choice(BARK)  # jagged lower lip
    head[(1, -3, 23)] = rng.choice(BARK)
    # A shelf fungus on its side, and a little toadstool on top
    for j in range(-1, 3):
        head[(4, j, 25)] = SHROOM
        head[(5, j, 25)] = SHROOM
    head[(4, 0, 24)] = SHROOM_D
    head[(-3, 1, 30)] = SHROOM_D
    for i in (-4, -3, -2):
        for j in (0, 1, 2):
            head[(i, j, 31)] = SHROOM
    head[(-3, 1, 32)] = SHROOM
    head[(-4, 0, 31)] = SHROOM_D
    parts["head"] = head

    arm = {}
    box(arm, range(-8, -5), range(-1, 2), range(9, 22), bark)
    for c in [(-8, -1, 21), (-6, -1, 21), (-8, 1, 21)]:
        arm.pop(c)
    for k, (i, j) in ((8, (-8, -1)), (7, (-8, -1)), (6, (-9, -2)), (8, (-6, -1)), (7, (-6, -2)),
                      (6, (-6, -3)), (8, (-7, 1)), (7, (-7, 1))):
        arm[(i, j, k)] = GROOVE
    arm[(-8, 0, 20)] = rng.choice(MOSS)
    parts["arm_L"] = arm
    parts["arm_R"] = mirror(arm)
    pivots = {"leg_L": (-2.5, 0.5, 10), "leg_R": (2.5, 0.5, 10), "body": (0, 0.5, 10),
              "head": (0, 0.5, 22), "arm_L": (-6.5, 0.5, 21), "arm_R": (6.5, 0.5, 21)}
    pose = {"arm_L": (-55, 0, 8), "arm_R": (-30, 0, -6), "head": (0, 0, -8), "leg_L": (12, 0, 0), "leg_R": (-12, 0, 0)}
    return parts, pivots, pose


# ------------------------------------------------------------------ Noyeur (drowner)

def noyeur():
    rng = random.Random(7)
    SKIN = ["#2f5f5a", "#2a5450", "#356a63"]
    SPOT = "#1b3936"
    BELLY = ["#9cbfa6", "#8fb39b"]
    WEED = ["#3d6b2a", "#4f8232", "#2f5621"]
    FIN = ["#7a2e4f", "#93395f"]
    EYE_W, PUPIL, GLOW, TOOTH, MOUTH = "*#4fe8c4", "#05100e", "*#5ff2d0", "#efe8d2", "#0c1615"

    skin = lambda *_: SPOT if rng.random() < 0.12 else rng.choice(SKIN)
    parts = {}
    leg = {}
    box(leg, range(-5, -1), range(-1, 3), range(1, 7), skin)
    box(leg, range(-6, -1), range(-4, 2), range(0, 1), skin)  # webbed foot
    for i in (-6, -4, -2):
        leg[(i, -5, 0)] = SPOT
    parts["leg_L"] = leg
    parts["leg_R"] = mirror(leg)

    body = {}
    box(body, range(-5, 5), range(-2, 4), range(7, 17), skin)
    for i in range(-3, 3):
        for k in range(7, 15):
            body[(i, -2, k)] = rng.choice(BELLY)
    for c in ((-3, 3, 15), (2, 3, 13), (-1, 3, 11), (3, 3, 9), (-4, 3, 12), (0, 3, 15)):
        body[(c[0], 4, c[2])] = GLOW  # bioluminescent spots on the back
    for x in (-5, -3, 3, 4):  # seaweed hanging from the shoulders
        for k in range(16 - rng.randint(4, 8), 17):
            body[(x, -3 if x < 0 else 4, k)] = rng.choice(WEED)
    parts["body"] = body

    head = {}
    box(head, range(-6, 6), range(-6, 2), range(15, 21), skin)
    for i in range(-6, 6):
        head[(i, -6, 15)] = MOUTH
        head[(i, -6, 16)] = MOUTH if abs(i + 0.5) < 4.5 else head[(i, -6, 16)]
    for i in (-4, -2, 1, 3):
        head[(i, -6, 16)] = TOOTH
    for i in (-3, 0, 2):
        head[(i, -6, 15)] = TOOTH
    for side in (-6, 4):  # big bulging eyes on the front corners
        for di in (0, 1):
            for k in (19, 20, 21):
                head[(side + di, -7, k)] = EYE_W
        head[(side + (1 if side < 0 else 0), -8, 20)] = PUPIL
        head[(side + (1 if side < 0 else 0), -7, 20)] = PUPIL
    for j in range(-4, 3):  # the crest fin
        for k in range(21, 21 + (4 - abs(j + 1) // 2)):
            head[(-1, j, k)] = FIN[(j + k) % 2]
            head[(0, j, k)] = FIN[(j + k + 1) % 2]
    parts["head"] = head

    arm = {}
    box(arm, range(-8, -5), range(-1, 2), range(4, 16), skin)
    for i in (-8, -7, -6):
        arm[(i, -2, 3)] = TOOTH
        arm[(i, -2, 4)] = SPOT
    parts["arm_L"] = arm
    parts["arm_R"] = mirror(arm)
    pivots = {"leg_L": (-3.5, 1, 7), "leg_R": (3.5, 1, 7), "body": (0, 1, 7), "head": (0, -1, 16),
              "arm_L": (-6.5, 0.5, 15), "arm_R": (6.5, 0.5, 15)}
    pose = {"arm_L": (-12, 0, -6), "arm_R": (8, 0, 6), "head": (-4, 0, 10), "body": (18, 0, 0)}
    return parts, pivots, pose


# ------------------------------------------------------------------ Braisille (fire)

def braisille():
    rng = random.Random(11)
    ROCK = ["#2b2523", "#221d1c", "#35302d", "#1a1615"]
    CRACK = ["*#ff6a00", "*#ff8c1a", "*#ffb000"]
    FLAME = ["*#ffd24a", "*#ffa31a", "*#ff6a00", "*#fff1a8"]
    EYE = "*#fff6c2"
    parts = {}
    body = {}
    box(body, range(-5, 5), range(-5, 5), range(8, 18), lambda *_: rng.choice(ROCK))
    for c in list(body):  # bevel the corners
        i, j, k = c
        if sum((i in (-5, 4), j in (-5, 4), k in (8, 17))) >= 2:
            del body[c]
    # lava cracks running down every side
    walks = [(-4, -5, 17, (0, 0, -1)), (-5, 1, 17, (0, 0, -1)),
             (4, -1, 17, (0, 0, -1)), (0, 4, 17, (0, 0, -1)), (-3, 4, 15, (0, 0, -1))]
    for i, j, k, _ in walks:
        for step in range(9):
            if (i, j, k) in body:
                body[(i, j, k)] = rng.choice(CRACK)
            k -= 1
            if rng.random() < 0.35:
                if j in (-5, 4):
                    i = max(-4, min(3, i + rng.choice((-1, 1))))
                else:
                    j = max(-4, min(3, j + rng.choice((-1, 1))))
    for i in range(-4, 4):
        for j in range(-4, 4):
            if rng.random() < 0.25:
                body[(i, j, 18)] = rng.choice(CRACK)
    # the face: two slit eyes and a jagged mouth
    for i in (-3, -2, 1, 2):
        body[(i, -5, 14)] = EYE
    body[(-2, -5, 15)] = EYE
    body[(1, -5, 15)] = EYE
    for i, k in ((-2, 11), (-1, 10), (0, 11), (1, 10)):
        body[(i, -5, k)] = CRACK[0]
    parts["body"] = body

    flame = {}
    for k in range(18, 26):
        r = max(0, 4 - (k - 18) // 2)
        for i in range(-r, r):
            for j in range(-r, r):
                if rng.random() < (0.95 - (k - 18) * 0.08):
                    flame[(i, j, k)] = FLAME[min(3, (k - 18) // 2)] if rng.random() < 0.8 else rng.choice(FLAME)
    flame[(0, 0, 26)] = FLAME[3]
    flame[(-1, -1, 27)] = FLAME[3]
    parts["flame"] = flame

    for n, (cx, cy, cz) in enumerate(((-9, -2, 15), (8, 1, 11), (-1, 7, 7), (5, -8, 18))):
        ember = {}
        s = 2 if n % 2 == 0 else 1
        box(ember, range(cx, cx + s), range(cy, cy + s), range(cz, cz + s), lambda *_: rng.choice(CRACK + FLAME[:2]))
        parts[f"ember{n}"] = ember
    # chunks of rock orbiting beneath: its "legs" are fragments, it floats
    shard = {}
    box(shard, range(-2, 2), range(-2, 2), range(4, 6), lambda *_: rng.choice(ROCK))
    shard[(0, 0, 3)] = ROCK[0]
    shard[(-1, 0, 4)] = CRACK[1]
    parts["shard"] = shard
    pivots = {name: (0, 0, 12) for name in parts}
    pose = {"body": (0, 0, -12)}
    return parts, pivots, pose


# ------------------------------------------------------------------ Mere (the boss, the Cinderlings' mother)

def mere():
    """A kiln of basalt that woke: a bell of cracked rock with a furnace in the chest, a small
    head under a crown of flames, two arms of slabs, and three clutches of embers she carries."""
    rng = random.Random(29)
    ROCK = ["#2a2422", "#211c1b", "#352d2a", "#181413"]
    PLATE = ["#443833", "#4d403a", "#3b312d"]
    ASH = ["#6b5f58", "#7a6e66"]
    CRACK = ["*#ff5a00", "*#ff7d12", "*#ffa51f"]
    CORE = ["*#ffd24a", "*#fff1a8", "*#ffb52e"]
    FLAME = ["*#fff1a8", "*#ffd24a", "*#ffa31a", "*#ff6a00", "*#d93a00"]
    BONE, EYE = "#1a1210", "*#fff6c2"
    parts = {}

    def radius(k):  # the bell: wide at the hem, narrow under the shoulders, then the shoulders
        if k < 24:
            return 11.5 - (k - 3) * 0.24
        return 8.2 if k < 30 else 8.2 - (k - 29) * 0.9

    body = {}
    for k in range(3, 34):
        r = radius(k)
        for i in range(-12, 12):
            for j in range(-12, 12):
                x, y = abs(i + 0.5), abs(j + 0.5)
                # an octagon rather than a circle: rock, not cloth
                if max(x, y) < r and x + y < r * 1.42:
                    body[(i, j, k)] = rng.choice(PLATE if k % 8 in (2, 3) else ROCK)
    for (i, j, k) in list(body):  # ash on the shoulders, a ragged glowing hem
        if k >= 29 and rng.random() < 0.4:
            body[(i, j, k)] = rng.choice(ASH)
        if k == 3 and rng.random() < 0.45:
            body[(i, j, k)] = rng.choice(CRACK)
        if k == 3 and rng.random() < 0.3:
            body.pop((i, j, k))
    # seams of fire winding up the bell
    for n in range(14):
        angle = 2 * math.pi * n / 14
        for k in range(4, 23):
            angle += rng.uniform(-0.13, 0.13)
            r = radius(k) + 0.5
            for step in range(16):
                c = (math.floor(math.cos(angle) * r), math.floor(math.sin(angle) * r), k)
                if c in body:
                    if rng.random() < 0.8:
                        body[c] = rng.choice(CRACK)
                    break
                r -= 0.35
    # the furnace of the chest: an arched mouth barred with rock over a white core
    for i in range(-7, 7):
        for k in range(12, 25):
            x = abs(i + 0.5) + max(0, k - 18) * 0.8
            if x < 5.2:
                for j in range(-12, -4):
                    body.pop((i, j, k), None)
                body[(i, -5, k)] = CORE[1] if x < 2 and k < 18 else rng.choice(CORE + CRACK)
                if i in (-3, 0, 2) and k < 22:
                    body[(i, -8, k)] = rng.choice(ROCK)
            elif x < 7.2:
                front = min((j for j in range(-12, 0) if (i, j, k) in body), default=None)
                if front is not None:
                    body[(i, front - 1, k)] = rng.choice(PLATE)
    parts["body"] = body

    head = {}
    box(head, range(-4, 4), range(-5, 4), range(33, 41), lambda *_: rng.choice(ROCK))
    box(head, range(-3, 3), range(-4, 3), range(41, 42), lambda *_: rng.choice(CRACK))
    for i, k in ((-4, 33), (3, 33), (-4, 40), (3, 40)):
        for j in range(-5, 4):
            head.pop((i, j, k), None)
    for i in range(-4, 4):  # a brow of plates over the eyes
        head[(i, -6, 39)] = rng.choice(PLATE)
    for i in (-3, -2, 1, 2):
        head[(i, -5, 37)] = EYE
        head[(i, -5, 38)] = EYE if i in (-2, 1) else BONE
    for i in range(-2, 2):  # the mouth: a grille
        head[(i, -5, 34)] = CRACK[2] if i % 2 else BONE
        head[(i, -5, 35)] = CRACK[0]
    for side in (-1, 1):  # two chimneys for horns, bent outward
        x = 4 if side > 0 else -6
        box(head, range(x, x + 2), range(-1, 1), range(37, 41), lambda *_: rng.choice(PLATE))
        x += side
        box(head, range(x, x + 2), range(-1, 1), range(41, 45), lambda *_: rng.choice(PLATE))
        box(head, range(x, x + 2), range(-1, 1), range(45, 46), CRACK[2])
    parts["head"] = head

    flame = {}
    for k in range(42, 51):
        r = max(0.0, 4.2 - (k - 42) * 0.45)
        for i in range(-5, 5):
            for j in range(-5, 5):
                if math.hypot(i + 0.5, j + 0.5) < r and rng.random() < 0.94 - (k - 42) * 0.06:
                    flame[(i, j, k)] = FLAME[min(4, (k - 42) // 2)]
    for i, j, top in ((-3, 1, 49), (2, -1, 51), (0, 2, 50)):
        for k in range(44, top):
            flame[(i, j, k)] = rng.choice(FLAME[2:])
    parts["flame"] = flame

    arm = {}
    for low, high, half in ((24, 31, 3), (16, 23, 2), (8, 15, 3)):
        box(arm, range(-13 - half, -13 + half), range(-half, half), range(low, high),
            lambda *_: rng.choice(ROCK + PLATE))
        box(arm, range(-14, -12), range(-1, 1), range(low - 1, low), CRACK[1])
    for i in (-16, -14, -12):  # fingers
        box(arm, range(i, i + 1), range(-3, -1), range(4, 8), lambda *_: rng.choice(PLATE))
        arm[(i, -3, 4)] = CRACK[0]
    for c in list(arm):
        if rng.random() < 0.07:
            arm[c] = rng.choice(CRACK)
    parts["arm_L"] = arm
    parts["arm_R"] = mirror(arm)

    for n, (cx, cy, cz) in enumerate(((-17, 6, 34), (14, 9, 30), (0, 15, 22))):
        clutch = {}
        for i in range(3):
            for j in range(3):
                for k in range(3):
                    if 1 in (i, j, k):
                        clutch[(cx + i, cy + j, cz + k)] = rng.choice(CRACK + FLAME[1:3] + ROCK[:1])
        parts[f"ember{n}"] = clutch
    pivots = {"body": (0, 0, 3), "head": (0, 0, 33), "flame": (0, 0, 42), "arm_L": (-12, 0, 29),
              "arm_R": (12, 0, 29), **{f"ember{n}": (0, 0, 24) for n in range(3)}}
    pose = {"arm_L": (0, 8, 0), "arm_R": (0, -8, 0), "head": (4, 0, 8)}
    return parts, pivots, pose


# ------------------------------------------------------------------ Rafaleur (fall)

def rafaleur():
    rng = random.Random(5)
    FEATHER = ["#2b2d5c", "#262852", "#323668"]
    STRIPE = ["#5c66b8", "#6b76c9"]
    TIP = "#141530"
    MASK = ["#e9e4d8", "#f4f0e6"]
    CHEST = ["#8f8fb8", "#a3a2c9"]
    EYE, PUPIL, BEAK, TALON = "*#ffd23f", "#140e05", "#d0a24a", "#c9b36a"
    feather = lambda *_: rng.choice(FEATHER)
    parts = {}
    body = {}
    box(body, range(-4, 4), range(-3, 4), range(10, 21), feather)
    for i in range(-3, 3):
        for k in range(11, 19):
            body[(i, -3, k)] = rng.choice(CHEST) if (i + k) % 2 else CHEST[0]
    for i in range(-3, 3):  # tail fan
        for j in range(4, 8):
            body[(i, j, 10 - (j - 4) // 2)] = TIP if j == 7 else rng.choice(FEATHER)
    for side in (-3, 1):  # talons
        for i in (side, side + 1):
            box(body, range(i, i + 1), range(-2, 0), range(7, 10), TALON)
            body[(i, -3, 7)] = TALON
    parts["body"] = body

    head = {}
    box(head, range(-5, 5), range(-4, 3), range(20, 27), feather)
    for i in range(-4, 4):  # heart-shaped white mask
        for k in range(21, 26):
            if not (k == 25 and i in (-1, 0)) and not (k == 21 and abs(i + 0.5) > 2.5):
                head[(i, -5, k)] = rng.choice(MASK)
    for side in (-3, 1):
        head[(side, -6, 23)] = EYE
        head[(side + 1, -6, 23)] = EYE
        head[(side, -6, 24)] = EYE
        head[(side + 1, -6, 24)] = EYE
        head[(side + (1 if side < 0 else 0), -7, 23)] = PUPIL
    head[(-1, -6, 22)] = BEAK
    head[(0, -6, 22)] = BEAK
    head[(-1, -7, 21)] = BEAK
    for side in (-5, 3):  # ear tufts
        for i in (side, side + 1):
            head[(i, -1, 27)] = FEATHER[0]
        head[(side if side < 0 else side + 1, -1, 28)] = TIP
        head[(side if side < 0 else side + 1, -1, 29)] = TIP
    parts["head"] = head

    wing = {}
    for i in range(-20, -4):
        span = -4 - i  # 1..16 outward
        top = 21 - span // 6
        bottom = 13 + span // 3 + (1 if i % 2 == 0 and span > 6 else 0)
        for j in range(-2, 3):
            if span > 12 and j == 2:
                continue
            for k in range(bottom, top + 1):
                c = TIP if k == bottom and span > 4 else (rng.choice(STRIPE) if k == top - 2 else rng.choice(FEATHER))
                wing[(i, j, k)] = c
    parts["wing_L"] = wing
    parts["wing_R"] = mirror(wing)
    pivots = {"body": (0, 0.5, 14), "head": (0, -0.5, 20), "wing_L": (-4, 0.5, 19), "wing_R": (4, 0.5, 19)}
    pose = {"wing_L": (0, 18, 0), "wing_R": (0, -18, 0), "head": (6, 0, 12)}
    return parts, pivots, pose



# ------------------------------------------------------------------ Ember (the Cinderling's throw)

def braise():
    """A glowing 2x2x2 ember, centred 2 voxels above the origin."""
    rng = random.Random(17)
    GLOW = ["*#ffd24a", "*#ff8c1a", "*#ff6a00"]
    ember = {}
    box(ember, range(-1, 1), range(-1, 1), range(1, 3), lambda *_: rng.choice(GLOW))
    return {"ember": ember}, {"ember": (0, 0, 2)}, {}


# ------------------------------------------------------------------ Arrow (the players' bow)

def fleche():
    """An arrow 14 voxels long, centred 2 voxels above the origin: a flint point, a shaft and
    four feathers."""
    rng = random.Random(19)
    SHAFT = ["#a9794a", "#946739", "#8a5f34"]
    FEATHER, SHADE = "#e6e1d6", "#a8a197"
    FLINT, EDGE = "#4a5058", "#79828c"
    arrow = {}
    for j in range(-6, 8):
        arrow[(0, j, 2)] = rng.choice(SHAFT)
    arrow[(0, -7, 2)] = EDGE
    arrow[(0, -6, 2)] = FLINT
    for i, k in ((-1, 2), (1, 2), (0, 1), (0, 3)):
        arrow[(i, -5, k)] = FLINT
        for j in (4, 5, 6):
            arrow[(i, j, k)] = SHADE if j == 5 else FEATHER
    return {"arrow": arrow}, {"arrow": (0, 0, 2)}, {}


def clips_fleche():
    still = {"arrow": [(0, None, None), (24, None, None)]}
    return {"Attack": still, "Idle": still, "Walk": still}


# ------------------------------------------------------------------ Grignoteur (cave rat, packs)

def grignoteur():
    rng = random.Random(13)
    FUR = ["#5b5249", "#4f473f", "#665c52"]
    BELLY = "#8a7f72"
    PINK, TOOTH, EYE, CLAW = "#c98f8f", "#efe6c8", "*#ff3b3b", "#2a2420"
    fur = lambda *_: rng.choice(FUR)
    parts = {}
    body = {}
    box(body, range(-3, 3), range(-4, 5), range(3, 8), fur)
    for i in range(-2, 2):
        for j in range(-3, 4):
            body[(i, j, 3)] = BELLY
    for j in range(-3, 5, 2):  # ragged fur ridge along the spine
        body[(rng.choice((-1, 0)), j, 8)] = FUR[1]
    parts["body"] = body
    head = {}
    box(head, range(-2, 2), range(-9, -4), range(4, 8), fur)
    box(head, range(-1, 1), range(-10, -9), range(4, 7), fur)
    head[(-1, -11, 5)] = PINK
    head[(0, -11, 5)] = PINK
    head[(-1, -10, 4)] = TOOTH
    head[(0, -10, 4)] = TOOTH
    head[(-1, -10, 3)] = TOOTH
    head[(0, -10, 3)] = TOOTH
    head[(-2, -8, 7)] = EYE
    head[(1, -8, 7)] = EYE
    for i in (-3, 2):  # ears
        head[(i, -6, 8)] = PINK
        head[(i, -6, 9)] = FUR[0]
    parts["head"] = head
    for name, xr, yr in (("leg_FL", range(-3, -1), range(-4, -2)), ("leg_FR", range(1, 3), range(-4, -2)),
                         ("leg_BL", range(-3, -1), range(2, 4)), ("leg_BR", range(1, 3), range(2, 4))):
        leg = {}
        box(leg, xr, yr, range(0, 3), lambda i, j, k: CLAW if k == 0 else fur())
        parts[name] = leg
    tail = {}
    for n, j in enumerate(range(5, 13)):
        tail[(0, j, 4 + (n // 3))] = PINK
    parts["tail"] = tail
    pivots = {"body": (0, 0, 5), "head": (0, -4.5, 6), "tail": (0.5, 5, 4.5),
              "leg_FL": (-2, -3, 3), "leg_FR": (2, -3, 3), "leg_BL": (-2, 3, 3), "leg_BR": (2, 3, 3)}
    return parts, pivots, {"head": (10, 0, 12), "tail": (0, 0, 20)}


# ------------------------------------------------------------------ Ronceux (bramble ambusher)

def ronceux():
    rng = random.Random(17)
    LEAF = ["#2d4a22", "#365a28", "#24401c", "#3f6630"]
    THORN, BERRY, MAW, FANG = "#d9cfb0", "*#ff2f4f", "#140a08", "#e8e0c4"
    parts = {}
    body = {}
    for i in range(-7, 7):
        for j in range(-6, 7):
            for k in range(0, 9):
                r = ((i + 0.5) / 7.2) ** 2 + ((j - 0.5) / 6.7) ** 2 + ((k - 1) / 8.5) ** 2
                if r < 1 and rng.random() < 0.96:
                    body[(i, j, k)] = rng.choice(LEAF)
    for (i, j, k) in list(body):  # thorns poking out
        if rng.random() < 0.05 and k > 1:
            d = (1 if i >= 0 else -1, 0, 0) if abs(i) > abs(j) else (0, 1 if j >= 0 else -1, 0)
            n = (i + d[0], j + d[1], k)
            if n not in body:
                body[n] = THORN
    # the maw across the front, hidden in the leaves until it opens
    for i in range(-4, 4):
        for k in (4, 5):
            body.pop((i, -6, k), None)
            body.pop((i, -5, k), None)
            body[(i, -4, k)] = MAW
        if i % 2 == 0:
            body[(i, -5, 5)] = FANG
    parts["body"] = body
    lid = {}
    for i in range(-6, 6):
        for j in range(-6, 6):
            for k in range(9, 12):
                r = ((i + 0.5) / 6.2) ** 2 + (j / 6.2) ** 2 + ((k - 8) / 4) ** 2
                if r < 1:
                    lid[(i, j, k)] = rng.choice(LEAF)
    for (i, j, k) in [(-3, -5, 10), (2, -5, 10), (-4, -4, 11), (3, -4, 11)]:
        lid[(i, j, k)] = BERRY  # four glowing berry eyes
    for i in range(-4, 4, 2):
        lid[(i, -6, 9)] = FANG
    parts["lid"] = lid
    for n, (x, y) in enumerate(((-8, -3), (7, -3), (0, 7))):
        vine = {}
        for k in range(1, 8):
            vine[(x, y, k)] = LEAF[k % 4]
            if k % 2 == 0:
                vine[(x + (1 if x < 0 else -1 if x > 0 else 0), y, k)] = THORN
        parts[f"vine{n}"] = vine
    pivots = {"body": (0, 0, 0), "lid": (0, 5, 9), "vine0": (-8, -3, 1), "vine1": (7, -3, 1), "vine2": (0, 7, 1)}
    return parts, pivots, {"lid": (-28, 0, 0), "vine0": (0, -20, 0), "vine1": (0, 25, 0)}


# ------------------------------------------------------------------ Rouleur (rolling boulder)

def rouleur():
    rng = random.Random(19)
    STONE = ["#7a7a80", "#6c6c72", "#86868c", "#5e5e64"]
    MOSS = ["#5d7a2e", "#4b6526"]
    EYE, PUPIL, CRACK = "*#2f8fe0", "#0b1a26", "#2c2c31"
    parts = {}
    body = {}
    c = 5.5
    for i in range(-6, 6):
        for j in range(-6, 6):
            for k in range(0, 12):
                r = max(abs(i + 0.5), abs(j + 0.5), abs(k - c)) + 0.35 * (
                    (abs(i + 0.5) > 4) + (abs(j + 0.5) > 4) + (abs(k - c) > 4))
                if r <= 6.2:
                    body[(i, j, k)] = MOSS[rng.random() < 0.5] if k >= 10 and rng.random() < 0.6 else rng.choice(STONE)
    for i in range(-2, 2):  # one big eye on the front
        for k in range(5, 9):
            body[(i, -6, k)] = EYE
    body[(-1, -7, 6)] = PUPIL
    body[(0, -7, 6)] = PUPIL
    body[(-1, -7, 7)] = PUPIL
    body[(0, -7, 7)] = PUPIL
    for (i, k) in ((-4, 3), (-3, 2), (-2, 3), (-1, 2), (0, 3), (1, 2), (2, 3), (3, 2)):
        body[(i, -6, k)] = CRACK  # cracked grin
    for (i, j, k) in ((5, -2, 4), (5, -1, 5), (5, 0, 6), (-6, 2, 7), (-6, 3, 8)):
        body[(i, j, k)] = CRACK
    parts["body"] = body
    return parts, {"body": (0, 0, 6)}, {"body": (8, 0, -10)}

# ------------------------------------------------------------------ the clips
#
# A clip is {part: [(frame, offset in voxels or None, rotation in degrees or None)]}; a part a
# clip leaves out is keyed at rest, so that no clip keeps the pose of the one before it.

def swing(amp, period=24, phase=0, axis=0, frames=24, steps=8):
    """A sine swing of `amp` degrees around `axis`, sampled `steps` times over `frames`."""
    out = []
    for s in range(steps + 1):
        f = frames * s / steps
        a = amp * math.sin(2 * math.pi * (f / period) + phase)
        rot = [0, 0, 0]
        rot[axis] = a
        out.append((round(f), None, tuple(rot)))
    return out


def bob(amp, period=24, frames=24, steps=8, phase=0, rot=None):
    return [(round(frames * s / steps), (0, 0, amp * math.sin(2 * math.pi * (frames * s / steps) / period + phase)), rot)
            for s in range(steps + 1)]


def keys(*pairs):
    """(frame, rotation) pairs, with no offset."""
    return [(f, None, r) for f, r in pairs]


PI = math.pi


def clips_souchard():
    walk = {"leg_L": swing(25), "leg_R": swing(25, phase=PI), "arm_L": swing(18, phase=PI),
            "arm_R": swing(18), "body": bob(-0.6, period=12), "head": swing(4, period=12, axis=2)}
    idle = {"body": bob(0.3, period=36, frames=72), "head": keys((0, (0, 0, 0)), (18, (0, 0, 18)), (36, (0, 0, 18)),
                                                                 (54, (0, 0, -14)), (72, (0, 0, 0))),
            "arm_L": swing(5, period=72, frames=72), "arm_R": swing(5, period=72, frames=72, phase=PI)}
    attack = {"arm_L": keys((0, (0, 0, 0)), (5, (-150, 0, 10)), (9, (-40, 0, 0)), (16, (0, 0, 0))),
              "arm_R": keys((0, (0, 0, 0)), (5, (-150, 0, -10)), (9, (-40, 0, 0)), (16, (0, 0, 0))),
              "body": keys((0, (0, 0, 0)), (5, (8, 0, 0)), (9, (-12, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


def clips_noyeur():
    walk = {"leg_L": swing(22), "leg_R": swing(22, phase=PI), "body": swing(6, period=24, axis=1),
            "arm_L": swing(14, phase=PI), "arm_R": swing(14), "head": swing(5, period=12)}
    idle = {"head": keys((0, (0, 0, 0)), (20, (-8, 0, 15)), (40, (6, 0, -12)), (72, (0, 0, 0))),
            "body": bob(0.4, period=36, frames=72), "arm_L": swing(6, period=72, frames=72),
            "arm_R": swing(6, period=72, frames=72, phase=1)}
    attack = {"arm_L": keys((0, (0, 0, 0)), (4, (-95, 0, -15)), (8, (-80, 0, 10)), (16, (0, 0, 0))),
              "arm_R": keys((0, (0, 0, 0)), (4, (-95, 0, 15)), (8, (-80, 0, -10)), (16, (0, 0, 0))),
              "body": keys((0, (0, 0, 0)), (4, (-15, 0, 0)), (8, (20, 0, 0)), (16, (0, 0, 0))),
              "head": keys((0, (0, 0, 0)), (4, (-20, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


def ember_orbit(frames, turns=1, tilt=0):
    return [(round(frames * s / 8), None, (tilt, 0, 360 * turns * s / 8)) for s in range(9)]


def clips_braisille(parts):
    def base(frames, tilt):
        c = {"body": bob(0.8, period=frames, frames=frames, rot=(tilt, 0, 0)),
             "flame": swing(10, period=frames / 2, frames=frames, axis=1),
             "shard": bob(-0.5, period=frames, frames=frames)}
        for n in range(4):
            c[f"ember{n}"] = ember_orbit(frames, turns=1 if n % 2 == 0 else -1, tilt=tilt)
        return c
    idle = base(72, 0)
    walk = base(24, 12)
    attack = {"body": keys((0, (0, 0, 0)), (5, (22, 0, 0)), (8, (-25, 0, 0)), (16, (0, 0, 0))),
              "flame": keys((0, (0, 0, 0)), (5, (-25, 0, 0)), (8, (30, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


def clips_mere():
    def base(frames, tilt, sway):
        return {"body": bob(0.7, period=frames, frames=frames, rot=(tilt, 0, 0)),
                "head": bob(0.9, period=frames, frames=frames, rot=(tilt, 0, 0)),
                "flame": swing(9, period=frames / 2, frames=frames, axis=1),
                "arm_L": swing(sway, period=frames, frames=frames),
                "arm_R": swing(-sway, period=frames, frames=frames),
                **{f"ember{n}": ember_orbit(frames, turns=1 if n != 1 else -1) for n in range(3)}}
    attack = {"arm_L": keys((0, (0, 0, 0)), (5, (-130, 12, 0)), (9, (35, 0, 0)), (16, (0, 0, 0))),
              "arm_R": keys((0, (0, 0, 0)), (5, (-130, -12, 0)), (9, (35, 0, 0)), (16, (0, 0, 0))),
              "head": keys((0, (0, 0, 0)), (5, (-18, 0, 0)), (9, (20, 0, 0)), (16, (0, 0, 0))),
              "flame": keys((0, (0, 0, 0)), (5, (-25, 0, 0)), (9, (30, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": base(72, 0, 3), "Walk": base(36, 4, 10)}


def clips_rafaleur():
    flap = lambda amp, period, frames: {"wing_L": swing(amp, period=period, frames=frames, axis=1),
                                        "wing_R": swing(-amp, period=period, frames=frames, axis=1)}
    idle = {**flap(14, 36, 72), "body": bob(1.0, period=36, frames=72),
            "head": keys((0, (0, 0, 0)), (24, (0, 0, 25)), (48, (0, 0, -25)), (72, (0, 0, 0)))}
    walk = {**flap(35, 12, 24), "body": bob(1.5, period=12, frames=24, rot=(12, 0, 0))}
    attack = {"wing_L": keys((0, (0, 0, 0)), (5, (0, -70, 0)), (10, (0, 50, 0)), (16, (0, 0, 0))),
              "wing_R": keys((0, (0, 0, 0)), (5, (0, 70, 0)), (10, (0, -50, 0)), (16, (0, 0, 0))),
              "body": keys((0, (0, 0, 0)), (5, (40, 0, 0)), (10, (-10, 0, 0)), (16, (0, 0, 0))),
              "head": keys((0, (0, 0, 0)), (5, (-30, 0, 0)), (16, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


def clips_braise():
    spin = {"ember": [(f, None, (360 * f / 12, 0, 360 * f / 24)) for f in range(0, 25, 3)]}
    return {"Attack": spin, "Idle": spin, "Walk": spin}


def clips_grignoteur():
    walk = {"leg_FL": swing(35, period=8), "leg_BR": swing(35, period=8), "leg_FR": swing(35, period=8, phase=PI),
            "leg_BL": swing(35, period=8, phase=PI), "tail": swing(20, period=8, axis=2),
            "body": bob(-0.4, period=4)}
    idle = {"head": keys((0, (0, 0, 0)), (6, (-10, 0, 0)), (12, (0, 0, 0)), (18, (-10, 0, 0)), (24, (0, 0, 0)),
                         (48, (0, 0, 20)), (72, (0, 0, 0))),
            "tail": swing(25, period=36, frames=72, axis=2)}
    attack = {"head": [(0, (0, 0, 0), (0, 0, 0)), (3, (0, -2, 0), (25, 0, 0)), (6, (0, 0, 0), (-10, 0, 0)), (12, (0, 0, 0), (0, 0, 0))],
              "body": [(0, (0, 0, 0), None), (3, (0, -1.5, 0), None), (12, (0, 0, 0), None)]}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


def clips_ronceux():
    vines = lambda amp, period, frames: {"vine0": swing(amp, period=period, frames=frames, axis=1),
                                         "vine1": swing(-amp, period=period, frames=frames, axis=1, phase=1),
                                         "vine2": swing(amp, period=period, frames=frames, axis=0, phase=2)}
    idle = {**vines(6, 72, 72), "body": swing(1.5, period=72, frames=72, axis=1)}
    walk = {**vines(20, 12, 24), "body": swing(4, period=12, frames=24, axis=1)}
    attack = {"lid": keys((0, (0, 0, 0)), (4, (-55, 0, 0)), (9, (-55, 0, 0)), (12, (0, 0, 0)), (16, (0, 0, 0))),
              "vine0": keys((0, (0, 0, 0)), (6, (-60, -30, 0)), (16, (0, 0, 0))),
              "vine1": keys((0, (0, 0, 0)), (6, (-60, 30, 0)), (16, (0, 0, 0))),
              "body": keys((0, (0, 0, 0)), (4, (-6, 0, 0)), (12, (0, 0, 0)))}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


def clips_rouleur():
    walk = {"body": [(f, (0, 0, abs(math.sin(math.pi * f / 12)) * 0.8), (-360 * f / 24, 0, 0)) for f in range(0, 25, 3)]}
    idle = {"body": keys((0, (0, 0, 0)), (18, (0, 0, 4)), (36, (0, 0, 0)), (54, (0, 0, -4)), (72, (0, 0, 0)))}
    attack = {"body": [(0, (0, 0, 0), (0, 0, 0)), (4, (0, 2, 0), (25, 0, 0)), (8, (0, -3, 1.5), (-40, 0, 0)), (16, (0, 0, 0), (0, 0, 0))]}
    return {"Attack": attack, "Idle": idle, "Walk": walk}


# name used in the game, builder, clips, linear clips (looping turns)
MOBS = {
    "Stumpling": ("Souchard", souchard, clips_souchard),
    "Drowner": ("Noyeur", noyeur, clips_noyeur),
    "Cinderling": ("Braisille", braisille, None),
    "GustOwl": ("Rafaleur", rafaleur, clips_rafaleur),
    "Gnawer": ("Grignoteur", grignoteur, clips_grignoteur),
    "Bramble": ("Ronceux", ronceux, clips_ronceux),
    "Roller": ("Rouleur", rouleur, clips_rouleur),
    "EmberMother": ("Mere", mere, clips_mere),
    "Ember": ("Braise", braise, clips_braise),
    "Arrow": ("Fleche", fleche, clips_fleche),
}
LINEAR = {"Cinderling", "EmberMother", "Roller", "Ember"}


# ------------------------------------------------------------------ the objects

def materials(colours, name):
    """Two materials over one palette texture, one pixel per colour: plain, and glowing for the
    colours marked with a `*`."""
    image = bpy.data.images.new(f"{name}_palette", P, P, alpha=False)
    px = [0.0] * (P * P * 4)
    index = {}
    for n, colour in enumerate(colours):
        px[n * 4:n * 4 + 4] = [c / 255 for c in bytes.fromhex(colour.lstrip("*#"))] + [1.0]
        index[colour] = n
    image.pixels.foreach_set(px)
    # Saved to a file so that the exporter embeds the pixels rather than a blank generated image
    image.filepath_raw = str(Path(tempfile.mkdtemp()) / f"{name}_palette.png")
    image.file_format = "PNG"
    image.save()
    mats = []
    for glow in (False, True):
        m = bpy.data.materials.new(name + ("_glow" if glow else ""))
        nt = m.node_tree
        bsdf = next(n for n in nt.nodes if n.type == "BSDF_PRINCIPLED")
        bsdf.inputs["Roughness"].default_value = 0.85
        tex = nt.nodes.new("ShaderNodeTexImage")
        tex.image = image
        tex.interpolation = "Closest"
        nt.links.new(tex.outputs["Color"], bsdf.inputs["Base Color"])
        if glow:
            nt.links.new(tex.outputs["Color"], bsdf.inputs["Emission Color"])
            bsdf.inputs["Emission Strength"].default_value = 2.0
        mats.append(m)
    return mats, index


def part_object(name, cells, pivot, mats, index, coll):
    """The visible faces of `cells` as one mesh, with its origin on `pivot`."""
    bm = bmesh.new()
    uv = bm.loops.layers.uv.new("UVMap")
    px, py, pz = pivot
    for (i, j, k), colour in cells.items():
        n = index[colour]
        u, v = ((n % P) + 0.5) / P, ((n // P) + 0.5) / P
        for (dx, dy, dz), corners in FACES:
            if (i + dx, j + dy, k + dz) in cells:
                continue
            f = bm.faces.new([bm.verts.new(((i + cx - px) * VOX, (j + cy - py) * VOX, (k + cz - pz) * VOX))
                              for cx, cy, cz in corners])
            f.material_index = 1 if colour.startswith("*") else 0
            for loop in f.loops:
                loop[uv].uv = (u, v)
    bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=VOX / 1000)
    mesh = bpy.data.meshes.new(name)
    bm.to_mesh(mesh)
    bm.free()
    for m in mats:
        mesh.materials.append(m)
    obj = bpy.data.objects.new(name, mesh)
    obj.location = (px * VOX, py * VOX, pz * VOX)
    coll.objects.link(obj)
    return obj


def build(name, coll, posed=False):
    """The mob as an empty holding one object per part; `posed` turns the parts to the pose of
    the previews."""
    _, builder, _ = MOBS[name]
    parts, pivots, pose = builder()
    mats, index = materials(sorted({c for cells in parts.values() for c in cells.values()}), name)
    root = bpy.data.objects.new(name, None)
    coll.objects.link(root)
    objects = {}
    for part, cells in parts.items():
        obj = part_object(f"{name}_{part}", cells, pivots[part], mats, index, coll)
        obj.parent = root
        if posed:
            obj.rotation_euler = tuple(math.radians(a) for a in pose.get(part, (0, 0, 0)))
        objects[part] = obj
    return root, objects


def add_clip(obj, clip, keyframes, linear):
    """Keys `obj` from its rest pose, then moves the action to an NLA track named after the clip,
    which the exporter turns into the glTF animation of that name."""
    rest = obj.location.copy()
    obj.animation_data_create()
    obj.animation_data.action = None
    for frame, offset, rotation in keyframes:
        obj.location = tuple(r + o * VOX for r, o in zip(rest, offset or (0, 0, 0)))
        obj.rotation_euler = tuple(math.radians(a) for a in (rotation or (0, 0, 0)))
        obj.keyframe_insert("location", frame=frame)
        obj.keyframe_insert("rotation_euler", frame=frame)
    obj.location = rest
    obj.rotation_euler = (0, 0, 0)
    action = obj.animation_data.action
    action.name = f"{obj.name}_{clip}"
    if linear:
        for layer in getattr(action, "layers", []):
            for strip in layer.strips:
                for bag in strip.channelbags:
                    for fc in bag.fcurves:
                        for kp in fc.keyframe_points:
                            kp.interpolation = "LINEAR"
        for fc in getattr(action, "fcurves", []):
            for kp in fc.keyframe_points:
                kp.interpolation = "LINEAR"
    track = obj.animation_data.nla_tracks.new()
    track.name = clip
    track.strips.new(clip, 0, action)
    obj.animation_data.action = None


def export(name, path):
    scene = empty_scene()
    root, objects = build(name, scene.collection)
    _, builder, clips = MOBS[name]
    clips = clips() if clips else clips_braisille(objects)
    length = 0
    for clip, per_part in sorted(clips.items()):
        clip_len = max(f for ks in per_part.values() for f, _, _ in ks)
        length = max(length, clip_len)
        for part, obj in objects.items():
            ks = per_part.get(part) or [(0, None, None), (clip_len, None, None)]
            add_clip(obj, clip, ks, name in LINEAR)
    for obj in scene.objects:
        obj.select_set(obj is root or obj.parent is root)
    path.parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.export_scene.gltf(filepath=str(path), export_format="GLB", use_selection=True,
                              use_active_scene=True, export_animations=True,
                              export_animation_mode="NLA_TRACKS",
                              export_optimize_animation_keep_anim_object=True)
    faces = sum(len(o.data.polygons) for o in objects.values())
    print(f"wrote {path}: {faces} faces, clips {sorted(clips)}")


def empty_scene():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps = FPS
    return scene


# ------------------------------------------------------------------ previews

def set_scene():
    scene = empty_scene()
    try:
        scene.render.engine = "CYCLES"
    except TypeError as e:
        print(e)
    scene.cycles.device = "CPU"
    scene.cycles.samples = 96
    scene.cycles.use_denoising = True
    scene.view_settings.view_transform = "Standard"
    world = bpy.data.worlds.new("Night")
    scene.world = world
    bg = next(n for n in world.node_tree.nodes if n.type == "BACKGROUND")
    bg.inputs["Color"].default_value = (0.035, 0.045, 0.09, 1)
    bg.inputs["Strength"].default_value = 0.6
    return scene


def ground(coll, size=3):
    """A patch of grass blocks."""
    rng = random.Random(1)
    cells = {}
    half = size * 8
    for i in range(-half, half):
        for j in range(-half, half):
            for k in range(-4, 0):
                cells[(i, j, k)] = rng.choice(["#4f7d34", "#5a8a3a", "#47722f"]) if k == -1 else rng.choice(["#6b4a2f", "#5e4029"])
    mats, index = materials(sorted(set(cells.values())), "Ground")
    return part_object("ground", cells, (0, 0, 0), mats, index, coll)


def lights(coll, warm=False):
    def area(name, energy, size, colour, location):
        light = bpy.data.lights.new(name, "AREA")
        light.energy, light.size, light.color = energy, size, colour
        obj = bpy.data.objects.new(name, light)
        obj.location = location
        obj.rotation_euler = (Vector((0, 0, 0.9)) - obj.location).to_track_quat("-Z", "Y").to_euler()
        coll.objects.link(obj)
    moon = bpy.data.lights.new("moon", "SUN")
    moon.energy, moon.color = 2.2, (0.75, 0.82, 1.0)
    obj = bpy.data.objects.new("moon", moon)
    obj.rotation_euler = (math.radians(50), math.radians(-25), math.radians(-35))
    coll.objects.link(obj)
    area("rim", 140, 2, (1.0, 0.62, 0.35) if warm else (0.55, 0.7, 1.0), (1.8, 2.6, 2.4))
    area("fill", 60, 3, (1, 1, 1), (-2.5, -3.0, 1.6))


def camera(scene, coll, target, dist, height, angle=-35, lens=50):
    cam = bpy.data.objects.new("cam", bpy.data.cameras.new("cam"))
    cam.data.lens = lens
    a = math.radians(angle)
    cam.location = (math.sin(a) * dist, -math.cos(a) * dist, height)
    cam.rotation_euler = (Vector(target) - cam.location).to_track_quat("-Z", "Y").to_euler()
    coll.objects.link(cam)
    scene.camera = cam


def render(scene, path, w, h):
    scene.render.resolution_x, scene.render.resolution_y = w, h
    scene.render.filepath = str(path)
    scene.render.image_settings.file_format = "PNG"
    bpy.ops.render.render(write_still=True)


# height of the centre of the frame, and lift off the ground, of each preview
FRAMING = {"Stumpling": (1.0, 0), "Drowner": (0.75, 0), "Cinderling": (0.95, 0), "GustOwl": (1.05, 0.55),
           "Gnawer": (0.3, 0), "Bramble": (0.4, 0), "Roller": (0.4, 0), "Ember": (0.3, 0.2),
           "Arrow": (0.3, 0.2), "EmberMother": (1.5, 0)}


def previews(out, names):
    out.mkdir(parents=True, exist_ok=True)
    for name in names:
        scene = set_scene()
        root, _ = build(name, scene.collection, posed=True)
        centre, lift = FRAMING[name]
        root.location.z = lift
        ground(scene.collection)
        lights(scene.collection, warm=name == "Cinderling")
        small = centre < 0.5
        camera(scene, scene.collection, (0, 0, centre), 2.6 if small else (5.6 if name == "GustOwl" else 7.5 if name == "EmberMother" else 4.6), centre + (0.45 if small else 0.55))
        render(scene, out / f"{name}.png", 900, 900)
    scene = set_scene()
    for name, x in (("GustOwl", -4.6), ("Stumpling", -2.4), ("Drowner", -0.5), ("Cinderling", 1.3),
                    ("Gnawer", 2.8), ("Bramble", 4.0), ("Roller", 5.5)):
        root, _ = build(name, scene.collection, posed=True)
        root.location = (x, 0, FRAMING[name][1])
    ground(scene.collection, size=14)
    lights(scene.collection)
    camera(scene, scene.collection, (0.5, 0, 0.9), 15.5, 2.3, angle=-8, lens=40)
    render(scene, out / "lineup.png", 1800, 700)


def main():
    if "--render" in ARGS:
        previews(Path(ARGS[ARGS.index("--render") + 1]), ARGS[ARGS.index("--render") + 2:] or list(MOBS))
        return
    for name in ARGS or MOBS:
        export(name, ROOT / "data" / "models" / "animated" / f"{name}.glb")


if __name__ == "__main__":
    main()
