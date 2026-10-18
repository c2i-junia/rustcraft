#!/usr/bin/env python3
"""Procedurally generates the game's sound effects into data/sounds/.

Every sound is synthesised from noise, filters and damped sines with a fixed
seed, so the output is reproducible and entirely original (MIT, like the code).

Requirements: numpy, scipy, and ffmpeg built with libvorbis.
Usage: python3 scripts/generate_sounds.py [output_dir]
"""

import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, sosfilt

SR = 44100
ROOT = Path(__file__).resolve().parent.parent
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "data" / "sounds"


# --- primitives -------------------------------------------------------------


def t_axis(duration):
    return np.arange(int(duration * SR)) / SR


def noise(rng, duration):
    return rng.uniform(-1.0, 1.0, int(duration * SR))


def band(x, low=None, high=None, order=2):
    if low and high:
        sos = butter(order, [low, high], "bandpass", fs=SR, output="sos")
    elif low:
        sos = butter(order, low, "highpass", fs=SR, output="sos")
    else:
        sos = butter(order, high, "lowpass", fs=SR, output="sos")
    return sosfilt(sos, x)


def env(duration, attack, tau):
    """Linear attack then exponential decay with time constant `tau`."""
    t = t_axis(duration)
    a = np.clip(t / max(attack, 1e-4), 0.0, 1.0)
    return a * np.exp(-np.maximum(t - attack, 0.0) / tau)


def modal(rng, duration, modes, detune=0.03):
    """Sum of damped sines: modes = [(freq, tau, amp), ...]."""
    t = t_axis(duration)
    out = np.zeros_like(t)
    for freq, tau, amp in modes:
        f = freq * (1.0 + rng.uniform(-detune, detune))
        out += amp * np.sin(2 * np.pi * f * t + rng.uniform(0, 2 * np.pi)) * np.exp(-t / tau)
    return out


def crackle(rng, duration, density, low, high, tau):
    """Sparse random clicks, band-limited: dry leaves, gravel, debris."""
    n = int(duration * SR)
    x = np.zeros(n)
    count = rng.poisson(density * duration)
    idx = rng.integers(0, n, count)
    x[idx] = rng.uniform(-1.0, 1.0, count) * rng.uniform(0.3, 1.0, count)
    return band(x, low, high) * env(duration, 0.002, tau)


def chirp(duration, f0, f1, tau):
    t = t_axis(duration)
    f = f0 + (f1 - f0) * (t / duration)
    phase = 2 * np.pi * np.cumsum(f) / SR
    return np.sin(phase) * env(duration, 0.003, tau)


def place_at(total, grain, offset):
    """Mixes `grain` into a silent buffer of `total` seconds at `offset` s."""
    out = np.zeros(int(total * SR))
    start = int(offset * SR)
    end = min(len(out), start + len(grain))
    out[start:end] += grain[: end - start]
    return out


def mix(*parts):
    n = max(len(p) for p in parts)
    out = np.zeros(n)
    for p in parts:
        out[: len(p)] += p
    return out


def finish(x, peak_db):
    """Short fades against clicks, then peak normalisation to `peak_db` dBFS."""
    fade = min(int(0.003 * SR), len(x) // 4)
    x = x.copy()
    x[:fade] *= np.linspace(0, 1, fade)
    x[-fade * 3 :] *= np.linspace(1, 0, fade * 3)
    peak = np.max(np.abs(x)) or 1.0
    return x / peak * 10 ** (peak_db / 20)


# --- material grains --------------------------------------------------------
# One short impact per material; steps, hits, breaks and placements are built
# by scaling, layering and repeating these.


def grain_grass(rng, d=0.18):
    body = band(noise(rng, d), 300, 3500) * env(d, 0.006, 0.045)
    return body * 0.6 + crackle(rng, d, 900, 1800, 7000, 0.06) * 2.5


def grain_dirt(rng, d=0.16):
    body = band(noise(rng, d), 80, 1100) * env(d, 0.004, 0.035)
    thud = modal(rng, d, [(85, 0.03, 0.8), (140, 0.02, 0.4)])
    return body + thud * 0.6 + crackle(rng, d, 250, 800, 3000, 0.03) * 0.8


def grain_stone(rng, d=0.14):
    click = band(noise(rng, d), 2000, 6000) * env(d, 0.0005, 0.006)
    body = band(noise(rng, d), 700, 3200) * env(d, 0.001, 0.022)
    ring = modal(rng, d, [(620, 0.03, 0.3), (1130, 0.02, 0.25), (1870, 0.015, 0.2)], 0.08)
    return click * 1.2 + body + ring * 0.5


def grain_wood(rng, d=0.2):
    knock = modal(rng, d, [(190, 0.07, 1.0), (420, 0.045, 0.6), (760, 0.03, 0.35), (1250, 0.02, 0.2)], 0.06)
    tap = band(noise(rng, d), 400, 2500) * env(d, 0.0005, 0.01)
    return knock * 0.8 + tap


def grain_sand(rng, d=0.24):
    hiss = band(noise(rng, d), 1500, 9000) * env(d, 0.02, 0.07)
    grit = crackle(rng, d, 2500, 3000, 10000, 0.08)
    return hiss * 0.7 + grit * 1.5


def grain_leaves(rng, d=0.22):
    rustle = band(noise(rng, d), 2500, 9000) * env(d, 0.01, 0.06)
    return rustle * 0.4 + crackle(rng, d, 1800, 2000, 9000, 0.07) * 3.0


def grain_snow(rng, d=0.22):
    squeak = band(noise(rng, d), 600, 4000) * env(d, 0.015, 0.05)
    return squeak * 0.5 + crackle(rng, d, 1400, 900, 5000, 0.07) * 2.0


def grain_glass(rng, d=0.35):
    ring = modal(rng, d, [(2300, 0.12, 0.6), (3700, 0.09, 0.5), (5200, 0.06, 0.4), (7100, 0.04, 0.3)], 0.1)
    tick = band(noise(rng, d), 4000, 12000) * env(d, 0.0005, 0.004)
    return ring + tick


GRAINS = {
    "grass": grain_grass,
    "dirt": grain_dirt,
    "stone": grain_stone,
    "wood": grain_wood,
    "sand": grain_sand,
    "leaves": grain_leaves,
    "snow": grain_snow,
    "glass": grain_glass,
}

# Materials a player can stand on; glass and leaves get no footsteps.
WALKABLE = ["grass", "dirt", "stone", "wood", "sand", "snow"]


# --- sound builders ---------------------------------------------------------


def build_step(rng, material):
    return finish(GRAINS[material](rng), -12)


def build_hit(rng, material):
    g = GRAINS[material](rng)
    return finish(g[: int(0.1 * SR)], -10)


def build_break(rng, material):
    """A main impact followed by a few smaller debris impacts."""
    total = 0.55
    parts = [place_at(total, GRAINS[material](rng), 0.0)]
    for _ in range(rng.integers(3, 6)):
        offset = rng.uniform(0.03, 0.22)
        parts.append(place_at(total, GRAINS[material](rng) * rng.uniform(0.25, 0.6), offset))
    if material == "glass":
        parts.append(crackle(rng, total, 3000, 3000, 12000, 0.12) * 2.0)
    return finish(mix(*parts), -4)


def build_place(rng, material):
    d = 0.2
    thud = modal(rng, d, [(110, 0.04, 1.0), (180, 0.025, 0.5)]) + band(noise(rng, d), 60, 600) * env(d, 0.002, 0.02)
    return finish(mix(thud * 0.7, GRAINS[material](rng) * 0.8), -6)


def build_land(rng):
    d = 0.25
    thud = modal(rng, d, [(70, 0.06, 1.0), (115, 0.04, 0.5)]) + band(noise(rng, d), 50, 800) * env(d, 0.003, 0.04)
    return finish(thud, -6)


def build_hurt(rng):
    d = 0.3
    punch = chirp(d, 220, 70, 0.08)
    body = band(noise(rng, d), 100, 1500) * env(d, 0.002, 0.03)
    return finish(punch + body * 0.6, -4)


def build_pickup(rng):
    d = 0.12
    pop = chirp(d, 520, 1150, 0.04) + chirp(d, 1040, 2300, 0.03) * 0.25
    return finish(pop, -8)


def build_ui_click(rng):
    d = 0.05
    tick = band(noise(rng, d), 1500, 5000) * env(d, 0.0005, 0.004)
    tone = modal(rng, d, [(1400, 0.01, 0.6)], 0.0)
    return finish(tick + tone, -10)


def build_splash(rng):
    total = 0.7
    wash = band(noise(rng, total), 300, 6000) * env(total, 0.01, 0.12)
    parts = [wash * 0.8]
    for _ in range(rng.integers(10, 16)):
        f0 = rng.uniform(400, 1200)
        bubble = chirp(0.06, f0, f0 * rng.uniform(1.5, 2.5), 0.02) * rng.uniform(0.2, 0.5)
        parts.append(place_at(total, bubble, rng.uniform(0.02, 0.45)))
    return finish(mix(*parts), -4)


def build_wind(rng):
    """Seamless 20 s loop: slowly swept low-passed noise, crossfaded at the seam."""
    d, xf = 20.0, 2.0
    x = noise(rng, d + xf)
    t = t_axis(d + xf)
    lfo = 0.5 + 0.5 * np.sin(2 * np.pi * t / (d / 3))  # 3 full sweeps per loop
    low = band(x, high=250, order=4)
    high = band(x, 250, 900, order=2)
    y = low + high * (0.3 + 0.7 * lfo)
    n, k = int(d * SR), int(xf * SR)
    ramp = np.linspace(0, 1, k)
    y[:k] = y[:k] * ramp + y[n : n + k] * (1 - ramp)
    y = y[:n]
    return y / np.max(np.abs(y)) * 10 ** (-12 / 20)


# --- output -----------------------------------------------------------------


def write_ogg(path, x):
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".wav") as tmp:
        wavfile.write(tmp.name, SR, (np.clip(x, -1, 1) * 32767).astype(np.int16))
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", tmp.name, "-c:a", "libvorbis", "-q:a", "4", str(path)],
            check=True,
        )
    print(path.relative_to(OUT))


def main():
    rng = np.random.default_rng(91)  # issue #91
    for material in WALKABLE:
        for i in range(1, 5):
            write_ogg(OUT / "step" / f"{material}{i}.ogg", build_step(rng, material))
    for material in GRAINS:
        for i in range(1, 4):
            write_ogg(OUT / "dig" / f"{material}{i}.ogg", build_hit(rng, material))
            write_ogg(OUT / "break" / f"{material}{i}.ogg", build_break(rng, material))
        write_ogg(OUT / "place" / f"{material}.ogg", build_place(rng, material))
    write_ogg(OUT / "player" / "land.ogg", build_land(rng))
    write_ogg(OUT / "player" / "hurt.ogg", build_hurt(rng))
    write_ogg(OUT / "player" / "pickup.ogg", build_pickup(rng))
    write_ogg(OUT / "ui" / "click.ogg", build_ui_click(rng))
    write_ogg(OUT / "liquid" / "splash.ogg", build_splash(rng))
    write_ogg(OUT / "ambient" / "wind.ogg", build_wind(rng))


if __name__ == "__main__":
    main()
