#!/usr/bin/env python3
"""Procedurally generates the game's sound effects into data/sounds/.

Every sound is synthesised with a fixed seed, so the output is reproducible and
entirely original (MIT, like the code). Crumbly materials (grass, dirt, sand,
snow, leaves) are clouds of tiny filtered grains, after Perry Cook's PhISEM;
hard ones (stone, wood, glass) are banks of damped resonant modes. Footsteps
are a heel then a toe, every sound gets a faint outdoor reflection, and each
kind of sound is normalised to one loudness (ITU-R BS.1770 K-weighting). The
rain is a seamless loop of thousands of drops; the thunder is summed from the
shocks of a whole lightning channel, as heard from near or far.

Requirements: numpy, scipy, and ffmpeg built with libvorbis.
Usage: python3 scripts/generate_sounds.py [output_dir] [--only PREFIX]

`--only weather/` writes only the files whose name starts with `weather/`: the encoder
does not give the same bytes twice, so rewriting every file would change them all.
"""

import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, fftconvolve, lfilter, sosfilt

SR = 44100
ROOT = Path(__file__).resolve().parent.parent
ONLY = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else ""
ARGS = [arg for arg in sys.argv[1:] if arg not in ("--only", ONLY)]
OUT = Path(ARGS[0]) if ARGS else ROOT / "data" / "sounds"
SEED = 91  # issue #91


# --- primitives -------------------------------------------------------------


def n_of(duration):
    return int(duration * SR)


def t_axis(duration):
    return np.arange(n_of(duration)) / SR


def white(rng, duration):
    return rng.standard_normal(n_of(duration))


def colored(rng, duration, alpha):
    """Noise whose power falls as 1/f^alpha: 1 is pink, 2 is brown."""
    n = n_of(duration)
    spectrum = np.fft.rfft(rng.standard_normal(n))
    f = np.fft.rfftfreq(n, 1 / SR)
    f[0] = f[1]
    x = np.fft.irfft(spectrum / f ** (alpha / 2), n)
    return x / (np.std(x) or 1.0)


def band(x, low=None, high=None, order=2):
    if low and high:
        sos = butter(order, [low, high], "bandpass", fs=SR, output="sos")
    elif low:
        sos = butter(order, low, "highpass", fs=SR, output="sos")
    else:
        sos = butter(order, high, "lowpass", fs=SR, output="sos")
    return sosfilt(sos, x)


def env(duration, attack, tau):
    """Smooth (raised-cosine) attack then exponential decay with time constant `tau`."""
    t = t_axis(duration)
    a = np.where(t < attack, 0.5 - 0.5 * np.cos(np.pi * t / max(attack, 1e-5)), 1.0)
    return a * np.exp(-np.maximum(t - attack, 0.0) / tau)


def modes(rng, duration, spec, detune=0.02):
    """Sum of damped sines, spec = [(freq, tau, amp), ...], each with a soft 0.3 ms onset."""
    t = t_axis(duration)
    onset = np.clip(t / 0.0003, 0.0, 1.0)
    out = np.zeros_like(t)
    for freq, tau, amp in spec:
        if freq >= SR / 2.2:
            continue
        f = freq * (1.0 + rng.uniform(-detune, detune))
        out += amp * np.sin(2 * np.pi * f * t + rng.uniform(0, 2 * np.pi)) * np.exp(-t / tau)
    return out * onset


def thump(rng, duration, f0, f1, tau, amp=1.0):
    """A low sine whose pitch falls from `f0` to `f1`: the weight of a body meeting the ground."""
    t = t_axis(duration)
    f = f1 + (f0 - f1) * np.exp(-t / 0.012)
    return amp * np.sin(2 * np.pi * np.cumsum(f) / SR) * env(duration, 0.001, tau)


def grains(rng, duration, rate, low, high, size=(0.0003, 0.0015), spread=0.6, bands=6):
    """Stochastic particles (PhISEM): short noise bursts at Poisson times.

    `rate` is the density in grains per second, an array over time; `size` the range
    of a grain's decay time; `spread` the log-normal spread of grain loudness, which
    makes the cloud crackly rather than hissy (capped, so that no grain
    stands out alone). Each grain is coloured by one of
    `bands` sub-bands of [low, high], so neighbours do not all sound the same.
    """
    n = n_of(duration)
    rate = np.broadcast_to(rate, (n,))
    count = rng.poisson(rate.sum() / SR)
    if count == 0:
        return np.zeros(n)
    starts = rng.choice(n, size=count, p=rate / rate.sum())
    edges = np.geomspace(low, high, bands + 1)
    lanes = np.zeros((bands, n))
    for start in starts:
        tau = np.exp(rng.uniform(np.log(size[0]), np.log(size[1])))
        length = min(n_of(tau * 5) + 1, n - start)
        burst = rng.standard_normal(length) * np.exp(-np.arange(length) / (tau * SR))
        lanes[rng.integers(bands), start : start + length] += burst * min(rng.lognormal(0.0, spread), 3.0)
    return sum(band(lane, edges[i], edges[i + 1]) for i, lane in enumerate(lanes))


def density(duration, attack, tau, peak):
    """A grain rate rising over `attack` then decaying with `tau`, peaking at `peak`."""
    return env(duration, attack, tau) * peak


def place_at(total, part, offset):
    """Mixes `part` into a silent buffer of `total` seconds at `offset` s."""
    out = np.zeros(n_of(total))
    start = n_of(offset)
    end = min(len(out), start + len(part))
    out[start:end] += part[: end - start]
    return out


def mix(*parts):
    n = max(len(p) for p in parts)
    out = np.zeros(n)
    for p in parts:
        out[: len(p)] += p
    return out


def rms(x):
    return np.sqrt(np.mean(x**2)) or 1.0


def outdoors(rng, x, wet=0.12):
    """A faint, dark, quickly dying reflection: the ground and the air around the player."""
    d = 0.3
    ir = band(white(rng, d), high=4500) * env(d, 0.004, 0.045)
    ir = place_at(d, ir, 0.006)
    ir /= np.sqrt(np.sum(ir**2))
    tail = fftconvolve(x, ir)[: len(x) + n_of(d)]
    return mix(x, tail * wet)


# --- loudness ---------------------------------------------------------------


def k_weighted(x):
    """The two filters of ITU-R BS.1770: a head-related high shelf, then a high-pass."""
    gain, q, fc = 3.99984385397, 0.7071752369554193, 1681.9744509555319
    a = 10 ** (gain / 40)
    w = 2 * np.pi * fc / SR
    alpha, c = np.sin(w) / (2 * q), np.cos(w)
    b = [a * ((a + 1) + (a - 1) * c + 2 * np.sqrt(a) * alpha), -2 * a * ((a - 1) + (a + 1) * c), a * ((a + 1) + (a - 1) * c - 2 * np.sqrt(a) * alpha)]
    den = [(a + 1) - (a - 1) * c + 2 * np.sqrt(a) * alpha, 2 * ((a - 1) - (a + 1) * c), (a + 1) - (a - 1) * c - 2 * np.sqrt(a) * alpha]
    x = lfilter(b, den, x)
    q, fc = 0.5003270373253953, 38.13547087613982
    w = 2 * np.pi * fc / SR
    alpha, c = np.sin(w) / (2 * q), np.cos(w)
    return lfilter([(1 + c) / 2, -(1 + c), (1 + c) / 2], [1 + alpha, -2 * c, 1 - alpha], x)


def loudness(x):
    """The loudest 100 ms of `x`, in LUFS: short sounds are judged by their peak moment."""
    y = k_weighted(x)
    w = min(n_of(0.1), len(y))
    energy = np.convolve(y**2, np.ones(w) / w, "valid")
    return -0.691 + 10 * np.log10(energy.max() + 1e-12)


def finish(x, lufs):
    """Removes DC, trims the silent tail, fades the ends, and sets the loudness to `lufs`
    (never above -1 dBFS peak)."""
    x = band(x, low=30)
    audible = np.nonzero(np.abs(x) > np.max(np.abs(x)) * 10 ** (-60 / 20))[0]
    x = x[: audible[-1] + 1] if len(audible) else x
    head, tail = min(n_of(0.001), len(x) // 4), min(n_of(0.02), len(x) // 3)
    x[:head] *= np.linspace(0, 1, head)
    x[-tail:] *= np.linspace(1, 0, tail) ** 2
    gain = 10 ** ((lufs - loudness(x)) / 20)
    gain = min(gain, 10 ** (-1 / 20) / np.max(np.abs(x)))
    return x * gain


# --- material contacts ------------------------------------------------------
# One contact per material, at a `strength` from a soft touch (0.3) to a blow
# (1.2): harder contacts are louder, brighter and denser. Footsteps, hits,
# breaks and placements are built from these.


def contact_grass(rng, s):
    d = 0.32
    soil = thump(rng, d, 140, 75, 0.022, 0.5) + band(colored(rng, d, 2), high=450) * env(d, 0.002, 0.02) * 0.6
    crunch = grains(rng, d, density(d, 0.004, 0.035 + 0.02 * s, 5000 * s), 1200, 7500, (0.0002, 0.0012), 0.7)
    swish = band(colored(rng, d, 1), 700, 4000) * env(d, 0.006, 0.03)
    return band(soil * s + crunch * 0.09 + swish * 0.06 * s, high=9000)


def contact_dirt(rng, s):
    d = 0.32
    soil = thump(rng, d, 160, 62, 0.03, 1.0) + band(colored(rng, d, 2), high=700) * env(d, 0.002, 0.028) * 0.9
    crumble = grains(rng, d, density(d, 0.003, 0.03 + 0.03 * s, 1800 * s), 250, 3000, (0.0006, 0.003), 0.7)
    return soil * s + crumble * 0.08


def contact_stone(rng, s):
    d = 0.3
    click = band(white(rng, d), 1800, 11000) * env(d, 0.0002, 0.0012 + 0.001 * s)
    body = band(colored(rng, d, 1), 500, 3000) * env(d, 0.0005, 0.008 + 0.006 * s)
    f0 = rng.uniform(850, 1300)
    ring = modes(rng, d, [(f0 * r, 0.03 / r**0.7, 1 / r) for r in (1.0, 1.59, 2.37, 3.21, 4.43)], 0.05)
    grit = grains(rng, d, density(d, 0.002, 0.02, 3000 * s), 2500, 9000, (0.0002, 0.0007), 0.8)
    low = thump(rng, d, 180, 100, 0.012, 1.0)
    return click * 0.5 * s + body * 0.5 + ring * 0.05 * s + grit * 0.06 + low * 0.25


def contact_wood(rng, s):
    d = 0.35
    f0 = rng.uniform(150, 200)
    # A free plank's modes; high modes die faster and only ring under harder blows
    plank = modes(rng, d, [(f0 * r, 0.08 / r**0.9, s ** (i * 0.6) / (1 + i)) for i, r in enumerate((1.0, 2.76, 5.40, 8.93))], 0.03)
    box = modes(rng, d, [(rng.uniform(90, 115), 0.035, 0.5)])
    tap = band(white(rng, d), 900, 5000) * env(d, 0.0002, 0.0025)
    body = band(colored(rng, d, 1), 200, 1800) * env(d, 0.0008, 0.012)
    return plank * 0.55 * s + box * 0.4 * s + tap * 0.25 * s + body * 0.2


def contact_sand(rng, s):
    d = 0.4
    pour = grains(rng, d, density(d, 0.01, 0.035 + 0.03 * s, 9000 * s), 1400, 7000, (0.0002, 0.0008), 0.45)
    slide = band(colored(rng, d, 1), 700, 3500) * env(d, 0.015, 0.05)
    soft = band(colored(rng, d, 2), high=300) * env(d, 0.004, 0.025)
    return band(pour * 0.07 + slide * 0.06 * s + soft * 0.3 * s, high=8000)


def contact_snow(rng, s):
    d = 0.4
    # Packed snow gives way in a few quick collapses rather than one smooth hiss
    rate = np.zeros(n_of(d))
    for _ in range(rng.integers(4, 8)):
        rate += place_at(d, env(0.03, 0.003, 0.008) * rng.uniform(0.5, 1.0), rng.uniform(0.0, 0.07 + 0.05 * s))
    crunch = grains(rng, d, rate * 9000 * s, 500, 4500, (0.0004, 0.002), 0.8)
    muffle = band(colored(rng, d, 2), high=280) * env(d, 0.004, 0.03)
    return band(crunch * 0.06 + muffle * 0.35 * s, high=6500)


def contact_leaves(rng, s):
    d = 0.4
    rustle = grains(rng, d, density(d, 0.01, 0.05 + 0.05 * s, 6000 * s), 1500, 8500, (0.0002, 0.001), 1.0)
    air = band(colored(rng, d, 1), 1200, 6000) * env(d, 0.01, 0.05)
    return band(rustle * 0.07 + air * 0.05 * s, high=10000)


def contact_glass(rng, s):
    d = 0.5
    f0 = rng.uniform(1900, 2600)
    ring = modes(rng, d, [(f0 * r, 0.14 / r**0.6, 1 / r**0.5) for r in (1.0, 1.53, 2.17, 2.84, 3.61)], 0.04)
    tick = band(white(rng, d), 3000, 14000) * env(d, 0.0002, 0.0015)
    knock = thump(rng, d, 300, 180, 0.01, 1.0)
    return ring * 0.35 * s + tick * 0.4 * s + knock * 0.3


CONTACTS = {
    "grass": contact_grass,
    "dirt": contact_dirt,
    "stone": contact_stone,
    "wood": contact_wood,
    "sand": contact_sand,
    "leaves": contact_leaves,
    "snow": contact_snow,
    "glass": contact_glass,
}

# Materials a player can stand on; glass and leaves get no footsteps.
WALKABLE = ["grass", "dirt", "stone", "wood", "sand", "snow"]


# --- break debris -----------------------------------------------------------


def debris_times(rng, count, spread):
    """When the bits of a broken block land: most at once, a few bouncing on later, none
    so late that it sounds like another event."""
    return np.sort(0.012 + np.minimum(rng.gamma(1.4, spread, count), spread * 4))


def rubble(rng, total, count, f_low, f_high, spread=0.06):
    """Small hard chips clattering down: one short inharmonic ping each."""
    parts = []
    for i, offset in enumerate(debris_times(rng, count, spread)):
        if offset >= total - 0.05:
            continue
        f0 = rng.uniform(f_low, f_high)
        chip = modes(rng, 0.1, [(f0, 0.012, 1.0), (f0 * 1.71, 0.008, 0.6), (f0 * 2.63, 0.005, 0.4)], 0.05)
        chip += band(white(rng, 0.1), 2000, 12000) * env(0.1, 0.0002, 0.001) * 0.6
        parts.append(place_at(total, chip * rng.uniform(0.3, 1.0) * 0.85**i, offset))
    return mix(np.zeros(n_of(total)), *parts)


def break_crumbly(rng, material, total, tail):
    """A blow, then the material itself falling apart for `tail` seconds."""
    first = CONTACTS[material](rng, 1.2)
    parts = [place_at(total, first, 0.0)]
    for offset in debris_times(rng, rng.integers(4, 7), 0.05):
        parts.append(place_at(total, CONTACTS[material](rng, rng.uniform(0.3, 0.6)) * 0.6, offset))
    falling = env(total, 0.02, tail)
    return mix(*parts), falling


def build_break(rng, material):
    if material == "stone":
        total = 0.7
        crack = band(white(rng, total), 700, 7000) * env(total, 0.0003, 0.012)
        boom = thump(rng, total, 200, 85, 0.05)
        crumble = grains(rng, total, density(total, 0.01, 0.12, 2500), 400, 4000, (0.0005, 0.002), 0.8)
        x = mix(CONTACTS["stone"](rng, 1.2), crack * 0.5, boom * 0.4, crumble * 0.05, rubble(rng, total, 14, 1500, 4200))
    elif material == "wood":
        total = 0.7
        snap = band(white(rng, total), 1000, 8000) * env(total, 0.0002, 0.006)
        splinter = grains(rng, total, density(total, 0.002, 0.03, 9000), 900, 7000, (0.0002, 0.001), 1.1)
        bits = [place_at(total, contact_wood(rng, 0.5) * 0.4, t) for t in debris_times(rng, 5, 0.07)]
        x = mix(contact_wood(rng, 1.3), snap * 0.4, splinter * 0.08, *bits)
    elif material == "glass":
        total = 1.0
        smash = band(white(rng, total), 1200, 14000) * env(total, 0.0002, 0.007)
        shards = []
        for i, offset in enumerate(debris_times(rng, 40, 0.07)):
            if offset >= total - 0.1:
                continue
            f0 = rng.uniform(2500, 7500)
            shard = modes(rng, 0.25, [(f0, rng.uniform(0.03, 0.09), 1.0), (f0 * 1.47, 0.03, 0.5), (f0 * 2.09, 0.02, 0.3)], 0.05)
            shards.append(place_at(total, shard * rng.uniform(0.2, 1.0) * 0.95**i * 0.6, offset))
        tinkle = grains(rng, total, density(total, 0.002, 0.12, 6000), 3000, 12000, (0.0002, 0.0008), 1.0)
        x = mix(contact_glass(rng, 1.2) * 0.5, smash * 0.6, tinkle * 0.08, *shards)
    elif material in ("grass", "dirt"):
        total = 0.6
        x, falling = break_crumbly(rng, "dirt", total, 0.12)
        crumble = grains(rng, total, falling * 2500, 250, 3500, (0.0006, 0.003), 0.8)
        x = mix(x, crumble * 0.05)
        if material == "grass":
            rip = grains(rng, total, density(total, 0.005, 0.06, 9000), 1200, 7500, (0.0002, 0.0012), 0.8)
            x = mix(x, rip * 0.08)
    else:  # sand, snow, leaves
        total = 0.7
        x, falling = break_crumbly(rng, material, total, 0.15)
        low, high = {"sand": (1200, 7000), "snow": (500, 4500), "leaves": (1500, 8500)}[material]
        x = mix(x, grains(rng, total, falling * 5000, low, high, (0.0002, 0.001), 0.7) * 0.05)
        if material == "leaves":
            for offset in rng.uniform(0.0, 0.15, 3):  # twigs snapping
                twig = band(white(rng, 0.05), 1500, 6000) * env(0.05, 0.0002, 0.003)
                x = mix(x, place_at(total, twig * 0.35, offset))
    return finish(outdoors(rng, x), -17)


# --- sound builders ---------------------------------------------------------


def build_step(rng, material):
    """A heel, then the toe rolling down a moment later, softer."""
    total = 0.45
    contact = CONTACTS[material]
    heel = contact(rng, rng.uniform(0.75, 0.9))
    toe = contact(rng, rng.uniform(0.35, 0.5))
    x = mix(place_at(total, heel, 0.0), place_at(total, toe * 0.5, rng.uniform(0.05, 0.085)))
    return finish(outdoors(rng, x, 0.08), -26)


def build_hit(rng, material):
    """One blow of the hand or tool mining a block, short enough to repeat four times a second."""
    x = CONTACTS[material](rng, 1.0)[: n_of(0.22)]
    x[-n_of(0.06) :] *= np.linspace(1, 0, n_of(0.06)) ** 2
    return finish(outdoors(rng, x, 0.1), -23)


def build_place(rng, material):
    """The block set down: a solid knock, then the material's own contact."""
    d = 0.35
    knock = thump(rng, d, 220, 105, 0.03) + band(colored(rng, d, 2), high=600) * env(d, 0.001, 0.015) * 0.5
    x = mix(knock * 0.5, CONTACTS[material](rng, 0.7))
    return finish(outdoors(rng, x), -20)


def build_land(rng):
    d = 0.4
    body = thump(rng, d, 120, 48, 0.06) + band(colored(rng, d, 2), high=500) * env(d, 0.002, 0.04) * 0.7
    cloth = band(colored(rng, d, 1), 600, 3500) * env(d, 0.008, 0.05)
    grit = grains(rng, d, density(d, 0.003, 0.03, 1500), 600, 5000, (0.0003, 0.0015), 0.7)
    return finish(outdoors(rng, body + cloth * 0.06 + grit * 0.04), -19)


def build_hurt(rng):
    """A punch to the body and a short grunt: a glottal buzz through the formants of 'uh'."""
    d = 0.35
    punch = thump(rng, d, 170, 60, 0.05) + band(colored(rng, d, 2), high=900) * env(d, 0.001, 0.025) * 0.6
    t = t_axis(d)
    f0 = 135 - 30 * t / d
    phase = np.cumsum(f0) / SR
    buzz = (phase % 1.0) * 2 - 1 + white(rng, d) * 0.15
    voice = sum(band(buzz, f * 0.88, f * 1.12) * g for f, g in ((620, 1.0), (1150, 0.5), (2400, 0.2)))
    voice *= env(d, 0.02, 0.07)
    return finish(mix(punch, place_at(d, voice * 0.6, 0.01)), -14)


def build_pickup(rng):
    """A rounded pop rising in pitch, then a softer echo of it a fifth higher."""
    d = 0.16

    def blip(f0, f1, tau):
        t = t_axis(d)
        f = f1 - (f1 - f0) * np.exp(-t / 0.018)
        phase = 2 * np.pi * np.cumsum(f) / SR
        return (np.sin(phase) + 0.15 * np.sin(2 * phase)) * env(d, 0.002, tau)

    return finish(mix(blip(480, 1050, 0.035), place_at(d, blip(720, 1570, 0.025) * 0.35, 0.045)), -20)


def build_ui_click(rng):
    d = 0.06
    tock = modes(rng, d, [(1750, 0.006, 1.0), (3500, 0.003, 0.3), (620, 0.008, 0.5)], 0.0)
    tick = band(white(rng, d), 2500, 9000) * env(d, 0.0002, 0.0008)
    return finish(tock + tick * 0.2, -24)


def build_item_pick(rng):
    """A stack lifted onto the cursor: a soft tick rising in pitch over a puff of cloth."""
    d = 0.09
    t = t_axis(d)
    f = 520 + 380 * (1 - np.exp(-t / 0.02))
    tick = np.sin(2 * np.pi * np.cumsum(f) / SR) * env(d, 0.003, 0.018)
    puff = band(colored(rng, d, 1), 800, 4000) * env(d, 0.002, 0.01)
    return finish(tick * 0.6 + puff * 0.25, -27)


def build_item_place(rng):
    """Items set down into a slot: a soft tap falling in pitch, lower than the pick."""
    d = 0.09
    t = t_axis(d)
    f = 300 + 260 * np.exp(-t / 0.015)
    tap = np.sin(2 * np.pi * np.cumsum(f) / SR) * env(d, 0.001, 0.016)
    tick = band(white(rng, d), 1200, 5000) * env(d, 0.0003, 0.002)
    felt = band(colored(rng, d, 2), high=500) * env(d, 0.001, 0.01)
    return finish(tap * 0.6 + tick * 0.15 + felt * 0.3, -27)


def build_craft(rng):
    """A woody knock: the planks of the crafting table, struck once."""
    d = 0.25
    x = mix(contact_wood(rng, 1.0)[: n_of(d)], thump(rng, d, 260, 140, 0.02) * 0.3)
    x[-n_of(0.05) :] *= np.linspace(1, 0, n_of(0.05)) ** 2
    return finish(x, -21)


def build_refused(rng):
    """A dull low thud, muffled: the action did not go through."""
    d = 0.2
    body = thump(rng, d, 150, 70, 0.045)
    felt = band(colored(rng, d, 2), high=350) * env(d, 0.002, 0.03)
    return finish(band(body + felt * 0.5, high=900), -22)


def build_splash(rng):
    """The entry, a crown of spray, then the air it pulled down bubbling back up."""
    total = 1.0
    entry = band(colored(rng, total, 1), 250, 5000) * env(total, 0.004, 0.07)
    spray = band(white(rng, total), 2000, 9000) * env(total, 0.01, 0.12)
    gulp = thump(rng, total, 320, 140, 0.04)
    parts = [entry * 0.5, spray * 0.12, gulp * 0.2]
    for offset in np.sort(rng.exponential(0.15, 40)):
        if offset > total - 0.1:
            continue
        # A bubble rings at its Minnaert frequency, rising as it shrinks near the surface
        f0 = rng.uniform(350, 1600)
        tau = 0.015 * 900 / f0
        t = t_axis(tau * 8)
        f = f0 * (1 + 2.5 * t)
        bubble = np.sin(2 * np.pi * np.cumsum(f) / SR) * env(tau * 8, 0.001, tau)
        parts.append(place_at(total, bubble * rng.uniform(0.15, 0.5) * np.exp(-offset / 0.25), 0.01 + offset))
    return finish(outdoors(rng, mix(*parts)), -16)


def build_wind(rng):
    """Seamless 20 s loop of gusts: their shape repeats exactly every 20 s, and the noise
    is crossfaded at the seam."""
    d, xf = 20.0, 2.0
    t = t_axis(d + xf)
    # Gusts are a few slow sines with whole numbers of cycles per loop, so they wrap
    gust = sum(np.sin(2 * np.pi * k * t / d + rng.uniform(0, 2 * np.pi)) * a for k, a in ((1, 0.5), (2, 0.35), (3, 0.25), (5, 0.12)))
    gust = (gust - gust.min()) / (gust.max() - gust.min())
    rumble = band(colored(rng, d + xf, 2), 40, 220, order=2)
    rush = band(colored(rng, d + xf, 1), 250, 1400, order=2)
    whistle = band(colored(rng, d + xf, 1), 560, 640, order=3) + band(colored(rng, d + xf, 1), 880, 940, order=3) * 0.5
    y = rumble * (0.6 + 0.4 * gust) + rush * 0.35 * gust**1.5 + whistle * 0.25 * gust**3
    n, k = n_of(d), n_of(xf)
    ramp = np.linspace(0, 1, k)
    y[:k] = y[:k] * np.sqrt(ramp) + y[n : n + k] * np.sqrt(1 - ramp)
    y = y[:n]
    return y * 10 ** ((-24 - loudness(y)) / 20)


# --- weather ----------------------------------------------------------------
# Rain is a texture of thousands of drops, made fast: the drops of one size share a
# few kernels, laid on impulse trains. Thunder follows the tortuous channel of a
# real stroke (Ribner and Roy): each metre of it sends a shock, an N-wave, which
# reaches the ear when its distance says, longer and duller the farther it comes
# from, louder where the channel runs across the line of sight; the crack, the
# claps and the rolling all come from the shape of the channel.

C_AIR = 343.0


def seam(y, d, xf):
    """`y`, `d + xf` seconds long, made a seamless loop of `d` seconds: its last `xf`
    seconds are crossfaded over its start with equal power, as the noise is uncorrelated."""
    n, k = n_of(d), n_of(xf)
    ramp = np.linspace(0, 1, k)
    y = y.copy()
    y[:k] = y[:k] * np.sqrt(ramp) + y[n : n + k] * np.sqrt(1 - ramp)
    return y[:n]


def loop_swell(rng, length, d, depth, cycles=(1, 2, 3, 5)):
    """A slow swell around 1, of whole numbers of cycles every `d` seconds: it wraps at the seam."""
    t = t_axis(length)
    x = sum(np.sin(2 * np.pi * k * t / d + rng.uniform(0, 2 * np.pi)) / k**0.5 for k in cycles)
    return 1 + depth * x / np.max(np.abs(x))


def impulses(rng, n, rate, spread):
    """Poisson times at `rate` a second over `n` samples, with log-normal loudnesses and
    random signs."""
    count = rng.poisson(rate * n / SR)
    at = rng.integers(0, n, count)
    gains = np.minimum(rng.lognormal(0.0, spread, count), 4.0) * rng.choice([-1.0, 1.0], count)
    return at, gains


def drops(rng, duration, rate, low, high, size, spread=0.7, kernels=24):
    """Many short noise bursts at Poisson times: a cloud of drops, as `grains` but fast. Each
    drop takes one of `kernels` bursts, of a decay within `size` and a band within [low, high]."""
    n = n_of(duration)
    at, gains = impulses(rng, n, rate, spread)
    which = rng.integers(0, kernels, len(at))
    edges = np.geomspace(low, high, 5)
    out = np.zeros(n)
    for i in range(kernels):
        tau = np.exp(rng.uniform(np.log(size[0]), np.log(size[1])))
        length = n_of(tau * 6) + 8
        kernel = rng.standard_normal(length) * np.exp(-np.arange(length) / (tau * SR))
        lane = rng.integers(0, 4)
        kernel = band(np.concatenate([kernel, np.zeros(64)]), edges[lane], edges[lane + 1] * 1.6)
        train = np.zeros(n)
        np.add.at(train, at[which == i], gains[which == i])
        out += fftconvolve(train, kernel)[:n]
    return out


def plinks(rng, duration, rate, low, high):
    """Drops falling into the water: each a tiny bubble ringing up in pitch, very short."""
    n = n_of(duration)
    out = np.zeros(n)
    at, gains = impulses(rng, n, rate, 0.6)
    for start, gain in zip(at, gains):
        f0 = rng.uniform(low, high)
        tau = rng.uniform(0.003, 0.008)
        t = t_axis(tau * 6)
        blip = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 30 * t)) / SR) * env(tau * 6, 0.0005, tau)
        end = min(n, start + len(blip))
        out[start:end] += blip[: end - start] * abs(gain)
    return out


def rain_layers(rng, length, d):
    """The rain heard out in the open: a soft broad hiss, a dense mist of fine drops on
    every side, closer drops tapping on the ground and the leaves, a few plinks into
    puddles, and the weight of the downpour far off."""
    swell = loop_swell(rng, length, d, 0.12)
    hiss = band(band(colored(rng, length, 0.8), 350, 9000), high=7000, order=1) * swell
    mist = drops(rng, length, 9000, 2500, 11000, (0.00006, 0.0003), 0.8)
    near = drops(rng, length, 260, 700, 6000, (0.0004, 0.0018), 0.9, kernels=40)
    taps = drops(rng, length, 14, 250, 2500, (0.0015, 0.005), 0.6, kernels=12)
    water = plinks(rng, length, 5, 1300, 3200)
    weight = band(colored(rng, length, 2), 60, 320) * swell
    return hiss, mist, near, taps, water, weight


def build_rain(rng):
    """Seamless 16 s loop of rain heard out in the open."""
    d, xf = 16.0, 2.0
    length = d + xf
    hiss, mist, near, taps, water, weight = rain_layers(rng, length, d)
    y = hiss * 1.0 + mist * 0.11 + near * 0.12 + taps * 0.5 + water * 0.05 + weight * 0.45
    # Softer at the very top: rain soothes, it does not fizz
    y = band(y, high=12000, order=2)
    y = seam(y, d, xf)
    return y * 10 ** ((-22 - loudness(y)) / 20)


def roof(rng, duration, rate, spread, low_modes, high_modes, kernels=16):
    """Drops drumming on a roof of planks: each knocks a few of its low modes, as a hand on a
    table, the modes a little apart from one spot of the roof to the next."""
    n = n_of(duration)
    at, gains = impulses(rng, n, rate, spread)
    which = rng.integers(0, kernels, len(at))
    out = np.zeros(n)
    for i in range(kernels):
        spec = [(rng.uniform(*low_modes), rng.uniform(0.012, 0.03), 1.0), (rng.uniform(*high_modes), rng.uniform(0.004, 0.01), 0.45)]
        kernel = modes(rng, 0.2, spec, 0.04) + band(white(rng, 0.2), 300, 1800) * env(0.2, 0.0003, 0.0015) * 0.3
        train = np.zeros(n)
        np.add.at(train, at[which == i], gains[which == i])
        out += fftconvolve(train, kernel)[:n]
    return out


def build_rain_indoor(rng):
    """Seamless 16 s loop of the same rain heard from under a roof: the hiss through the
    walls, dull, and the drops drumming on the planks overhead, a few heavier ones."""
    d, xf = 16.0, 2.0
    length = d + xf
    hiss, mist, near, taps, water, weight = rain_layers(rng, length, d)
    outside = band(hiss * 1.0 + mist * 0.11 + near * 0.12 + taps * 0.5 + weight * 0.6, high=650, order=4)
    swell = loop_swell(rng, length, d, 0.15)
    drumming = roof(rng, length, 420, 0.7, (120, 260), (420, 900)) * swell
    heavy = roof(rng, length, 3.5, 0.5, (70, 140), (220, 400), kernels=6)
    room = band(colored(rng, length, 2), 40, 180)
    y = outside * 1.0 + drumming * 0.05 + heavy * 0.12 + room * 0.25
    y = band(y, high=2200, order=2)
    y = seam(y, d, xf)
    return y * 10 ** ((-24 - loudness(y)) / 20)


def stroke_channel(rng, height, branches, cloud_reach):
    """The segments of a lightning channel in metres, the strike at the origin and the
    ground the plane y = 0: a tortuous main channel up to the cloud base at `height`, a
    few branches leaving it downward, and channels running through the cloud as far as
    `cloud_reach`. Each segment as (middle, direction, length, strength)."""

    def walk(start, heading, length, step, wander):
        points = [np.array(start, float)]
        direction = np.array(heading, float)
        travelled = 0.0
        while travelled < length:
            turn = rng.standard_normal(3) * wander
            direction = direction + turn
            direction = 0.6 * direction / np.linalg.norm(direction) + 0.4 * np.array(heading, float)
            direction /= np.linalg.norm(direction)
            points.append(points[-1] + direction * step)
            travelled += step
        return points

    lean = rng.standard_normal(3) * np.array([0.25, 0, 0.25])
    main = walk((0, 0, 0), np.array([0, 1, 0]) + lean, height, 4.0, 0.7)
    strands = [(main, 1.0)]
    for _ in range(branches):
        start = main[rng.integers(len(main) // 4, len(main) - 1)]
        heading = rng.standard_normal(3) * np.array([1, 0, 1])
        heading = heading / np.linalg.norm(heading) * 0.8 + np.array([0, -0.6, 0])
        strands.append((walk(start, heading, rng.uniform(0.15, 0.4) * start[1], 4.0, 0.8), 0.6))
    # Through the cloud the channels are fainter, and leave the upper main channel here and there
    for _ in range(4):
        start = main[rng.integers(len(main) * 3 // 4, len(main))]
        heading = rng.standard_normal(3) * np.array([1, 0.2, 1])
        reach = rng.uniform(0.3, 1.0) * cloud_reach
        strands.append((walk(start, heading / np.linalg.norm(heading), reach, 6.0, 0.6), 0.2))
    segments = []
    for strand, strength in strands:
        points = np.array(strand)
        points[:, 1] = np.maximum(points[:, 1], 0.5)
        middles = (points[1:] + points[:-1]) / 2
        steps = points[1:] - points[:-1]
        lengths = np.linalg.norm(steps, axis=1)
        # Fainter toward the tip, and in beads along the way
        along = np.linspace(0, 1, len(lengths))
        beads = np.exp(np.convolve(rng.standard_normal(len(lengths)), np.ones(8) / 8, "same") * 1.2)
        strengths = strength * (1 - 0.85 * along**1.5) * beads
        if strand is not main:
            strengths *= np.minimum(along * 12 + 0.2, 1)
        segments.append((middles, steps / lengths[:, None], lengths, strengths))
    return [np.concatenate(parts) for parts in zip(*segments)]


def n_wave(half):
    """The shock of a segment: a sudden rise, a straight fall through zero, a sudden return."""
    k = max(int(half * SR), 1)
    return np.linspace(1.0, -1.0, 2 * k)


def thunder(rng, distance, duration, height=1600.0, branches=6, cloud_reach=2500.0):
    """Thunder heard from `distance` metres off the strike, the shock of the foot of the
    channel at the start: what a channel through the cloud sends from nearer is dropped.
    Returns the shocks summed, still bright, and when each part of the sound left the
    channel as a distance, to dull it with the distance travelled."""
    middles, directions, lengths, strengths = stroke_channel(rng, height, branches, cloud_reach)
    angle = rng.uniform(0, 2 * np.pi)
    ear = np.array([np.cos(angle) * distance, 1.7, np.sin(angle) * distance])
    to_ear = ear - middles
    r = np.linalg.norm(to_ear, axis=1)
    across = 1 - (np.sum(directions * to_ear, axis=1) / r) ** 2
    gain = strengths * lengths / r**0.6 * (0.15 + across**2) * rng.lognormal(0, 0.35, len(r))
    # The farther a shock comes from, the longer it has stretched
    half = 0.00022 * (r / 40.0) ** 0.6 * rng.lognormal(0, 0.25, len(r))
    n = n_of(duration)
    first = r[0]
    start = (r - first) / C_AIR
    at = (start * SR).astype(int)
    keep = (at >= 0) & (at < n)
    out = np.zeros(n)
    buckets = np.geomspace(half[keep].min(), half[keep].max() * 1.001, 20)
    which = np.digitize(half, buckets)
    for b in np.unique(which[keep]):
        mask = keep & (which == b)
        train = np.zeros(n)
        np.add.at(train, at[mask], gain[mask])
        out += fftconvolve(train, n_wave(np.mean(half[mask])))[:n]
    # The distance each instant of the sound came from
    travelled = first + t_axis(duration) * C_AIR
    return out, travelled


def air(x, travelled, reference):
    """Dulls `x` with the distance each instant travelled through the air, past `reference`
    metres: the cutoff falls as the distance grows."""
    cutoffs = [14000, 7000, 3500, 1800, 900, 450, 220, 110]
    # The cutoff for a distance: 14 kHz at the reference, an octave lower every doubling
    wanted = 14000 * (reference / np.maximum(travelled, reference))
    position = np.clip(np.log2(14000 / wanted), 0, len(cutoffs) - 1)
    out = np.zeros_like(x)
    for i, cutoff in enumerate(cutoffs):
        weight = np.clip(1 - np.abs(position - i), 0, 1)
        if weight.any():
            out += band(x, high=cutoff, order=2) * weight
    return out


def echoes(rng, x, decay, wet, high, early=6):
    """The thunder thrown back by the hills and the clouds: a few early echoes, then a long
    dark tail."""
    d = decay * 4
    ir = band(white(rng, d), high=high, order=2) * env(d, 0.08, decay)
    for _ in range(early):
        ir += place_at(d, np.array([rng.uniform(0.3, 1.0)]), rng.uniform(0.12, 1.4)) * 30
    ir = band(ir, high=high * 1.5)
    ir /= np.sqrt(np.sum(ir**2))
    return mix(x, fftconvolve(x, ir)[: len(x) + n_of(d)] * wet)


def rumble_bed(rng, x, duration, low=25, high=110, smooth=0.12):
    """A low roll that follows the loudness of `x`: the rumble carries what the shocks only
    hint at below 100 Hz."""
    level = np.sqrt(np.convolve(x**2, np.ones(n_of(smooth)) / n_of(smooth), "same"))
    return band(colored(rng, duration, 2), low, high, order=2) * level / (np.max(level) or 1.0)


def roll(rng, duration, rate, depth):
    """A slow random swell around 1, about `rate` swells a second, `depth` decibels deep: the
    thunder rolls in waves, as its sound comes from farther and farther parts of the sky."""
    n = n_of(duration)
    slow = band(rng.standard_normal(n), high=rate, order=2)
    slow /= np.std(slow)
    return 10 ** (depth * np.tanh(slow) / 20)


def fade_out(x, duration, length):
    x = x[: n_of(duration)].copy()
    k = n_of(length)
    x[-k:] *= np.linspace(1, 0, k) ** 2
    return x


def limit(x, head):
    """`x` through a soft limiter set by its loudest part in the first `head` seconds: the
    shocks pile up, as the ear and any microphone hear a close strike."""
    level = np.percentile(np.abs(x[: n_of(head)]), 99.5)
    return np.tanh(x / level)


def build_thunder_near(rng):
    """A strike close by: a tearing crack of shocks, a deep boom, then the long roll of the
    rest of the channel and its echoes from the hills."""
    d = 9.0
    shocks, travelled = thunder(rng, rng.uniform(40, 70), d, branches=rng.integers(4, 8))
    bright = air(limit(shocks, 0.4), travelled, 60)
    boom = band(colored(rng, d, 2), 28, 160, order=2) * env(d, 0.025, 0.35)
    thud = thump(rng, d, 90, 45, 0.12)
    bed = rumble_bed(rng, bright, d, 28, 140)
    tail = mix(np.ones(n_of(0.6)), roll(rng, d, 1.2, 4))[: n_of(d)]
    x = (bright * 0.8 + bed * 1.5) * tail + place_at(d, boom * 3.0 + thud * 0.5, rng.uniform(0.03, 0.07))
    x = echoes(rng, x, 0.9, 0.35, 1800)
    x = band(x, low=28)
    return finish(fade_out(x, d, 2.0), -11)


def build_thunder_far(rng):
    """A strike far off: no crack left, a low roll swelling and fading over seconds."""
    d = 11.0
    shocks, travelled = thunder(rng, rng.uniform(2500, 4500), d, branches=rng.integers(5, 9), cloud_reach=4000.0)
    dull = air(shocks, travelled, 150)
    bed = rumble_bed(rng, dull, d, 22, 90, 0.25)
    x = (dull / np.max(np.abs(dull)) * 0.6 + bed * 1.5) * roll(rng, d, 0.9, 6)
    x = echoes(rng, x, 1.4, 0.5, 600)
    x = band(band(x, low=24), high=1200, order=2)
    swell = np.minimum(t_axis(len(x) / SR) / 0.25, 1.0)
    return finish(fade_out(x * swell, d, 3.0), -17)


# --- creatures --------------------------------------------------------------
# Every creature has a voice of its own. The animals' are a glottal buzz, its pitch
# following a curve, through the formants of a vowel (or two, crossfaded, for a
# vowel that moves); the hostiles are made of what they are: a rat's chitter, a
# bush's twigs, a boulder's grinding, a stump's creaking wood, water, fire, wind.
# Each has a cry heard now and then, and a hurt; each hostile an approach, heard
# while it hunts, and an attack.


def curve(d, *points):
    """A value over `d` seconds, linear between `points`, given as (time, value) pairs."""
    t = t_axis(d)
    times, values = zip(*points)
    return np.interp(t, times, values)


def vibrato(d, rate, depth, rng=None):
    """Around 1, `depth` deep, `rate` times a second."""
    t = t_axis(d)
    phase = rng.uniform(0, 2 * np.pi) if rng is not None else 0.0
    return 1 + depth * np.sin(2 * np.pi * rate * t + phase)


def glottal(rng, f0, noise=0.1, jitter=0.01):
    """A buzz at the pitch `f0` (an array): a sawtooth whose pitch wanders a little, with some
    breath."""
    n = len(f0)
    wander = 1 + jitter * band(rng.standard_normal(n), high=40) * 6
    phase = np.cumsum(f0 * wander) / SR
    saw = (phase % 1.0) * 2 - 1
    return saw + rng.standard_normal(n) * noise


def formants(x, spec, q=0.12):
    """`x` through a bank of band-passes, spec = [(freq, gain), ...]: a vowel."""
    return sum(band(x, f * (1 - q), f * (1 + q)) * g for f, g in spec)


def vowel(x, a, b, morph):
    """`x` through the vowel `a` turning into `b` as `morph` goes from 0 to 1."""
    return formants(x, a) * (1 - morph) + formants(x, b) * morph


VOWEL_A = [(800, 1.0), (1200, 0.6), (2500, 0.2)]
VOWEL_E = [(530, 1.0), (1850, 0.5), (2500, 0.2)]
VOWEL_I = [(300, 0.8), (2250, 0.6), (3000, 0.2)]
VOWEL_O = [(500, 1.0), (850, 0.6), (2400, 0.1)]
VOWEL_U = [(320, 1.0), (750, 0.4), (2300, 0.05)]
NASAL = [(250, 1.0), (1000, 0.4), (2200, 0.15)]


def sweep(d, f0, f1, tau):
    """A sine whose pitch glides from `f0` to `f1`, `tau` the time of the glide."""
    t = t_axis(d)
    f = f1 + (f0 - f1) * np.exp(-t / tau)
    return np.sin(2 * np.pi * np.cumsum(f) / SR)


def punch(rng, d, f0=150, amp=1.0):
    """The blow that comes before a hurt."""
    return (thump(rng, d, f0, f0 * 0.38, 0.04) + band(colored(rng, d, 2), high=800) * env(d, 0.001, 0.02) * 0.6) * amp


def hurt_of(rng, voice, lufs=-16, f0=150):
    """A blow, then `voice`, a short cry of pain."""
    d = len(voice) / SR + 0.05
    return finish(mix(punch(rng, d, f0), place_at(d, voice, 0.015)), lufs)


def whoosh(rng, d, low, high, attack, tau):
    """Air rushing past: noise through a band that opens and closes with its swell."""
    x = band(colored(rng, d, 1), low, high, order=2)
    return x * env(d, attack, tau)


def crackle(rng, d, rate, low=1500, high=9000):
    """Fire: sharp grains at random, over a soft hiss."""
    pops = grains(rng, d, np.full(n_of(d), rate), low, high, (0.0001, 0.0006), 1.2, bands=4)
    hiss = band(colored(rng, d, 1), 2000, 8000) * 0.15
    return pops * 0.5 + hiss


def bubbles(rng, d, rate, low=300, high=900):
    """Water: little bubbles rising in pitch as they close, at random."""
    out = np.zeros(n_of(d))
    for _ in range(rng.poisson(rate * d)):
        f0 = rng.uniform(low, high)
        tau = rng.uniform(0.008, 0.025)
        length = tau * 6
        t = t_axis(length)
        bubble = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 3 * t)) / SR) * env(length, 0.001, tau)
        out += place_at(d, bubble * rng.uniform(0.3, 1.0), rng.uniform(0, max(d - length, 0.0)))
    return out


def creak(rng, d, f0, rate, q=0.15):
    """Wood bending: a slow train of clicks, a stick-slip, through the formants of a hollow
    log."""
    pulses = np.zeros(n_of(d))
    t = 0.0
    while t < d:
        pulses[min(n_of(t), len(pulses) - 1)] = rng.uniform(0.5, 1.0)
        t += 1.0 / (rate * rng.uniform(0.8, 1.2))
    body = formants(pulses, [(f0, 1.0), (f0 * 2.3, 0.6), (f0 * 4.1, 0.3)], q)
    return body


# Animals


def sheep_voice(rng, d=0.9, pitch=1.0):
    """Baa: a bleating buzz, its loudness pulsing, through an 'a' that closes to an 'e'."""
    f0 = curve(d, (0, 290), (0.15, 320), (d, 260)) * pitch * vibrato(d, 7, 0.03, rng)
    x = vowel(glottal(rng, f0, 0.12), VOWEL_A, VOWEL_E, curve(d, (0, 0), (d, 1)))
    bleat = 0.65 + 0.35 * np.sin(2 * np.pi * 7.5 * t_axis(d)) ** 2
    return x * bleat * env(d, 0.05, d * 0.6)


def cow_voice(rng, d=1.6, pitch=1.0):
    """Moo: a low buzz through an 'm' opening to an 'oo' then an 'a'."""
    f0 = curve(d, (0, 105), (0.4, 120), (d, 92)) * pitch * vibrato(d, 4, 0.01, rng)
    buzz = glottal(rng, f0, 0.05)
    x = vowel(buzz, NASAL, VOWEL_U, curve(d, (0, 0), (0.25, 1), (d, 1))) * (1 - curve(d, (0, 0), (0.7, 0), (d, 0.6)))
    x += formants(buzz, VOWEL_O) * curve(d, (0, 0), (0.7, 0), (d, 0.6))
    return band(x, high=2500) * env(d, 0.12, d * 0.7)


def pig_voice(rng, d=0.55, pitch=1.0):
    """Two snorting grunts: a low rough buzz through the nose."""
    out = np.zeros(n_of(d))
    for start, length in ((0.0, 0.17), (0.24, 0.22)):
        f0 = curve(length, (0, 120), (length, 85)) * pitch * rng.uniform(0.95, 1.05)
        x = formants(glottal(rng, f0, 0.35, 0.04), [(350, 1.0), (1100, 0.5), (2400, 0.2)], 0.2)
        out += place_at(d, x * env(length, 0.01, 0.06), start)
    return out


def chicken_voice(rng, d=0.9, pitch=1.0):
    """Clucks: three short dry clucks, then a longer 'bawk'."""
    out = np.zeros(n_of(d))
    for start in (0.0, 0.11, 0.22):
        length = 0.06
        f0 = curve(length, (0, 520), (length, 430)) * pitch
        x = formants(glottal(rng, f0, 0.2), [(900, 1.0), (1900, 0.5)], 0.15)
        out += place_at(d, x * env(length, 0.003, 0.02), start)
    length = 0.4
    f0 = curve(length, (0, 450), (0.08, 620), (length, 480)) * pitch
    x = vowel(glottal(rng, f0, 0.15), VOWEL_A, VOWEL_O, curve(length, (0, 0), (length, 1)))
    out += place_at(d, x * env(length, 0.01, 0.12), 0.38)
    return out


def fox_voice(rng, d=0.75, pitch=1.0):
    """Two shrill yips, the second higher, hoarse."""
    out = np.zeros(n_of(d))
    for start, top in ((0.0, 900), (0.3, 1050)):
        length = 0.22
        f0 = curve(length, (0, top * 0.7), (0.05, top), (length, top * 0.6)) * pitch
        x = vowel(glottal(rng, f0, 0.3, 0.03), VOWEL_E, VOWEL_A, curve(length, (0, 0), (length, 1)))
        out += place_at(d, x * env(length, 0.01, 0.07), start)
    return out


def wolf_voice(rng, d=2.2, pitch=1.0):
    """A howl: an almost pure tone rising, held, then falling, through an 'oo'."""
    f0 = curve(d, (0, 330), (0.5, 520), (1.5, 500), (d, 380)) * pitch * vibrato(d, 5, 0.012, rng)
    phase = 2 * np.pi * np.cumsum(f0) / SR
    tone = np.sin(phase) + 0.35 * np.sin(2 * phase) + 0.12 * np.sin(3 * phase)
    breath = band(colored(rng, d, 1), 600, 2500) * 0.05
    return (tone + breath) * env(d, 0.3, d * 0.5) * curve(d, (0, 1), (d - 0.4, 1), (d, 0.2))


def cat_voice(rng, d=0.8, pitch=1.0):
    """Meow: an 'i' opening to an 'a', closing to an 'u', its pitch rising then falling."""
    f0 = curve(d, (0, 520), (0.3, 720), (d, 430)) * pitch
    buzz = glottal(rng, f0, 0.06)
    morph = curve(d, (0, 0), (0.35, 1), (d, 1))
    x = vowel(buzz, VOWEL_I, VOWEL_A, morph) * (1 - curve(d, (0, 0), (0.45, 0), (d, 1)))
    x += formants(buzz, VOWEL_U) * curve(d, (0, 0), (0.45, 0), (d, 1))
    return x * env(d, 0.06, d * 0.45)


def rabbit_voice(rng, d=0.6, pitch=1.0):
    """A rabbit is quiet: three quick sniffs and a soft thump of its foot."""
    out = np.zeros(n_of(d))
    for start in (0.0, 0.09, 0.18):
        length = 0.06
        sniff = band(colored(rng, length, 1), 2500, 7000) * env(length, 0.01, 0.015)
        out += place_at(d, sniff * 0.5, start)
    out += place_at(d, thump(rng, 0.2, 110 * pitch, 55, 0.04), 0.38)
    return out


def horse_voice(rng, d=1.5, pitch=1.0):
    """A whinny: a high quavering cry falling away, then a snort."""
    body = d - 0.35
    f0 = curve(body, (0, 650), (0.15, 820), (body, 380)) * pitch * vibrato(body, 11, 0.06, rng)
    x = vowel(glottal(rng, f0, 0.15, 0.02), VOWEL_I, VOWEL_A, curve(body, (0, 0), (body, 1)))
    x *= env(body, 0.04, body * 0.5)
    snort = band(colored(rng, 0.3, 1), 300, 2500) * env(0.3, 0.01, 0.08)
    snort *= 1 + 0.6 * np.sin(2 * np.pi * 30 * t_axis(0.3))
    return mix(x, place_at(d, snort * 0.8, body))


def goat_voice(rng, d=0.8, pitch=1.0):
    """Meh: a nasal bleat, higher and harsher than a sheep's, quavering fast."""
    f0 = curve(d, (0, 380), (0.1, 430), (d, 360)) * pitch * vibrato(d, 13, 0.05, rng)
    x = vowel(glottal(rng, f0, 0.2, 0.03), NASAL, VOWEL_E, curve(d, (0, 0), (0.12, 1), (d, 1)))
    return x * env(d, 0.03, d * 0.5)


ANIMALS = {
    "sheep": (sheep_voice, -20),
    "cow": (cow_voice, -19),
    "pig": (pig_voice, -19),
    "chicken": (chicken_voice, -21),
    "fox": (fox_voice, -21),
    "wolf": (wolf_voice, -21),
    "cat": (cat_voice, -21),
    "rabbit": (rabbit_voice, -24),
    "horse": (horse_voice, -19),
    "goat": (goat_voice, -20),
}


def build_animal_cry(rng, animal):
    voice, lufs = ANIMALS[animal]
    return finish(outdoors(rng, voice(rng, pitch=rng.uniform(0.95, 1.05))), lufs)


def build_animal_hurt(rng, animal):
    """The animal's own voice, short, strained and higher."""
    voice, _ = ANIMALS[animal]
    x = voice(rng, pitch=1.25)
    x = x[: n_of(0.35)] * curve(0.35, (0, 1), (0.25, 1), (0.35, 0))[: len(x[: n_of(0.35)])]
    return hurt_of(rng, x * 0.8, -16)


# Hostiles


def gnawer_chitter(rng, d, rate, top=3200):
    """Quick squeaks, one after the other."""
    out = np.zeros(n_of(d))
    t = 0.0
    while t < d - 0.05:
        length = rng.uniform(0.025, 0.05)
        f = rng.uniform(top * 0.75, top)
        squeak = sweep(length, f, f * rng.uniform(0.8, 1.15), 0.01) * env(length, 0.003, length / 3)
        out += place_at(d, squeak * rng.uniform(0.5, 1.0), t)
        t += rng.exponential(1 / rate) + length
    return out


def build_gnawer_cry(rng):
    return finish(outdoors(rng, gnawer_chitter(rng, 0.6, 10)), -22)


def build_gnawer_approach(rng):
    """It scurries, chattering its teeth, squeaking fast."""
    d = 1.4
    teeth = grains(rng, d, np.full(n_of(d), 60), 3000, 9000, (0.0002, 0.0006), 0.5, bands=3)
    paws = grains(rng, d, np.full(n_of(d), 25), 300, 2000, (0.001, 0.003), 0.5, bands=3)
    x = gnawer_chitter(rng, d, 16, 3800) + teeth * 0.4 + paws * 0.25
    return finish(outdoors(rng, x * curve(d, (0, 0.3), (0.3, 1), (d, 1))), -20)


def build_gnawer_attack(rng):
    """A hiss, then the snap of its teeth."""
    d = 0.35
    hiss = band(white(rng, d), 3000, 9000) * env(d, 0.02, 0.06)
    snap = modes(rng, d, [(2600, 0.01, 1.0), (4100, 0.006, 0.6)]) + band(white(rng, d), 2000, 10000) * env(d, 0.0002, 0.002)
    return finish(mix(hiss * 0.5, place_at(d, snap, 0.1)), -16)


def build_gnawer_hurt(rng):
    d = 0.25
    squeal = sweep(d, 4200, 2600, 0.15) * env(d, 0.005, 0.07)
    return hurt_of(rng, squeal * 0.5, -17, f0=190)


def twigs(rng, d, rate, s=1.0):
    """Dry twigs rubbing and cracking."""
    rustle = grains(rng, d, np.full(n_of(d), rate), 1500, 8000, (0.0002, 0.001), 1.0)
    cracks = grains(rng, d, np.full(n_of(d), rate / 40), 600, 3000, (0.001, 0.004), 1.0, bands=3)
    return rustle * 0.07 + cracks * 0.3 * s


def build_bramble_cry(rng):
    """Its branches rustle, and one creaks."""
    d = 1.2
    x = twigs(rng, d, 2500) * curve(d, (0, 0), (0.3, 1), (d, 0.3)) + creak(rng, d, 420, 35) * 0.15 * curve(d, (0, 0), (0.4, 1), (d, 0))
    return finish(outdoors(rng, x), -24)


def build_bramble_approach(rng):
    """A thicket shaking harder and harder."""
    d = 1.6
    x = twigs(rng, d, 7000, 1.5) * curve(d, (0, 0.3), (d * 0.8, 1), (d, 0.6))
    x += band(colored(rng, d, 1), 800, 4000) * 0.05 * curve(d, (0, 0), (d, 1))
    return finish(outdoors(rng, x), -20)


def build_bramble_attack(rng):
    """A thorny whip lashing through the air, then its crack."""
    d = 0.4
    lash = band(colored(rng, d, 1), 1500, 6000) * env(d, 0.08, 0.03)
    crack = band(white(rng, d), 1200, 9000) * env(d, 0.0003, 0.004) + modes(rng, d, [(1100, 0.02, 1.0), (2300, 0.01, 0.5)])
    return finish(mix(lash * 0.5, place_at(d, crack, 0.1)), -15)


def build_bramble_hurt(rng):
    d = 0.3
    snap = band(white(rng, d), 800, 6000) * env(d, 0.0003, 0.006) + twigs(rng, d, 9000, 2.0) * env(d, 0.005, 0.08)
    return hurt_of(rng, snap, -17, f0=200)


def grind(rng, d, low, high, rate):
    """Stone on stone: rough gravelly noise in a low band, shaken at `rate`."""
    x = band(colored(rng, d, 1.5), low, high, order=2)
    shake = 1 + 0.5 * np.sin(2 * np.pi * rate * t_axis(d)) + 0.3 * band(rng.standard_normal(n_of(d)), high=rate * 2) * 6
    grit = grains(rng, d, np.full(n_of(d), 400), 800, 4000, (0.0005, 0.002), 0.9, bands=3)
    return x * np.clip(shake, 0, None) + grit * 0.05


def build_roller_cry(rng):
    """A low grinding, a boulder shifting where it lies."""
    d = 1.4
    return finish(outdoors(rng, grind(rng, d, 50, 400, 3) * env(d, 0.3, 0.6)), -22)


def build_roller_approach(rng):
    """It rolls: a heavy rumble beating once a turn."""
    d = 2.0
    beats = sum(place_at(d, thump(rng, 0.3, 90, 45, 0.08), t) for t in np.arange(0.0, d - 0.3, 0.32))
    x = grind(rng, d, 40, 600, 3.1) * 0.6 + beats * 0.8
    return finish(outdoors(rng, x * curve(d, (0, 0.2), (0.4, 1), (d, 1))), -17)


def build_roller_attack(rng):
    """Its wind-up: a grinding that rises, as it gathers itself."""
    d = 1.0
    t = t_axis(d)
    x = grind(rng, d, 60, 500, 4) * (0.3 + t)
    scrape = band(colored(rng, d, 1), 700, 2500) * curve(d, (0, 0), (d, 1)) * 0.3
    return finish(x + scrape, -17)


def build_roller_hurt(rng):
    d = 0.4
    knock = contact_stone(rng, 1.2)
    rubble_bits = rubble(rng, d, 5, 1500, 4000)
    return finish(mix(knock, rubble_bits * 0.3), -15)


def groan(rng, d, f0_curve, q=0.15):
    """A deep wooden groan: a slow creak through a hollow trunk's formants."""
    f0 = curve(d, *f0_curve)
    buzz = glottal(rng, f0, 0.25, 0.05)
    return formants(buzz, [(220, 1.0), (520, 0.6), (1300, 0.2)], q)


def build_stumpling_cry(rng):
    """A long creak of old wood, low, groaning."""
    d = 1.8
    x = groan(rng, d, ((0, 55), (0.8, 70), (d, 50))) * env(d, 0.3, 0.9) + creak(rng, d, 300, 18) * 0.2 * env(d, 0.4, 0.6)
    return finish(outdoors(rng, x), -20)


def build_stumpling_approach(rng):
    """Heavy wooden steps, groaning as it comes."""
    d = 2.0
    steps = sum(place_at(d, contact_wood(rng, 1.0) + thump(rng, 0.35, 100, 40, 0.08), t) for t in (0.1, 0.75, 1.4))
    x = steps * 0.8 + groan(rng, d, ((0, 60), (d, 75))) * 0.5 * env(d, 0.6, 1.0)
    return finish(outdoors(rng, x), -17)


def build_stumpling_attack(rng):
    """A heavy branch swung through the air, then the blow."""
    d = 0.6
    swing = whoosh(rng, d, 150, 900, 0.18, 0.05)
    blow = mix(thump(rng, d, 130, 45, 0.08), contact_wood(rng, 1.2))
    return finish(mix(swing * 0.6, place_at(d, blow, 0.22)), -14)


def build_stumpling_hurt(rng):
    d = 0.45
    x = mix(contact_wood(rng, 1.2), groan(rng, d, ((0, 85), (d, 60))) * env(d, 0.02, 0.15) * 0.5)
    return finish(x, -15)


def gurgle(rng, d, f0_curve):
    """A drowned moan: a low voice through 'u', bubbling."""
    f0 = curve(d, *f0_curve) * vibrato(d, 6, 0.05, rng)
    voice = formants(glottal(rng, f0, 0.1, 0.03), VOWEL_U, 0.2)
    # Water in the throat: the voice broken up by bubbles
    wet = 0.6 + 0.4 * np.sin(2 * np.pi * 9 * t_axis(d) + rng.uniform(0, 6))
    return band(voice * wet, high=1800) + bubbles(rng, d, 25) * 0.3


def build_drowner_cry(rng):
    d = 1.6
    x = gurgle(rng, d, ((0, 90), (0.5, 105), (d, 80))) * env(d, 0.3, 0.7)
    return finish(outdoors(rng, x), -21)


def build_drowner_approach(rng):
    """A moan rising, water churning."""
    d = 1.8
    x = gurgle(rng, d, ((0, 80), (d, 130))) * curve(d, (0, 0.2), (d * 0.8, 1), (d, 0.6))
    x += bubbles(rng, d, 60, 200, 700) * 0.4 + band(colored(rng, d, 1), 300, 2000) * 0.05
    return finish(outdoors(rng, x), -18)


def build_drowner_attack(rng):
    """It lunges: a splash, and a gurgled roar."""
    d = 0.8
    splash = band(white(rng, d), 600, 7000) * env(d, 0.002, 0.08) + bubbles(rng, d, 80) * 0.5
    roar = gurgle(rng, d, ((0, 140), (d, 90))) * env(d, 0.03, 0.25)
    return finish(mix(splash * 0.6, roar), -15)


def build_drowner_hurt(rng):
    d = 0.35
    choke = gurgle(rng, d, ((0, 160), (d, 110))) * env(d, 0.01, 0.1)
    return hurt_of(rng, choke + bubbles(rng, d, 120) * 0.4, -16)


def flame(rng, d, low=150, high=1500):
    """A flame's roar: low noise, fluttering."""
    roar = band(colored(rng, d, 1.5), low, high, order=2)
    flutter = 1 + 0.4 * band(rng.standard_normal(n_of(d)), high=12) * 8
    return roar * np.clip(flutter, 0.2, None)


def build_cinderling_cry(rng):
    """It burns: a crackle and a breath of flame."""
    d = 1.4
    x = crackle(rng, d, 40) + flame(rng, d) * 0.4 * env(d, 0.4, 0.5)
    return finish(outdoors(rng, x * env(d, 0.1, 1.0)), -22)


def build_cinderling_approach(rng):
    """Its fire roars up."""
    d = 1.8
    x = flame(rng, d, 100, 2000) * curve(d, (0, 0.2), (d * 0.7, 1), (d, 0.7)) + crackle(rng, d, 90) * 0.6
    return finish(outdoors(rng, x), -18)


def build_cinderling_attack(rng):
    """It throws an ember: a breath drawn in, then a burst of flame flung away."""
    d = 0.8
    draw = flame(rng, d, 200, 1200) * curve(d, (0, 0), (0.25, 1), (0.3, 0), (d, 0)) * 0.6
    burst = whoosh(rng, d, 300, 3000, 0.02, 0.15)
    pop = thump(rng, d, 220, 70, 0.05)
    return finish(mix(draw, place_at(d, burst + pop * 0.5 + crackle(rng, d, 200)[: n_of(d)] * 0.3, 0.3)), -15)


def build_cinderling_hurt(rng):
    d = 0.4
    sizzle = band(white(rng, d), 3000, 10000) * env(d, 0.005, 0.12)
    crack = modes(rng, d, [(900, 0.03, 1.0), (1700, 0.02, 0.5), (2900, 0.01, 0.3)])
    return hurt_of(rng, sizzle * 0.5 + crack * 0.4, -16, f0=170)


def build_ember_cry(rng):
    """An ember in flight: a short fizzing whoosh."""
    d = 0.7
    x = whoosh(rng, d, 500, 4000, 0.05, 0.2) + crackle(rng, d, 150) * env(d, 0.01, 0.3)
    return finish(x, -20)


def hoot(rng, length, f0):
    """One soft hoot: a near sine through 'u', breathy."""
    f = curve(length, (0, f0 * 0.92), (0.05, f0), (length, f0 * 0.9))
    phase = 2 * np.pi * np.cumsum(f) / SR
    tone = np.sin(phase) + 0.15 * np.sin(2 * phase)
    breath = band(colored(rng, length, 1), f0, f0 * 4) * 0.08
    return (tone + breath) * env(length, 0.04, length * 0.35)


def build_gust_owl_cry(rng):
    """Hoo, hoo-hoo, hoooo."""
    d = 1.8
    x = np.zeros(n_of(d))
    for start, length, f in ((0.0, 0.3, 360), (0.5, 0.15, 380), (0.7, 0.15, 380), (0.95, 0.6, 340)):
        x += place_at(d, hoot(rng, length, f), start)
    return finish(outdoors(rng, x, 0.25), -21)


def build_gust_owl_approach(rng):
    """It circles overhead: great wing beats in a rushing wind."""
    d = 2.0
    beats = sum(place_at(d, whoosh(rng, 0.35, 150, 1200, 0.12, 0.08), t) for t in np.arange(0.0, d - 0.35, 0.45))
    wind = band(colored(rng, d, 1), 300, 2500) * curve(d, (0, 0.2), (1.0, 0.6), (d, 0.3))
    return finish(outdoors(rng, beats + wind * 0.3), -18)


def build_gust_owl_attack(rng):
    """It dives: a screech falling away over the wind of its dive."""
    d = 1.0
    f0 = curve(d, (0, 1700), (0.15, 2100), (d, 1200)) * vibrato(d, 25, 0.03, rng)
    screech = formants(glottal(rng, f0, 0.5, 0.05), [(2500, 1.0), (4000, 0.6), (6000, 0.3)], 0.2) * env(d, 0.03, 0.35)
    dive = whoosh(rng, d, 400, 3000, 0.4, 0.3)
    return finish(mix(screech, dive * 0.5), -15)


def build_gust_owl_hurt(rng):
    d = 0.3
    f0 = curve(d, (0, 1400), (d, 900))
    squawk = formants(glottal(rng, f0, 0.4), [(1800, 1.0), (3200, 0.5)], 0.2) * env(d, 0.005, 0.09)
    feathers = band(colored(rng, d, 1), 2000, 7000) * env(d, 0.01, 0.06)
    return hurt_of(rng, squawk + feathers * 0.3, -16)


HOSTILES = {
    "gnawer": (build_gnawer_cry, build_gnawer_approach, build_gnawer_attack, build_gnawer_hurt),
    "bramble": (build_bramble_cry, build_bramble_approach, build_bramble_attack, build_bramble_hurt),
    "roller": (build_roller_cry, build_roller_approach, build_roller_attack, build_roller_hurt),
    "stumpling": (build_stumpling_cry, build_stumpling_approach, build_stumpling_attack, build_stumpling_hurt),
    "drowner": (build_drowner_cry, build_drowner_approach, build_drowner_attack, build_drowner_hurt),
    "cinderling": (build_cinderling_cry, build_cinderling_approach, build_cinderling_attack, build_cinderling_hurt),
    "gust_owl": (build_gust_owl_cry, build_gust_owl_approach, build_gust_owl_attack, build_gust_owl_hurt),
}


# --- output -----------------------------------------------------------------


def rng_for(name):
    """Each file has its own seed, so changing one sound leaves the others as they are."""
    return np.random.default_rng([SEED, zlib.crc32(name.encode())])


def write_ogg(name, build, *args):
    if not name.startswith(ONLY):
        return
    path = OUT / f"{name}.ogg"
    x = build(rng_for(name), *args)
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".wav") as tmp:
        wavfile.write(tmp.name, SR, (np.clip(x, -1, 1) * 32767).astype(np.int16))
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", tmp.name, "-c:a", "libvorbis", "-q:a", "5", str(path)],
            check=True,
        )
    print(path.relative_to(OUT))


def main():
    for material in WALKABLE:
        for i in range(1, 5):
            write_ogg(f"step/{material}{i}", build_step, material)
    for material in CONTACTS:
        for i in range(1, 4):
            write_ogg(f"dig/{material}{i}", build_hit, material)
            write_ogg(f"break/{material}{i}", build_break, material)
        write_ogg(f"place/{material}", build_place, material)
    write_ogg("player/land", build_land)
    write_ogg("player/hurt", build_hurt)
    write_ogg("player/pickup", build_pickup)
    for animal in ANIMALS:
        for i in range(1, 3):
            write_ogg(f"mob/{animal}/cry{i}", build_animal_cry, animal)
        write_ogg(f"mob/{animal}/hurt", build_animal_hurt, animal)
    for hostile, (cry, approach, attack, hurt) in HOSTILES.items():
        for i in range(1, 3):
            write_ogg(f"mob/{hostile}/cry{i}", cry)
        write_ogg(f"mob/{hostile}/approach", approach)
        write_ogg(f"mob/{hostile}/attack", attack)
        write_ogg(f"mob/{hostile}/hurt", hurt)
    write_ogg("mob/ember/cry1", build_ember_cry)
    write_ogg("ui/click", build_ui_click)
    write_ogg("ui/item_pick", build_item_pick)
    write_ogg("ui/item_place", build_item_place)
    write_ogg("ui/craft", build_craft)
    write_ogg("ui/refused", build_refused)
    write_ogg("liquid/splash", build_splash)
    write_ogg("ambient/wind", build_wind)
    write_ogg("weather/rain", build_rain)
    write_ogg("weather/rain_indoor", build_rain_indoor)
    for i in range(1, 4):
        write_ogg(f"weather/thunder_near{i}", build_thunder_near)
        write_ogg(f"weather/thunder_far{i}", build_thunder_far)


if __name__ == "__main__":
    main()
