"""Builds the voxel models of the animals other than the sheep, data/models/animated/<Kind>.glb.

They share the voxels, the palette and the way to move of the sheep (generate_sheep.py): rigid
parts of 1/16 m voxels, no skinning, feet on the origin, facing -Y in Blender. The exporter sorts
the clips by name: Eat (only for the animals that graze), Idle, Run and Walk, the indices that
client/src/mob/models.rs expects.

Usage: blender --background --factory-startup --python scripts/generate_animals.py -- [out_dir]
"""

import random
import sys
from pathlib import Path

import bpy

sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_sheep import FPS, ROOT, VOX, add_clip, box, deg, palette_material, part_object  # noqa: E402

ARGS = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
OUT_DIR = Path(ARGS[0]) if ARGS else ROOT / "data" / "models" / "animated"
EYE = "#1f1a1a"


class Animal:
    """The voxels of an animal by part, and where each part turns."""

    def __init__(self, name, seed, drop=0, hops=False):
        self.name, self.rng, self.drop, self.hops = name, random.Random(seed), drop, hops
        self.file = name
        self.parts, self.pivots = {}, {}

    def fur(self, *colours):
        """A colour drawn for each voxel, the first ones more often."""
        weights = [2 ** (len(colours) - n) for n in range(len(colours))]
        return lambda *_: self.rng.choices(colours, weights=weights)[0]

    def part(self, name, pivot):
        self.pivots[name] = pivot
        self.parts.setdefault(name, {})

    def box(self, part, xs, ys, zs, colour):
        paint = colour if callable(colour) else lambda *_: colour
        box(self.parts[part], range(*xs), range(*ys), range(*zs), paint)

    def mirror(self, part, xs, ys, zs, colour):
        """A box and its reflection on the other side of the animal."""
        self.box(part, xs, ys, zs, colour)
        self.box(part, (-xs[1], -xs[0]), ys, zs, colour)

    def legs(self, half, width, front, back, depth, top, colour):
        """Four legs, their outer side at `half`, the front and the back ones from those y."""
        for end, y in (("F", front), ("B", back)):
            for side, x in (("L", -half), ("R", half - width)):
                name = f"leg_{end}{side}"
                self.part(name, (x + width / 2, y + depth / 2, top))
                self.box(name, (x, x + width), (y, y + depth), (0, top), colour)

    def round_back(self, half, top):
        """Takes the long upper edges off the body."""
        for c in list(self.parts["body"]):
            if c[2] == top - 1 and c[0] in (-half, half - 1):
                del self.parts["body"][c]

    def eyes(self, x, y, z, colour=EYE):
        self.mirror("head", (x, x + 1), (y, y + 1), (z, z + 1), colour)


def banded(low, low_colour, colour):
    """`low_colour` under the height `low`, `colour` above."""
    return lambda i, j, k: low_colour if k < low else colour(i, j, k)


def fox():
    a = Animal("Fox", 1)
    red, cream, dark = a.fur("#c9652b", "#d4733a", "#b85822"), "#f0e4d2", "#3b2a26"
    a.part("body", (0, 0, 6))
    a.box("body", (-3, 3), (-5, 5), (4, 9), red)
    a.box("body", (-2, 2), (-5, 4), (4, 5), cream)
    a.part("head", (0, -5, 8))
    a.box("head", (-3, 3), (-9, -5), (6, 11), red)
    a.box("head", (-3, 3), (-9, -7), (6, 7), cream)
    a.box("head", (-1, 1), (-12, -9), (6, 7), cream)
    a.box("head", (-1, 1), (-12, -9), (7, 8), red)
    a.box("head", (-1, 1), (-12, -11), (7, 8), dark)
    a.mirror("head", (1, 3), (-7, -6), (11, 12), red)
    a.mirror("head", (2, 3), (-7, -6), (12, 13), dark)
    a.eyes(1, -9, 8)
    a.legs(3, 2, -5, 3, 2, 4, banded(3, dark, red))
    a.part("tail", (0, 5, 8))
    a.box("tail", (-1, 1), (5, 7), (6, 8), red)
    a.box("tail", (-2, 2), (7, 11), (5, 8), red)
    a.box("tail", (-1, 1), (11, 13), (6, 8), cream)
    return a


def cow():
    a = Animal("Cow", 2, drop=7)
    white = a.fur("#eee8dc", "#e4dccd")
    brown = a.fur("#8a4f2c", "#7d4526", "#965a35")
    pink, hoof, horn = "#dba493", "#463a33", "#d8c9a4"
    spots = [(a.rng.randint(-6, 5), a.rng.randint(-9, 8), a.rng.randint(9, 19), a.rng.uniform(3, 4.5))
             for _ in range(7)]

    def patched(i, j, k):
        spotted = any((i - x) ** 2 + (j - y) ** 2 + (k - z) ** 2 < r * r for x, y, z, r in spots)
        return brown() if spotted else white()

    a.part("body", (0, 0, 14))
    a.box("body", (-6, 6), (-9, 9), (9, 19), patched)
    a.round_back(6, 19)
    a.box("body", (-2, 2), (2, 4), (8, 9), pink)
    a.part("head", (0, -9, 17))
    a.box("head", (-4, 4), (-14, -9), (14, 21), brown)
    a.box("head", (-1, 1), (-14, -13), (17, 21), white)
    a.box("head", (-3, 3), (-16, -14), (14, 17), pink)
    a.mirror("head", (1, 2), (-16, -15), (15, 16), "#7c4f45")
    a.mirror("head", (4, 6), (-11, -10), (18, 20), brown)
    a.mirror("head", (3, 4), (-12, -11), (21, 23), horn)
    a.eyes(2, -14, 18)
    a.legs(6, 4, -8, 4, 4, 9, banded(2, hoof, white))
    a.part("tail", (0, 9, 18))
    a.box("tail", (-1, 1), (9, 10), (10, 18), white)
    a.box("tail", (-1, 1), (9, 11), (8, 11), brown)
    return a


def pig():
    a = Animal("Pig", 3, drop=3)
    pink = a.fur("#e6a3a0", "#dd9895", "#ecb0ab")
    dark, snout, hoof = a.fur("#8d5d5c", "#83534f"), "#d07f82", "#6b4645"
    a.part("body", (0, 0, 9))
    a.box("body", (-5, 5), (-7, 7), (5, 13), lambda i, j, k: dark() if 2 <= j < 6 and i + k > 11 else pink())
    a.round_back(5, 13)
    a.part("head", (0, -7, 9))
    a.box("head", (-4, 4), (-12, -7), (6, 13), pink)
    a.box("head", (-2, 2), (-14, -12), (7, 10), snout)
    a.mirror("head", (1, 2), (-14, -13), (8, 9), "#7c4548")
    a.mirror("head", (3, 5), (-11, -10), (11, 14), dark)
    a.eyes(2, -12, 10)
    a.legs(5, 3, -6, 3, 3, 5, banded(1, hoof, pink))
    a.part("tail", (0, 7, 11))
    a.box("tail", (0, 1), (7, 9), (10, 11), snout)
    a.box("tail", (0, 1), (8, 9), (11, 12), snout)
    return a


def chicken():
    a = Animal("Chicken", 4, drop=3)
    russet = a.fur("#a85a2a", "#b9682f", "#98502a")
    gold, black = a.fur("#d99a45", "#cf8d3a"), a.fur("#2b2a30", "#353842")
    yellow, red = "#dda136", "#c5352f"
    a.part("body", (0, 0, 7))
    a.box("body", (-3, 3), (-3, 4), (4, 9), russet)
    a.box("body", (-2, 2), (-4, -3), (4, 8), gold)
    a.part("tail", (0, 4, 8))
    a.box("tail", (-2, 2), (4, 5), (7, 11), black)
    a.box("tail", (-1, 1), (5, 6), (8, 12), black)
    a.part("head", (0, -4, 8))
    a.box("head", (-2, 2), (-6, -4), (8, 13), gold)
    a.box("head", (-1, 1), (-8, -6), (10, 11), yellow)
    a.box("head", (-1, 1), (-7, -6), (9, 10), red)
    a.box("head", (-1, 1), (-6, -4), (13, 14), red)
    a.eyes(1, -6, 11)
    for side, x, wing_x, shoulder in (("L", -2, -4, -3), ("R", 1, 3, 3)):
        leg, wing = f"leg_{side}", f"wing_{side}"
        a.part(leg, (x + 0.5, 0.5, 4))
        a.box(leg, (x, x + 1), (0, 1), (0, 4), yellow)
        a.box(leg, (x, x + 1), (-2, 0), (0, 1), yellow)
        a.part(wing, (shoulder, 0, 8))
        a.box(wing, (wing_x, wing_x + 1), (-2, 3), (5, 8), banded(6, "#7d4122", russet))
    return a


def rabbit():
    a = Animal("Rabbit", 5, drop=1, hops=True)
    fur, white = a.fur("#9a8468", "#8d785d", "#a68f72"), "#ece5d8"
    a.part("body", (0, 0, 4))
    a.box("body", (-2, 2), (-3, 3), (2, 6), fur)
    a.part("head", (0, -3, 6))
    a.box("head", (-2, 2), (-6, -3), (4, 8), fur)
    a.box("head", (-1, 1), (-7, -6), (4, 5), white)
    a.box("head", (-1, 1), (-7, -6), (5, 6), "#c98685")
    a.mirror("head", (1, 2), (-4, -3), (8, 12), fur)
    a.eyes(1, -6, 6)
    a.legs(2, 1, -3, 0, 1, 2, fur)
    for side, x in (("L", -2), ("R", 1)):
        a.box(f"leg_B{side}", (x, x + 1), (0, 3), (0, 1), fur)
    a.part("tail", (0, 3, 4))
    a.box("tail", (-1, 1), (3, 5), (3, 5), white)
    return a


def wolf():
    a = Animal("Wolf", 6)
    grey, dark = a.fur("#7c828a", "#888e95", "#70767e"), a.fur("#4b5058", "#444850")
    pale, nose = "#d9d6cf", "#25242a"
    backed = banded(11, None, dark)
    coat = lambda i, j, k: backed(i, j, k) or grey()
    a.part("body", (0, 0, 10))
    a.box("body", (-3, 3), (-7, 7), (7, 13), coat)
    a.box("body", (-4, 4), (-7, -2), (8, 13), coat)
    a.box("body", (-2, 2), (-8, -7), (7, 10), pale)
    a.part("head", (0, -7, 12))
    a.box("head", (-3, 3), (-12, -8), (10, 15), banded(14, "#7c828a", dark))
    a.box("head", (-2, 2), (-8, -7), (10, 14), grey)
    a.box("head", (-3, 3), (-12, -10), (10, 12), pale)
    a.box("head", (-1, 1), (-15, -12), (10, 12), pale)
    a.box("head", (-1, 1), (-15, -12), (12, 13), grey)
    a.box("head", (-1, 1), (-15, -14), (12, 13), nose)
    a.mirror("head", (1, 3), (-10, -9), (15, 17), dark)
    a.eyes(1, -12, 12, "#d5a637")
    a.legs(3, 2, -6, 5, 2, 7, banded(3, pale, grey))
    a.part("tail", (0, 7, 12))
    a.box("tail", (-1, 1), (7, 10), (9, 12), dark)
    a.box("tail", (-1, 1), (10, 14), (8, 11), grey)
    return a


# The coats of the horses (`shared::world::Coat`): the body, then the mane and the tail.
COATS = {
    "Bay": (("#8b4d2b", "#7f4426", "#965631"), ("#2a211f", "#332927")),
    "Chestnut": (("#a5582a", "#9a4f22", "#b2632f"), ("#7a3a1a", "#6e3416")),
    "Black": (("#2b2523", "#241f1d", "#332c29"), ("#141010", "#1c1717")),
    "Grey": (("#9a9a96", "#8c8c88", "#a8a8a3"), ("#4a4a48", "#575754")),
    "White": (("#e8e4dc", "#ddd8ce", "#f1eee8"), ("#cfc9bd", "#bdb6a9")),
    "Dun": (("#c8a96e", "#bb9c62", "#d3b67b"), ("#3a2c22", "#45362a")),
}


def horse(coat="Bay"):
    """A horse of `coat`, its saddle a part of its own, which the game shows on a saddled one;
    the bay one is Horse.glb, the others Horse<Coat>.glb, all of the same parts, which the clips
    of Horse.glb move."""
    a = Animal("Horse", 7, drop=8)
    a.file = "Horse" if coat == "Bay" else f"Horse{coat}"
    body, hair = COATS[coat]
    bay = a.fur(*body)
    mane, hoof, white, muzzle = a.fur(*hair), "#352b27", "#ebe5da", "#4c3028"
    a.part("body", (0, 0, 16))
    a.box("body", (-5, 5), (-10, 10), (12, 21), bay)
    a.round_back(5, 21)
    a.part("head", (0, -10, 20))
    a.box("head", (-2, 2), (-13, -8), (21, 29), bay)
    a.box("head", (-2, 2), (-13, -10), (16, 21), bay)
    a.box("head", (-1, 1), (-8, -7), (21, 30), mane)
    a.box("head", (-1, 1), (-14, -8), (29, 31), mane)
    a.box("head", (-3, 3), (-18, -13), (24, 29), bay)
    a.box("head", (-2, 2), (-21, -18), (24, 28), muzzle)
    a.box("head", (-1, 1), (-19, -14), (28, 29), white)
    a.mirror("head", (2, 3), (-14, -13), (29, 31), bay)
    a.eyes(2, -18, 27)
    a.legs(5, 3, -9, 6, 3, 12, lambda i, j, k: hoof if k < 2 else white if k < 4 else bay())
    a.part("tail", (0, 10, 19))
    a.box("tail", (-1, 1), (10, 12), (9, 20), mane)
    # The saddle on the back, its girth under the belly and its stirrups, moving with the body
    leather, dark, iron = "#5b3a22", "#3f2716", "#9aa0a6"
    a.part("saddle", (0, 0, 16))
    a.box("saddle", (-5, 5), (-4, 3), (21, 22), leather)
    a.box("saddle", (-3, 3), (-5, -4), (21, 23), dark)
    a.box("saddle", (-3, 3), (3, 4), (21, 23), dark)
    a.mirror("saddle", (5, 6), (-1, 1), (12, 22), dark)
    a.box("saddle", (-5, 5), (-1, 1), (11, 12), dark)
    a.mirror("saddle", (5, 6), (-1, 1), (9, 11), iron)
    # The horse armours, one part each, which the game shows on the horse that wears it: plates
    # on its flanks before and behind the girth, on its chest and over its rump
    for material, (plate, edge) in HORSE_ARMOURS.items():
        name = f"armour_{material}"
        shade = a.fur(plate, plate, edge)
        a.part(name, (0, 0, 16))
        a.mirror(name, (5, 6), (-10, -2), (13, 21), shade)
        a.mirror(name, (5, 6), (2, 10), (13, 21), shade)
        a.box(name, (-5, 5), (-11, -10), (12, 21), shade)
        a.box(name, (-5, 5), (4, 10), (21, 22), shade)
        a.box(name, (-1, 1), (-11, -10), (16, 18), edge)
    return a


# The horse armours (`ItemId::horse_armour`): the plates, then their edges.
HORSE_ARMOURS = {
    "leather": ("#8b5a2b", "#6e4520"),
    "iron": ("#c8ccd0", "#9ea3a8"),
    "gold": ("#e8c547", "#c09a22"),
    "diamond": ("#5fd3d0", "#3a9e9b"),
}


def long_eared(name, seed, half, length, top, legs, coat, mane, ears):
    """A donkey or a mule: a horse shorter of leg and of neck, with long ears, its saddle and
    the chest it carries on its flanks parts of their own, which the game shows on a saddled one
    and on one that carries a chest."""
    a = Animal(name, seed, drop=7)
    body, belly = a.fur(*coat), "#d8d0c4"
    hair, hoof, muzzle = a.fur(*mane), "#352b27", "#cfc6b8"
    a.part("body", (0, 0, (top + legs) // 2 + 2))
    a.box("body", (-half, half), (-length, length), (legs, top),
          lambda i, j, k: belly if k == legs else body())
    a.round_back(half, top)
    neck = -length
    a.part("head", (0, neck, top - 1))
    a.box("head", (-2, 2), (neck - 3, neck + 2), (top - 1, top + 6), body)
    a.box("head", (-1, 1), (neck + 2, neck + 3), (top, top + 6), hair)
    a.box("head", (-3, 3), (neck - 8, neck - 3), (top + 2, top + 7), body)
    a.box("head", (-2, 2), (neck - 10, neck - 8), (top + 2, top + 6), muzzle)
    a.mirror("head", (1, 3), (neck - 4, neck - 3), (top + 7, top + 7 + ears), body)
    a.mirror("head", (1, 2), (neck - 4, neck - 3), (top + 7, top + 6 + ears), hair)
    a.eyes(2, neck - 7, top + 5)
    a.legs(half, 3, -length + 1, length - 4, 3, legs,
           lambda i, j, k: hoof if k < 2 else body())
    a.part("tail", (0, length, top - 2))
    a.box("tail", (-1, 1), (length, length + 2), (legs - 2, top - 1), hair)
    leather, dark, iron = "#5b3a22", "#3f2716", "#9aa0a6"
    a.part("saddle", (0, 0, top - 1))
    a.box("saddle", (-half, half), (-4, 3), (top, top + 1), leather)
    a.box("saddle", (-3, 3), (-5, -4), (top, top + 2), dark)
    a.mirror("saddle", (half, half + 1), (-1, 1), (legs, top + 1), dark)
    a.box("saddle", (-half, half), (-1, 1), (legs - 1, legs), dark)
    # The chest, in two halves hung on its flanks behind the saddle, its lid and latch
    wood, trim = a.fur("#8b6a3c", "#80613a", "#94723f"), "#4a3520"
    a.part("chest", (0, 0, top - 1))
    a.mirror("chest", (half, half + 3), (2, 8), (legs + 1, top - 1), wood)
    a.mirror("chest", (half, half + 3), (2, 8), (top - 1, top), trim)
    a.mirror("chest", (half + 3, half + 4), (4, 6), (top - 4, top - 2), iron)
    return a


def donkey():
    return long_eared("Donkey", 11, 4, 8, 18, 9, ("#8a7d6e", "#7f7365", "#958878"),
                      ("#4a4038", "#40372f"), 5)


def mule():
    return long_eared("Mule", 12, 5, 9, 20, 11, ("#6b4a32", "#634430", "#75513a"),
                      ("#3a2a20", "#33241b"), 4)


def goat():
    a = Animal("Goat", 8, drop=6)
    coat = a.fur("#b9ab96", "#ad9f8a", "#c4b7a3")
    dark, horn, hoof = a.fur("#6b5848", "#614f41"), "#8a7a63", "#3d342e"
    a.part("body", (0, 0, 12))
    a.box("body", (-4, 4), (-6, 7), (8, 16), lambda i, j, k: dark() if i in (-1, 0) and k == 15 else coat())
    a.part("head", (0, -6, 15))
    a.box("head", (-3, 3), (-10, -6), (14, 20), coat)
    a.box("head", (-2, 2), (-13, -10), (14, 17), dark)
    a.box("head", (-1, 1), (-12, -11), (11, 14), "#e5ddcd")
    a.mirror("head", (3, 5), (-8, -7), (17, 18), dark)
    a.mirror("head", (1, 2), (-9, -7), (20, 23), horn)
    a.mirror("head", (1, 2), (-7, -5), (22, 24), horn)
    a.eyes(2, -10, 17)
    a.legs(4, 3, -5, 3, 3, 8, lambda i, j, k: hoof if k < 2 else dark() if k < 5 else coat())
    a.part("tail", (0, 7, 15))
    a.box("tail", (-1, 1), (7, 9), (14, 17), dark)
    return a


def cat():
    a = Animal("Cat", 9)
    plain, stripe = a.fur("#8f8376", "#988c7f"), a.fur("#574c43", "#5f544a")
    white = "#eee8df"
    a.part("body", (0, 0, 6))
    a.box("body", (-2, 2), (-5, 5), (4, 8), lambda i, j, k: stripe() if j % 3 == 0 and k > 4 else plain())
    a.box("body", (-1, 1), (-5, -4), (4, 7), white)
    a.part("head", (0, -5, 8))
    a.box("head", (-3, 3), (-9, -5), (6, 10), plain)
    a.box("head", (-1, 1), (-10, -9), (6, 7), white)
    a.box("head", (-1, 1), (-10, -9), (7, 8), "#c98685")
    a.box("head", (-1, 1), (-8, -5), (9, 10), stripe)
    a.mirror("head", (2, 3), (-7, -6), (10, 12), stripe)
    a.eyes(1, -9, 8, "#7fae52")
    a.legs(2, 1, -4, 3, 2, 4, banded(1, white, plain))
    a.part("tail", (0, 5, 7))
    a.box("tail", (-1, 1), (5, 9), (6, 7), plain)
    a.box("tail", (-1, 1), (8, 9), (7, 12), lambda i, j, k: stripe() if k % 3 == 0 else plain())
    return a


ANIMALS = (fox, cow, pig, chicken, rabbit, wolf, horse, goat, cat, donkey, mule)


# ---------------------------------------------------------------------------- the clips

def stride(animal, name, length, swing, bob):
    """One stride of `length` frames, the legs swinging by `swing` degrees."""
    frames = [round(length * n / 4) for n in range(5)]
    up = [0, bob * VOX, 0, bob * VOX, 0]
    angles = [0, 1, 0, -1, 0]
    side = 1 if name in ("leg_FL", "leg_BR", "leg_L") else -1
    if animal.hops:
        # both ends of the animal leave the ground together
        up = [0, 2 * bob * VOX, 3 * bob * VOX, bob * VOX, 0]
        side = -1 if name.startswith("leg_F") else 1
    if name.startswith("leg"):
        turn, lift = (swing * side, 0, 0), up if animal.hops else [0] * 5
    elif name.startswith("wing"):
        turn, lift = (0, swing * 0.6 * (1 if name.endswith("L") else -1), 0), up
        angles = [abs(s) for s in angles]
    else:
        turn, lift = {"head": (3, 0, 0), "tail": (0, 0, 14)}.get(name, (0, 0, 0)), up
    return [(f, (0, 0, z), deg(*(t * s for t in turn))) for f, z, s in zip(frames, lift, angles)]


def still():
    return [(0, (0, 0, 0), deg(0, 0, 0)), (72, (0, 0, 0), deg(0, 0, 0))]


def eat(animal, name):
    if name != "head":
        return still()
    down = (0, -VOX, -animal.drop * VOX)
    return [(0, (0, 0, 0), deg(0, 0, 0)), (12, down, deg(48, 0, 0)), (24, down, deg(56, 0, 0)),
            (36, down, deg(45, 0, 0)), (48, down, deg(56, 0, 0)), (60, (0, 0, 0), deg(0, 0, 0)),
            (72, (0, 0, 0), deg(0, 0, 0))]


def idle(_, name):
    turns = {"head": ((0, 0), (12, 14), (24, 14), (36, -11), (48, -11), (60, 0), (72, 0)),
             "tail": ((0, 0), (12, 16), (24, -16), (36, 0), (72, 0))}
    if name not in turns:
        return still()
    return [(f, None, deg(0, 0, a)) for f, a in turns[name]]


# ---------------------------------------------------------------------------- the file

def export(animal):
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.fps = FPS
    colours = sorted({c for cells in animal.parts.values() for c in cells.values()})
    material, index = palette_material(colours, animal.name)
    root = bpy.data.objects.new(animal.name, None)
    scene.collection.objects.link(root)
    clips = {
        "Idle": lambda name: idle(animal, name),
        "Run": lambda name: stride(animal, name, 12, 42, 1.0),
        "Walk": lambda name: stride(animal, name, 24, 24, 0.5),
    }
    if animal.drop:
        clips = {"Eat": lambda name: eat(animal, name), **clips}
    for name, cells in animal.parts.items():
        # No face is hidden by another part, as they move apart
        obj = part_object(name, cells, animal.pivots[name], {}, material, index, scene.collection)
        obj.parent = root
        for clip, keys in clips.items():
            add_clip(obj, clip, keys(name))
    for obj in scene.objects:
        obj.select_set(True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / f"{animal.file}.glb"
    bpy.ops.export_scene.gltf(filepath=str(out), export_format="GLB", use_selection=True,
                              use_active_scene=True, export_animations=True,
                              export_animation_mode="NLA_TRACKS",
                              export_optimize_animation_keep_anim_object=True)
    print(f"wrote {out}: {len(colours)} colours")


if __name__ == "__main__":
    for build in ANIMALS:
        export(build())
    for coat in COATS:
        if coat != "Bay":
            export(horse(coat))
