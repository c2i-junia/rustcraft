#!/usr/bin/env python3
"""Procedurally generates the game's sound effects into data/sounds/.

Every sound is synthesised with a fixed seed, so the output is reproducible and
entirely original (MIT, like the code). Crumbly materials (grass, dirt, sand,
snow, leaves) are clouds of tiny filtered grains, after Perry Cook's PhISEM;
hard ones (stone, wood, glass) are banks of damped resonant modes. Footsteps
are a heel then a toe, every sound gets a faint outdoor reflection, and each
kind of sound is normalised to one loudness (ITU-R BS.1770 K-weighting).

Requirements: numpy, scipy, and ffmpeg built with libvorbis.
Usage: python3 scripts/generate_sounds.py [output_dir]
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
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else ROOT / "data" / "sounds"
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


# --- output -----------------------------------------------------------------


def rng_for(name):
    """Each file has its own seed, so changing one sound leaves the others as they are."""
    return np.random.default_rng([SEED, zlib.crc32(name.encode())])


def write_ogg(name, build, *args):
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
    write_ogg("ui/click", build_ui_click)
    write_ogg("ui/item_pick", build_item_pick)
    write_ogg("ui/item_place", build_item_place)
    write_ogg("ui/craft", build_craft)
    write_ogg("ui/refused", build_refused)
    write_ogg("liquid/splash", build_splash)
    write_ogg("ambient/wind", build_wind)


if __name__ == "__main__":
    main()
