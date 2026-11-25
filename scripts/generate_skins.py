#!/usr/bin/env python3
"""Draws the skins shipped with the game, in data/graphics/skins/.

A skin is a 64x64 image in the usual layout of block-people skins: each part of the body (head,
body, arms, legs) is a box whose six faces are laid out around a cross at a fixed place of the
image, and the second layer (hat, jacket, sleeves, trousers) at another. The client's model
(client/src/player/model.rs) reads them there. The skins here are drawn from a few parameters:
skin tone, hair, shirt, trousers and shoes, with a little noise so that a face is not flat.

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
    hair_len = spec.get("hair_len", 3)  # rows of hair down the sides
    hair_back = spec.get("hair_back", 6)  # rows of hair down the back
    fringe = spec.get("fringe", 2)  # rows of hair on the forehead
    beard = spec.get("beard")

    def head(face, x, y, w, h):
        if face == "top":
            return hair
        if face == "bottom":
            return shade(skin, 0.85)
        if face == "back":
            return hair if y < hair_back else skin
        if face in ("left", "right"):
            # The sides: hair down to `hair_len`, and two rows more toward the back
            back_col = x < 3 if face == "right" else x > 4
            return hair if y < hair_len or (back_col and y < hair_len + 2) else skin
        # front: fringe, eyebrows, eyes, nose, mouth
        if y < fringe:
            return hair
        if fringe <= 1 and y == fringe and x in (0, 7):
            return hair
        if y == 3 and x in (1, 2, 5, 6):
            return shade(hair, 0.8)
        if y == 4 and x in (1, 6):
            return (240, 240, 240)
        if y == 4 and x in (2, 5):
            return eyes
        if y == 5 and x in (3, 4):
            return shade(skin, 0.88)
        if y == 6 and x in (3, 4):
            return shade(skin, 0.65)
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
        return shirt

    def arm(face, x, y, w, h):
        sleeve = spec.get("sleeve", 4)
        if face == "top":
            return shirt if sleeve > 0 else skin
        if face == "bottom":
            return skin
        if y < sleeve:
            return shirt if y < sleeve - 1 else shade(shirt, 0.85)
        return skin

    def leg(face, x, y, w, h):
        if face == "top":
            return trousers
        if face == "bottom" or y >= h - 2:
            return shoes if face != "bottom" else shade(shoes, 0.8)
        return trousers

    paint(img, rng, *HEAD, head)
    paint(img, rng, *BODY, body)
    paint(img, rng, *RIGHT_ARM, arm)
    paint(img, rng, *LEFT_ARM, arm)
    paint(img, rng, *RIGHT_LEG, leg)
    paint(img, rng, *LEFT_LEG, leg)
    if spec.get("hat"):
        hat = spec["hat"]

        def hat_layer(face, x, y, w, h):
            if face == "top":
                return hat
            if face == "bottom":
                return None
            return hat if y < 2 else None

        paint(img, rng, *HAT, hat_layer)
    return img


SKINS = [
    dict(name="Wanderer", skin=(222, 172, 136), hair=(84, 52, 30), eyes=(60, 90, 160),
         shirt=(54, 120, 92), trousers=(58, 64, 92), shoes=(70, 50, 40)),
]


def main():
    os.makedirs(OUT, exist_ok=True)
    for spec in SKINS:
        path = os.path.join(OUT, f"{spec['name']}.png")
        draw(spec).save(path)
        print(path)


if __name__ == "__main__":
    main()
