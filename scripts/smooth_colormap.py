#!/usr/bin/env python3
"""Smooths a colour map of the Pixel Perfection pack into a gradient.

The pack paints its colour maps (colormap/grass.png, colormap/foliage.png) as
flat regions, with a line at the climate of each vanilla biome and labels in
the half no climate reaches. The client reads them at any climate, where the
regions show as patches with hard edges. The marker lines are taken out with a
median filter, then the half the climates reach (y >= x) is blurred on its
own, and the other half takes the colour of the diagonal at the same height.

Requirements: numpy, pillow.
Usage: python3 scripts/smooth_colormap.py <pack colour map> <output>
"""

import sys

import numpy as np
from PIL import Image, ImageFilter

# The spread of the blur, in texels of the 256 by 256 map
SIGMA = 18.0
MEDIAN = 7


def blur(channel, sigma):
    radius = int(3 * sigma)
    kernel = np.exp(-0.5 * (np.arange(-radius, radius + 1) / sigma) ** 2)
    padded = np.pad(channel, radius, mode="constant")
    rows = np.apply_along_axis(lambda row: np.convolve(row, kernel, "valid"), 1, padded)
    return np.apply_along_axis(lambda col: np.convolve(col, kernel, "valid"), 0, rows)


def smooth(source):
    size = source.width
    colors = np.asarray(source.filter(ImageFilter.MedianFilter(MEDIAN)), dtype=np.float64)
    y, x = np.mgrid[0:size, 0:size]
    # The median reads the labels next to the diagonal, which keeps its own colours
    colors[y == x] = np.asarray(source, dtype=np.float64)[y == x]

    # Blurred within the used half only, each texel by the weight of the used texels around
    used = (y >= x).astype(np.float64)
    weight = blur(used, SIGMA)
    with np.errstate(invalid="ignore", divide="ignore"):
        result = np.stack(
            [blur(colors[..., c] * used, SIGMA) for c in range(3)], axis=-1
        ) / weight[..., None]
    for row in range(size):
        result[row, row + 1 :] = result[row, row]
    return Image.fromarray(np.clip(result.round(), 0, 255).astype(np.uint8), "RGB")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    smooth(Image.open(sys.argv[1]).convert("RGB")).save(sys.argv[2])
