#!/usr/bin/env python3
"""Draws the skins shipped with the game, in data/graphics/skins/.

A skin is a 64x64 image in the usual layout of block-people skins: each part of the body (head,
body, arms, legs) is a box whose six faces are laid out around a cross at a fixed place of the
image, and the second layer (hat, jacket, sleeves, trousers) at another. The client's model
(client/src/player/model.rs) reads them there. The skins here are drawn from a few parameters:
skin tone, hair and its style, a beard, glasses, shirt, trousers and shoes, and on the second layer
a hat and a jacket, with a little noise so that a face is not flat. Their names are those of
shared::players::skin::BUILT_IN_SKINS.

Run it from the root of the repository: python3 scripts/generate_skins.py
"""

import os
import random

from PIL import Image

OUT = os.path.join("data", "graphics", "skins")

# Where each box of the first layer starts in the image, and its size in pixels (w, h, d)
HEAD = ((0, 0), (8, 8, 8))
HAT = ((32, 0), (8, 8, 8))
BODY = ((16, 16), (8, 12, 4))
RIGHT_ARM = ((40, 16), (4, 12, 4))
LEFT_ARM = ((32, 48), (4, 12, 4))
RIGHT_LEG = ((0, 16), (4, 12, 4))
LEFT_LEG = ((16, 48), (4, 12, 4))


def faces(origin, size):
    """The rectangle (x, y, w, h) of each face of a box."""
    (u, v), (w, h, d) = origin, size
    return {
        "top": (u + d, v, w, d),
        "bottom": (u + d + w, v, w, d),
        "right": (u, v + d, d, h),
        "front": (u + d, v + d, w, h),
        "left": (u + d + w, v + d, d, h),
        "back": (u + 2 * d + w, v + d, w, h),
    }


def shade(color, amount):
    return tuple(max(0, min(255, int(c * amount))) for c in color)


def paint(img, rng, origin, size, colour_at):
    """Fills each face of a box with `colour_at(face, x, y, w, h)`, plus a little noise."""
    for face, (fx, fy, fw, fh) in faces(origin, size).items():
        for y in range(fh):
            for x in range(fw):
                color = colour_at(face, x, y, fw, fh)
                if color is None:
                    continue
                noise = 1.0 + rng.uniform(-0.05, 0.05)
                img.putpixel((fx + x, fy + y), shade(color, noise) + (255,))


def draw(spec):
    rng = random.Random(spec["name"])
    img = Image.new("RGBA", (64, 64), (0, 0, 0, 0))
    skin, hair, eyes = spec["skin"], spec["hair"], spec["eyes"]
    shirt, trousers, shoes = spec["shirt"], spec["trousers"], spec["shoes"]
    style = spec.get("hair_style", "short")  # short, long or bald
    hair_len = spec.get("hair_len", 7 if style == "long" else 3)  # rows of hair down the sides
    hair_back = spec.get("hair_back", 8 if style == "long" else 6)  # rows down the back
    fringe = spec.get("fringe", 2)  # rows of hair on the forehead
    beard = spec.get("beard")
    stripes = spec.get("stripes")  # a second colour of the shirt, every other row

    def head(face, x, y, w, h):
        if face == "top":
            if style == "bald":
                return shade(skin, 1.05)
            return hair
        if face == "bottom":
            return shade(skin, 0.85)
        if style == "bald":
            # a rim of hair above the ears and round the back
            if face == "back":
                return hair if y in (3, 4) else skin
            if face in ("left", "right"):
                ear = x < 4 if face == "right" else x > 3
                return hair if y in (2, 3) and ear else skin
        if face == "back":
            return hair if y < hair_back else skin
        if face in ("left", "right"):
            # The sides: hair down to `hair_len`, and two rows more toward the back
            back_col = x < 3 if face == "right" else x > 4
            return hair if y < hair_len or (back_col and y < hair_len + 2) else skin
        # front: fringe, eyebrows, eyes, nose, mouth
        if style != "bald" and y < fringe:
            return hair
        if style == "long" and x in (0, 7) and y < 7:
            return hair
        if style != "bald" and fringe <= 1 and y == fringe and x in (0, 7):
            return hair
        if y == 3 and x in (1, 2, 5, 6):
            return shade(spec.get("brows", hair), 0.8)
        if spec.get("glasses") and y == 4 and x in (0, 3, 4, 7):
            return spec["glasses"]
        if y == 4 and x in (1, 6):
            return (240, 240, 240)
        if y == 4 and x in (2, 5):
            return eyes
        if y == 5 and x in (3, 4):
            return shade(skin, 0.88)
        if y == 6 and x in (3, 4):
            return spec.get("lips", shade(skin, 0.65))
        if beard and y >= 5 and (y == 7 or x in (0, 1, 6, 7) or (y == 6 and x in (2, 5))):
            return beard
        return skin

    def body(face, x, y, w, h):
        if face == "front" and y == 0 and x in (3, 4):
            return skin
        if face == "front" and y == 1 and x in (3, 4):
            return shade(shirt, 0.75)
        if face in ("front", "back", "left", "right") and y == h - 1:
            return spec.get("belt", shade(trousers, 0.7))
        if face == "front" and spec.get("buttons") and x == 4 and y in (3, 6, 9):
            return shade(shirt, 0.7)
        if spec.get("overalls") and face in ("front", "back"):
            # a bib from the chest down, and its straps over the shoulders
            if y >= 5 and 1 <= x <= w - 2 or x in (1, w - 2):
                return spec["overalls"]
        if stripes and face != "top" and face != "bottom" and y % 2 == 1:
            return stripes
        return shirt

    def arm(face, x, y, w, h):
        sleeve = spec.get("sleeve", 4)
        if face == "top":
            return shirt if sleeve > 0 else skin
        if face == "bottom":
            return spec.get("gloves", skin)
        if spec.get("gloves") and y >= h - 2:
            return spec["gloves"]
        if y < sleeve:
            if stripes and y % 2 == 1:
                return stripes
            return shirt if y < sleeve - 1 else shade(shirt, 0.85)
        return skin

    def leg(face, x, y, w, h):
        if face == "top":
            return trousers
        if face == "bottom" or y >= h - 2:
            return shoes if face != "bottom" else shade(shoes, 0.8)
        if spec.get("overalls") and face in ("front", "back"):
            return spec["overalls"]
        return trousers

    paint(img, rng, *HEAD, head)
    paint(img, rng, *BODY, body)
    paint(img, rng, *RIGHT_ARM, arm)
    paint(img, rng, *LEFT_ARM, arm)
    paint(img, rng, *RIGHT_LEG, leg)
    paint(img, rng, *LEFT_LEG, leg)
    if spec.get("hat"):
        paint(img, rng, *HAT, hat_layer(spec["hat"], spec.get("hat_kind", "band"), spec))
    if spec.get("jacket"):
        jacket = spec["jacket"]
        length = spec.get("jacket_sleeve", 12)

        def jacket_body(face, x, y, w, h):
            if face == "bottom":
                return None
            # open in front, down to the hips
            if face == "front" and 2 < x < w - 3:
                return None
            if face == "front" and y == 0:
                return shade(jacket, 1.15)
            return jacket

        def jacket_arm(face, x, y, w, h):
            if face == "bottom":
                return None
            if face == "top":
                return jacket
            if y >= length:
                return None
            return jacket if y < length - 1 else shade(jacket, 0.8)

        paint(img, rng, *JACKET, jacket_body)
        paint(img, rng, *RIGHT_SLEEVE, jacket_arm)
        paint(img, rng, *LEFT_SLEEVE, jacket_arm)
    return img


def hat_layer(hat, kind, spec):
    """The second layer of the head: what covers the hair, by `kind`."""

    def band(face, x, y, w, h):
        # a band of two rows: a beanie, a headscarf
        if face == "top":
            return hat
        if face == "bottom":
            return None
        return hat if y < 2 else None

    def cap(face, x, y, w, h):
        # a crown of two rows, and a peak of a row over the eyes
        if face == "top":
            return hat
        if face == "bottom":
            return None
        if face == "front" and y == 2:
            return shade(hat, 0.75)
        return hat if y < 2 else None

    def helmet(face, x, y, w, h):
        # down the sides and the back, open on the face
        if face == "top":
            return hat
        if face == "bottom":
            return None
        if face == "front":
            if y < 2 or (y == 2 and x in (0, 7)) or x in (3, 4) and y < 6:
                return hat if y != 1 else shade(hat, 0.8)
            return None
        return hat if y < 6 else None

    def hardhat(face, x, y, w, h):
        # a crown of two rows, a lamp on its front, and a peak all round
        if face == "top":
            return hat
        if face == "bottom":
            return None
        if face == "front" and y == 1 and x in (3, 4):
            return (255, 250, 200)
        if y == 2:
            return shade(hat, 0.8)
        return hat if y < 2 else None

    def hood(face, x, y, w, h):
        # all round the head but the face, which it frames
        if face == "bottom":
            return None
        if face == "front":
            return shade(hat, 0.8) if y == 0 or x in (0, 7) else None
        return hat

    def brim(face, x, y, w, h):
        # a crown with a band, the brim of the hat as low as the layer reaches
        if face == "top":
            return hat
        if face == "bottom":
            return None
        if y == 1:
            return spec.get("hat_band", shade(hat, 0.6))
        return hat if y < 3 else None

    def tall(face, x, y, w, h):
        # a cook's hat: white all down to the brow
        if face == "top":
            return hat
        if face == "bottom":
            return None
        return shade(hat, 0.9) if y == 2 else hat if y < 2 else None

    return {"band": band, "cap": cap, "hardhat": hardhat, "helmet": helmet, "hood": hood, "brim": brim,
            "tall": tall}[kind]


# The second layers of the body and the arms: a jacket and its sleeves
JACKET = ((16, 32), (8, 12, 4))
RIGHT_SLEEVE = ((40, 32), (4, 12, 4))
LEFT_SLEEVE = ((48, 48), (4, 12, 4))

# The skins shipped, each a character of its own. Their names are the ids the client and the
# server know them by (shared::players::skin::BUILT_IN_SKINS), in the same order.
SKINS = [
    dict(name="Wanderer", skin=(222, 172, 136), hair=(84, 52, 30), eyes=(60, 90, 160),
         shirt=(54, 120, 92), trousers=(58, 64, 92), shoes=(70, 50, 40)),
    dict(name="Miner", skin=(141, 85, 54), hair=(30, 24, 22), eyes=(70, 45, 30),
         shirt=(200, 110, 40), trousers=(60, 60, 66), shoes=(45, 35, 30), beard=(36, 28, 24),
         hat=(235, 190, 40), hat_kind="hardhat", gloves=(120, 90, 60), sleeve=12),
    dict(name="Gardener", skin=(240, 200, 170), hair=(200, 150, 60), eyes=(70, 120, 60),
         shirt=(230, 220, 190), trousers=(90, 110, 150), shoes=(90, 60, 40),
         overalls=(70, 110, 160), hat=(220, 190, 110), hat_kind="brim",
         hat_band=(170, 60, 50), hair_style="long", fringe=1),
    dict(name="Sailor", skin=(205, 150, 110), hair=(40, 30, 25), eyes=(60, 70, 80),
         shirt=(240, 240, 235), stripes=(40, 70, 140), trousers=(30, 40, 80),
         shoes=(25, 25, 30), beard=(60, 40, 30), hat=(180, 40, 40), sleeve=12),
    dict(name="Scholar", skin=(245, 215, 190), hair=(200, 200, 205), eyes=(80, 60, 50),
         shirt=(100, 50, 110), trousers=(100, 50, 110), shoes=(50, 30, 30),
         belt=(200, 170, 60), sleeve=12, glasses=(40, 40, 40), beard=(210, 210, 215),
         hair_style="bald"),
    dict(name="Ranger", skin=(180, 130, 95), hair=(110, 60, 30), eyes=(60, 110, 70),
         shirt=(110, 90, 60), trousers=(70, 60, 45), shoes=(60, 45, 30),
         hat=(50, 95, 55), hat_kind="hood", jacket=(50, 95, 55), jacket_sleeve=10),
    dict(name="Smith", skin=(95, 60, 40), hair=(20, 18, 18), eyes=(50, 35, 25),
         shirt=(150, 140, 130), trousers=(70, 55, 45), shoes=(40, 30, 25), sleeve=1,
         beard=(25, 22, 22), hair_style="bald", overalls=(110, 70, 40)),
    dict(name="Baker", skin=(250, 210, 185), hair=(190, 80, 40), eyes=(60, 120, 150),
         shirt=(245, 240, 230), trousers=(150, 120, 90), shoes=(110, 80, 60), buttons=True,
         hat=(250, 250, 250), hat_kind="tall", lips=(200, 110, 100)),
    dict(name="Guard", skin=(230, 185, 150), hair=(150, 110, 60), eyes=(90, 90, 100),
         shirt=(150, 155, 160), trousers=(90, 60, 50), shoes=(50, 40, 35), sleeve=8,
         hat=(170, 175, 180), hat_kind="helmet", jacket=(150, 40, 40), jacket_sleeve=0,
         gloves=(90, 60, 50)),
    dict(name="Courier", skin=(160, 105, 70), hair=(25, 20, 20), eyes=(50, 35, 25),
         shirt=(210, 60, 50), trousers=(40, 45, 55), shoes=(230, 230, 230),
         hat=(40, 120, 200), hat_kind="cap", hair_style="long", fringe=1,
         lips=(130, 70, 60)),
    dict(name="Healer", skin=(120, 80, 55), hair=(15, 12, 12), eyes=(40, 30, 25),
         shirt=(235, 240, 240), trousers=(60, 140, 140), shoes=(70, 70, 75),
         hair_style="long", jacket=(60, 140, 140), jacket_sleeve=4),
    dict(name="Fisher", skin=(235, 195, 160), hair=(130, 120, 110), eyes=(70, 100, 130),
         shirt=(240, 200, 40), trousers=(240, 200, 40), shoes=(40, 60, 40), sleeve=12,
         beard=(150, 140, 130), hat=(240, 200, 40), hat_kind="hood", gloves=(40, 60, 40)),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    for spec in SKINS:
        path = os.path.join(OUT, f"{spec['name']}.png")
        draw(spec).save(path)
        print(path)


if __name__ == "__main__":
    main()
