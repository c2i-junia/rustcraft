#!/usr/bin/env python3
"""Procedurally generates the sounds of the later features into data/sounds/: the armour, the
bow and its arrows, the experience, the engraving table, the anvil, the trade and the villagers' voices,
the breeding and the taming, a mob falling into water, and the stings of the progress (a depth
reached, an achievement, a boss, the victory, the credits, a skin imported), and the critical
blows and the sweeps of the melee, and the mounts: a saddle put on, a boat's oars and a blow on its
hull, and the shield's parry and the crossbow's loading and shot, and the fishing: a rod cast,
a bite and the line drawn in.

Built with the primitives of `generate_sounds.py` (its seed, its loudness and its output), so
that they sound of a piece with the others; every sound is synthesised here, entirely original
(MIT, like the code). The chimes are struck bars and small bells, of our own tuning; metal is
banks of inharmonic modes, leather and cloth filtered noise, the bow a plucked string over a
wooden stave that creaks by stick-slip.

Requirements: numpy, scipy, and ffmpeg built with libvorbis.
Usage: python3 scripts/generate_feature_sounds.py [output_dir] [--only PREFIX]
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generate_sounds import (
    NASAL,
    VOWEL_A,
    VOWEL_E,
    VOWEL_I,
    VOWEL_O,
    VOWEL_U,
    band,
    colored,
    contact_stone,
    contact_wood,
    curve,
    env,
    finish,
    formants,
    glottal,
    grains,
    density,
    hurt_of,
    mix,
    modes,
    n_of,
    outdoors,
    place_at,
    rubble,
    t_axis,
    thump,
    vibrato,
    white,
    whoosh,
    write_ogg,
)

# --- shared voices ----------------------------------------------------------


def bell(rng, d, f0, tau, ratios=(1.0, 2.0, 2.99, 4.15), bright=0.5):
    """A small struck bell or bar: partials over `f0`, the higher dying sooner."""
    spec = [(f0 * r, tau / r**0.6, bright ** i) for i, r in enumerate(ratios)]
    return modes(rng, d, spec, 0.003)


def notes(rng, d, seq, tau=0.3, ratios=(1.0, 2.0, 2.99, 4.15), bright=0.5):
    """Bells struck one after the other, seq = [(time, freq, gain), ...]."""
    return mix(np.zeros(n_of(d)), *(place_at(d, bell(rng, d - t, f, tau, ratios, bright) * g, t) for t, f, g in seq))


def clinks(rng, d, times, f_low, f_high, tau=0.04):
    """Small metal pieces knocking: a short inharmonic ring each."""
    parts = []
    for t in times:
        f0 = rng.uniform(f_low, f_high)
        ring = modes(rng, 0.25, [(f0, tau, 1.0), (f0 * 2.32, tau * 0.6, 0.6), (f0 * 4.11, tau * 0.4, 0.35)], 0.04)
        tick = band(white(rng, 0.25), 3000, 12000) * env(0.25, 0.0002, 0.0008) * 0.4
        parts.append(place_at(d, (ring + tick) * rng.uniform(0.5, 1.0), t))
    return mix(np.zeros(n_of(d)), *parts)


def rustle(rng, d, swells, low=600, high=3500):
    """Leather or cloth moving: filtered noise in swells, swells = [(time, length), ...]."""
    out = np.zeros(n_of(d))
    for t, length in swells:
        x = band(colored(rng, length, 1), low, high) * np.sin(np.pi * t_axis(length) / length) ** 2
        out += place_at(d, x, t)
    creak = grains(rng, d, np.full(n_of(d), 250.0), low, high * 1.5, (0.0004, 0.002), 0.5, bands=3)
    return out + creak * 0.3 * (out != 0)


# --- armour -----------------------------------------------------------------

METALS = {
    # lowest ring, highest ring, its decay: bronze warmer and longer, iron brighter
    "bronze": (1300, 2600, 0.07),
    "iron": (2000, 4200, 0.05),
    # hard and glassy
    "diamond": (3000, 6400, 0.06),
}


def build_leather_on(rng):
    """A strap pulled tight: leather sliding then settling, with a soft pat."""
    d = 0.35
    x = rustle(rng, d, [(0.0, 0.16), (0.12, 0.18)], 500, 3000)
    pat = thump(rng, d, 160, 90, 0.025) * 0.5
    return finish(mix(x, place_at(d, pat, 0.2)), -25)


def build_leather_off(rng):
    d = 0.3
    x = rustle(rng, d, [(0.0, 0.22)], 700, 3800)
    return finish(x, -26)


def build_metal_on(rng, metal):
    """The piece slipped on: plates knocking, then settling on cloth."""
    low, high, tau = METALS[metal]
    d = 0.45
    knocks = clinks(rng, d, [0.0, 0.07 + rng.uniform(0, 0.02), 0.19 + rng.uniform(0, 0.03)], low, high, tau)
    cloth = rustle(rng, d, [(0.0, 0.2)], 600, 3000)
    return finish(outdoors(rng, knocks + cloth * 0.25, 0.08), -22)


def build_metal_off(rng, metal):
    low, high, tau = METALS[metal]
    d = 0.35
    knocks = clinks(rng, d, [0.0, 0.1 + rng.uniform(0, 0.02)], low * 0.9, high * 0.9, tau)
    cloth = rustle(rng, d, [(0.0, 0.18)], 600, 3000)
    return finish(outdoors(rng, knocks + cloth * 0.3, 0.08), -24)


def build_armour_break(rng):
    """A piece giving way: a snap, then its bits falling."""
    d = 0.6
    snap = band(white(rng, d), 1200, 9000) * env(d, 0.0003, 0.006)
    crack = modes(rng, d, [(620, 0.03, 1.0), (1540, 0.02, 0.7), (2900, 0.012, 0.5)], 0.05)
    bits = rubble(rng, d, 9, 2200, 5200, 0.05)
    return finish(outdoors(rng, snap * 0.6 + crack * 0.5 + bits * 0.6 + thump(rng, d, 200, 90, 0.03) * 0.4), -17)


# --- bow --------------------------------------------------------------------


def build_bow_draw(rng):
    """The stave bending: a stick-slip creak quickening as the string comes back, under a
    tightening hum."""
    d = 0.85
    rate = curve(d, (0, 18), (0.6, 55), (d, 70))
    phase = np.cumsum(rate) / 44100
    ticks = np.diff(np.floor(phase), prepend=0.0) * rng.uniform(0.5, 1.0, n_of(d))
    wood = modes(rng, 0.02, [(520, 0.004, 1.0), (1380, 0.002, 0.5), (2600, 0.001, 0.3)])
    creak = np.convolve(ticks, wood)[: n_of(d)]
    creak = band(creak, 300, 4000)
    hum = band(colored(rng, d, 1), 180, 900) * curve(d, (0, 0.0), (0.7, 0.4), (d, 0.3))
    shape = curve(d, (0, 0), (0.05, 0.7), (0.6, 1.0), (d, 0.6))
    return finish((creak + hum * 0.08) * shape, -26)


def build_bow_loose(rng):
    """The string let go: a low twang, the stave's knock and the arrow's hiss leaving."""
    d = 0.45
    f0 = rng.uniform(170, 190)
    string = modes(rng, d, [(f0 * k, 0.09 / k**0.5, 1 / k) for k in range(1, 8)], 0.004)
    string *= vibrato(d, 30, 0.004, rng)
    knock = place_at(d, contact_wood(rng, 0.7), 0.0)
    hiss = whoosh(rng, d, 1500, 7000, 0.01, 0.06)
    return finish(outdoors(rng, string * 0.7 + knock * 0.5 + hiss * 0.25), -18)


def build_arrow_hit_block(rng):
    """The head biting in, then the shaft quivering."""
    d = 0.5
    bite = band(white(rng, d), 800, 6000) * env(d, 0.0002, 0.004) + thump(rng, d, 300, 140, 0.02) * 0.6
    t = t_axis(d)
    quiver = np.sin(2 * np.pi * 420 * t) * (0.5 + 0.5 * np.sin(2 * np.pi * 38 * t)) * env(d, 0.003, 0.09)
    return finish(outdoors(rng, bite + band(quiver, 200, 3000) * 0.35), -20)


def build_arrow_hit_flesh(rng):
    """A dull thwack into a body."""
    d = 0.25
    body = thump(rng, d, 190, 70, 0.04) + band(colored(rng, d, 2), high=1200) * env(d, 0.0005, 0.02) * 0.8
    slap = band(white(rng, d), 600, 3500) * env(d, 0.0003, 0.006) * 0.4
    return finish(outdoors(rng, body + slap), -18)


def build_arrow_hit_player(rng):
    """The same thwack, through clothes."""
    d = 0.28
    body = thump(rng, d, 170, 65, 0.045) + band(colored(rng, d, 2), high=900) * env(d, 0.0005, 0.02) * 0.8
    cloth = band(colored(rng, d, 1), 900, 4000) * env(d, 0.002, 0.03) * 0.25
    return finish(outdoors(rng, body + cloth), -18)


def build_arrow_pickup(rng):
    """The shaft pulled out and tapped against the others in the quiver."""
    d = 0.3
    pull = band(colored(rng, d, 1), 1200, 5000) * env(d, 0.005, 0.03)
    taps = mix(np.zeros(n_of(d)), *(place_at(d, contact_wood(rng, 0.35)[: n_of(0.15)] * g, t) for t, g in ((0.07, 1.0), (0.15, 0.6))))
    return finish(pull * 0.3 + taps, -23)


# --- experience -------------------------------------------------------------


def build_orb(rng, f0):
    """A drop of light: a tiny glassy bell, quickly gone."""
    d = 0.22
    x = bell(rng, d, f0, 0.07, (1.0, 2.0, 3.01), 0.35)
    shimmer = band(white(rng, d), 6000, 12000) * env(d, 0.001, 0.01) * 0.05
    return finish(x + shimmer, -25)


def build_level_up(rng):
    """Three bells climbing a major chord, the last ringing on with its octave."""
    d = 1.1
    f = 784.0
    seq = [(0.0, f, 0.8), (0.09, f * 1.26, 0.85), (0.18, f * 1.5, 0.9), (0.18, f * 2.0, 0.35)]
    x = notes(rng, d, seq, 0.32)
    sparkle = grains(rng, d, density(d, 0.2, 0.25, 120), 5000, 11000, (0.0005, 0.002), 0.5, bands=3)
    return finish(outdoors(rng, x + sparkle * 0.04, 0.15), -19)


# --- engraving table --------------------------------------------------------


def build_engraving_open(rng):
    """Stone lid sliding aside, and a low hum waking under it."""
    d = 0.7
    grind = grains(rng, d, density(d, 0.04, 0.12, 2500), 400, 4000, (0.0003, 0.0015), 0.7)
    knock = contact_stone(rng, 0.5)[: n_of(d)]
    t = t_axis(d)
    hum = (np.sin(2 * np.pi * 110 * t) + 0.4 * np.sin(2 * np.pi * 165 * t)) * curve(d, (0, 0), (0.25, 0.5), (d, 0))
    return finish(grind * 0.4 + place_at(d, knock * 0.6, 0.12)[: n_of(d)] + hum * 0.25, -22)


def build_enchant(rng):
    """A rising shimmer of bells, and the hum that bound it fading."""
    d = 1.0
    base = 523.0
    seq = [(i * 0.07, base * r, 0.5 + 0.1 * i) for i, r in enumerate((1.0, 1.19, 1.5, 1.78, 2.0))]
    x = notes(rng, d, seq, 0.25, (1.0, 2.76, 5.4), 0.35)
    t = t_axis(d)
    hum = np.sin(2 * np.pi * 220 * t) * vibrato(d, 5, 0.3, rng) * curve(d, (0, 0.5), (d, 0))
    sparkle = grains(rng, d, density(d, 0.1, 0.3, 300), 4000, 11000, (0.0004, 0.002), 0.6, bands=3)
    return finish(outdoors(rng, x + hum * 0.08 + sparkle * 0.06, 0.15), -19)


def build_enchant_refused(rng):
    """The hum trying to wake and dying: a dull falling tone under a thud."""
    d = 0.4
    t = t_axis(d)
    f = 180 * np.exp(-t / 0.4)
    tone = np.sin(2 * np.pi * np.cumsum(f) / 44100) * env(d, 0.01, 0.12)
    body = thump(rng, d, 140, 70, 0.04)
    return finish(band(tone * 0.6 + body, high=1500), -22)


# --- anvil ------------------------------------------------------------------


def anvil_strike(rng, f0, tau):
    """A hammer on the anvil: a heavy inharmonic ring over a dull knock."""
    d = 0.9
    ring = modes(rng, d, [(f0, tau, 1.0), (f0 * 2.76, tau * 0.7, 0.7), (f0 * 5.40, tau * 0.45, 0.45), (f0 * 8.93, tau * 0.3, 0.25)], 0.01)
    tick = band(white(rng, d), 2500, 10000) * env(d, 0.0002, 0.002) * 0.5
    knock = thump(rng, d, 180, 90, 0.03) * 0.6
    return ring + tick + knock


def build_anvil_open(rng):
    """The hammer laid on the anvil: a light double tap."""
    d = 0.6
    taps = clinks(rng, d, [0.0, 0.11], 1700, 2100, 0.09)
    return finish(outdoors(rng, taps, 0.1), -24)


def build_anvil_use(rng):
    """Two strikes of the hammer, the second the louder."""
    d = 1.2
    first = anvil_strike(rng, 1150, 0.16) * 0.6
    second = anvil_strike(rng, 1180, 0.22)
    x = place_at(d, first, 0.0) + place_at(d, second, 0.22)
    return finish(outdoors(rng, x, 0.12), -19)


# --- trade ------------------------------------------------------------------


def build_trade_open(rng):
    """A satchel opened onto the counter."""
    d = 0.35
    x = rustle(rng, d, [(0.0, 0.14)], 700, 3500)
    knock = contact_wood(rng, 0.5)[: n_of(0.2)]
    return finish(mix(x, place_at(d, knock * 0.8, 0.12)), -23)


def build_trade_done(rng):
    """Coins counted out onto the counter."""
    d = 0.55
    times = np.sort(rng.uniform(0.0, 0.25, 5))
    coins = clinks(rng, d, times, 3200, 5600, 0.05)
    return finish(outdoors(rng, coins, 0.08), -21)


def build_trade_refused(rng):
    """A coin put down flat and pushed back: a dead tick and a low thud."""
    d = 0.3
    tick = modes(rng, d, [(2400, 0.008, 1.0), (5300, 0.004, 0.4)])
    body = thump(rng, d, 130, 65, 0.04)
    return finish(band(tick * 0.4 + place_at(d, body, 0.04), high=2500), -22)


# --- villagers --------------------------------------------------------------

# Pitch, vowels and breath of each trade's voice: the farmer warm, the smith gruff and low, the
# cartographer quick and high. They speak no words, a hum and a vowel or two.
VILLAGERS = {
    "farmer": (150, [NASAL, VOWEL_O, VOWEL_A], 0.08),
    "smith": (100, [VOWEL_U, VOWEL_A, VOWEL_O], 0.2),
    "cartographer": (205, [VOWEL_E, VOWEL_I, NASAL], 0.06),
}


def villager_voice(rng, villager, syllables, rise):
    """A few syllables at the voice's pitch, the last one bending up (`rise` > 1) or down."""
    f0, vowels, breath = VILLAGERS[villager]
    out = []
    for i, length in enumerate(syllables):
        last = i == len(syllables) - 1
        start = f0 * rng.uniform(0.95, 1.08)
        end = start * (rise if last else rng.uniform(0.9, 1.05))
        f = curve(length, (0, start), (length, end))
        x = glottal(rng, f, breath, 0.015)
        spec = vowels[(i + rng.integers(2)) % len(vowels)]
        x = formants(x, spec, 0.15) * env(length, 0.02, length * 0.4)
        out.append((x, length))
    d = sum(length for _, length in out) + 0.05 * len(out) + 0.1
    t, y = 0.0, np.zeros(n_of(d))
    for x, length in out:
        y += place_at(d, x, t)
        t += length + rng.uniform(0.02, 0.06)
    return y


def build_villager_cry(rng, villager, variant):
    syllables, rise = ([0.14, 0.22], 1.15) if variant == 1 else ([0.12, 0.1, 0.2], 0.85)
    return finish(outdoors(rng, villager_voice(rng, villager, syllables, rise)), -21)


def build_villager_hurt(rng, villager):
    x = villager_voice(rng, villager, [0.16], 0.8)
    return hurt_of(rng, x * 0.8, -16, VILLAGERS[villager][0])


# --- animals ----------------------------------------------------------------


def build_love(rng):
    """Hearts rising: two soft notes climbing, a bubble of air between."""
    d = 0.6
    x = notes(rng, d, [(0.0, 880, 0.7), (0.11, 1175, 0.8)], 0.18, (1.0, 2.0, 3.0), 0.25)
    return finish(outdoors(rng, x, 0.1), -24)


def build_birth(rng):
    """A soft pop, then three little notes rising."""
    d = 0.7
    pop = thump(rng, d, 380, 180, 0.03) + band(colored(rng, d, 1), 300, 2500) * env(d, 0.002, 0.02) * 0.3
    x = notes(rng, d, [(0.1, 1046, 0.5), (0.17, 1318, 0.55), (0.24, 1568, 0.6)], 0.15, (1.0, 2.0, 3.0), 0.25)
    return finish(outdoors(rng, pop * 0.6 + x, 0.1), -21)


def build_tamed(rng):
    """A warm chord, struck together, and a breath of contentment under it."""
    d = 0.9
    x = notes(rng, d, [(0.0, 659, 0.7), (0.02, 830, 0.6), (0.04, 988, 0.5)], 0.35, (1.0, 2.0, 3.0), 0.3)
    breath = band(colored(rng, d, 1), 200, 1200) * curve(d, (0, 0), (0.2, 0.3), (0.7, 0))
    return finish(outdoors(rng, x + breath * 0.15, 0.12), -20)


def build_mob_splash(rng):
    """A body flopping into water: a slap, a short spray and a few bubbles."""
    total = 0.7
    entry = band(colored(rng, total, 1), 200, 4000) * env(total, 0.003, 0.05)
    spray = band(white(rng, total), 2000, 8000) * env(total, 0.008, 0.08)
    gulp = thump(rng, total, 260, 110, 0.04)
    parts = [entry * 0.6, spray * 0.1, gulp * 0.3]
    for offset in np.sort(rng.exponential(0.1, 14)):
        if offset > total - 0.15:
            continue
        f0 = rng.uniform(300, 1100)
        tau = 0.015 * 900 / f0
        t = t_axis(tau * 8)
        bubble = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 2.5 * t)) / 44100) * env(tau * 8, 0.001, tau)
        parts.append(place_at(total, bubble * rng.uniform(0.15, 0.4), 0.02 + offset))
    return finish(outdoors(rng, mix(*parts)), -18)


# --- progress ---------------------------------------------------------------


def build_depth(rng):
    """A low, discreet sting: a deep fifth swelling out of nothing, and a far bell."""
    d = 2.0
    t = t_axis(d)
    drone = sum(g * np.sin(2 * np.pi * f * t + rng.uniform(0, 6.3)) for f, g in ((55, 1.0), (82.5, 0.6), (110, 0.3)))
    drone *= curve(d, (0, 0), (0.5, 1.0), (d, 0))
    far = place_at(d, bell(rng, d, 330, 0.5, (1.0, 2.76, 5.4), 0.3), 0.3)[: n_of(d)]
    return finish(drone + band(far, high=2000) * 0.15, -26)


def build_achievement(rng):
    """A short bright chime: two bells, a fifth apart."""
    d = 0.8
    x = notes(rng, d, [(0.0, 1046, 0.8), (0.1, 1568, 0.9)], 0.25, (1.0, 2.0, 2.99, 4.15), 0.4)
    return finish(x, -21)


def build_boss_bar(rng):
    """Something great has noticed you: a deep drum, and a low dissonant swell."""
    d = 1.6
    drum = thump(rng, d, 90, 38, 0.25) + band(colored(rng, d, 2), high=300) * env(d, 0.002, 0.1) * 0.5
    t = t_axis(d)
    saw = sum(((t * f) % 1.0) * 2 - 1 for f in (73.4, 77.8))
    swell = band(saw, high=600) * curve(d, (0, 0), (0.5, 0.5), (1.1, 0.4), (d, 0))
    return finish(outdoors(rng, drum + swell * 0.3, 0.2), -18)


def build_victory(rng):
    """The boss beaten: bells climbing to a chord that rings out."""
    d = 2.4
    f = 523.0
    seq = [(0.0, f, 0.6), (0.12, f * 1.26, 0.65), (0.24, f * 1.5, 0.7), (0.42, f * 2.0, 0.8), (0.42, f * 1.26 * 2, 0.4), (0.42, f * 1.5 * 2, 0.35)]
    x = notes(rng, d, seq, 0.6)
    t = t_axis(d)
    pad = sum(np.sin(2 * np.pi * g * t) for g in (f / 2, f * 0.75)) * curve(d, (0, 0), (0.5, 0.4), (d, 0))
    return finish(outdoors(rng, x + pad * 0.1, 0.2), -17)


def build_credits(rng):
    """A warm chord opening slowly, as the names start to rise."""
    d = 2.2
    t = t_axis(d)
    chord = sum(g * np.sin(2 * np.pi * f * t) * vibrato(d, 4.5, 0.002, rng) for f, g in ((196, 1.0), (247, 0.7), (294, 0.6), (392, 0.3)))
    chord *= curve(d, (0, 0), (0.7, 1.0), (d, 0))
    x = notes(rng, d, [(0.5, 784, 0.25)], 0.5)
    return finish(chord + x, -23)


def build_skin_import(rng):
    """A skin taken in: a soft blip and its answer a fourth higher."""
    d = 0.3
    x = notes(rng, d, [(0.0, 784, 0.7), (0.07, 1046, 0.8)], 0.08, (1.0, 2.0, 3.0), 0.25)
    return finish(x, -23)


def build_critical(rng):
    """A critical blow: a hard crack on the body, and a bright metallic ring over it."""
    d = 0.4
    crack = band(white(rng, d), 1500, 9000) * env(d, 0.0002, 0.006)
    body = thump(rng, d, 240, 80, 0.035)
    ring = modes(rng, d, [(1850, 0.12, 1.0), (1850 * 2.41, 0.07, 0.5), (1850 * 3.87, 0.04, 0.3)], 0.01)
    return finish(outdoors(rng, crack * 0.6 + body * 0.7 + ring * 0.35), -17)


def build_sweep(rng):
    """The sweep of a sword: the blade cutting the air in a wide arc, its swell rising then falling."""
    d = 0.4
    swing = whoosh(rng, d, 900, 6000, 0.09, 0.08)
    edge = band(white(rng, d), 4000, 11000) * env(d, 0.12, 0.04) * 0.3
    return finish(outdoors(rng, swing + edge), -19)


def build_shield_block(rng):
    """A blow parried by a shield: a dull knock on planks, their ring, and the iron rim's clink."""
    d = 0.45
    knock = thump(rng, d, 170, 85, 0.05)
    planks = modes(rng, d, [(240, 0.06, 1.0), (410, 0.04, 0.6), (690, 0.03, 0.4)], 0.01)
    crack = band(white(rng, d), 800, 4000) * env(d, 0.0003, 0.008)
    rim = clinks(rng, d, [0.005], 1900, 2600, 0.03)
    return finish(outdoors(rng, knock * 0.9 + planks * 0.5 + crack * 0.35 + rim * 0.2), -17)


def build_crossbow_load(rng):
    """A crossbow cranked: the ratchet clicking slower as the string tightens, the stave
    creaking under it, and the latch snapping shut."""
    d = 1.25
    times = [0.04 + 1.05 * (k / 9) ** 1.3 for k in range(9)]
    ratchet = clinks(rng, d, times, 2600, 3600, 0.012)
    rate = curve(d, (0, 20), (0.8, 50), (d, 60))
    ticks = np.diff(np.floor(np.cumsum(rate) / 44100), prepend=0.0) * rng.uniform(0.5, 1.0, n_of(d))
    wood = modes(rng, 0.02, [(480, 0.004, 1.0), (1300, 0.002, 0.5)])
    creak = band(np.convolve(ticks, wood)[: n_of(d)], 300, 3500) * curve(d, (0, 0.2), (1.0, 1.0), (d, 0.4))
    latch = clinks(rng, d, [1.15], 1500, 1900, 0.03) + place_at(d, contact_wood(rng, 0.5), 1.15) * 0.4
    return finish(ratchet * 0.5 + creak * 0.4 + latch * 0.8, -24)


def build_crossbow_shot(rng):
    """A loaded crossbow let go: the latch's click, the short heavy string's snap and the bolt's
    hiss."""
    d = 0.45
    click = clinks(rng, d, [0.0], 2200, 2800, 0.01)
    f0 = rng.uniform(120, 135)
    string = modes(rng, d, [(f0 * k, 0.06 / k**0.5, 1 / k) for k in range(1, 7)], 0.002)
    snap = band(white(rng, d), 1500, 9000) * env(d, 0.0003, 0.01)
    knock = thump(rng, d, 260, 90, 0.03)
    hiss = whoosh(rng, d, 1800, 8000, 0.008, 0.05)
    return finish(outdoors(rng, click * 0.4 + string * 0.6 + snap * 0.5 + knock * 0.6 + hiss * 0.3), -17)


# --- mounts -----------------------------------------------------------------

def build_saddle(rng):
    """A saddle thrown on a horse: the leather flapping down, its girth pulled tight and a
    buckle's clink."""
    d = 0.6
    flap = thump(rng, d, 190, 95, 0.04) * 0.6 + band(colored(rng, d, 1), 300, 2500) * env(d, 0.002, 0.05) * 0.5
    girth = rustle(rng, d, [(0.18, 0.2), (0.32, 0.16)], 500, 3200)
    buckle = clinks(rng, d, [0.46], 2300, 4200, 0.03)
    return finish(outdoors(rng, flap + girth * 0.6 + buckle * 0.35), -22)


def build_boat_paddle(rng):
    """An oar's stroke: the blade dipping in, the water pushed along it, the drops it lifts."""
    d = 0.7
    dip = band(colored(rng, d, 1), 250, 3000) * env(d, 0.004, 0.04)
    push = band(colored(rng, d, 1), 180, 1400) * env(d, 0.12, 0.14)
    parts = [dip * 0.5, push * 0.35]
    for offset in np.sort(rng.uniform(0.25, 0.55, 6)):
        f0 = rng.uniform(600, 1500)
        tau = 0.012 * 900 / f0
        t = t_axis(tau * 8)
        drop = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 2.0 * t)) / 44100) * env(tau * 8, 0.001, tau)
        parts.append(place_at(d, drop * rng.uniform(0.1, 0.25), offset))
    return finish(mix(*parts), -26)


def build_boat_hit(rng):
    """A blow on the hull: a hollow wooden knock that rings in the planks."""
    d = 0.5
    knock = thump(rng, d, 210, 120, 0.05)
    hull = modes(rng, d, [(330, 0.09, 1.0), (520, 0.06, 0.6), (870, 0.04, 0.4)], 0.02)
    crack = band(white(rng, d), 900, 5000) * env(d, 0.0005, 0.012)
    return finish(outdoors(rng, knock * 0.8 + hull * 0.5 + crack * 0.3), -19)


# --- fishing ----------------------------------------------------------------


def drops(rng, d, count, start, end, f_low=600, f_high=1500, gain=(0.1, 0.25)):
    """Drops of water falling back: short rising blips."""
    parts = []
    for offset in np.sort(rng.uniform(start, end, count)):
        f0 = rng.uniform(f_low, f_high)
        tau = 0.012 * 900 / f0
        t = t_axis(tau * 8)
        drop = np.sin(2 * np.pi * np.cumsum(f0 * (1 + 2.0 * t)) / 44100) * env(tau * 8, 0.001, tau)
        parts.append(place_at(d, drop * rng.uniform(*gain), offset))
    return mix(np.zeros(n_of(d)), *parts)


def build_fishing_cast(rng):
    """A rod cast: the rod swishing through the air and the line whistling off the reel."""
    d = 0.6
    swish = band(colored(rng, d, 1), 500, 3000) * curve(d, (0, 0), (0.06, 1), (0.22, 0))
    t = t_axis(d)
    whirr = np.sin(2 * np.pi * np.cumsum(np.interp(t, [0, d], [2600, 1900])) / 44100)
    whirr *= (0.5 + 0.5 * np.sign(np.sin(2 * np.pi * 70 * t))) * curve(d, (0, 0), (0.1, 0.6), (0.5, 0))
    return finish(outdoors(rng, swish * 0.7 + whirr * 0.12), -22)


def build_fishing_bite(rng):
    """A bite: the float pulled under with a plop, and the bubbles that rise after it."""
    d = 0.7
    plop = thump(rng, d, 420, 160, 0.035) * 0.7
    slap = band(colored(rng, d, 1), 400, 3500) * env(d, 0.002, 0.03) * 0.4
    bubbles = drops(rng, d, 9, 0.05, 0.45, 350, 1100, (0.15, 0.35))
    return finish(outdoors(rng, plop + slap + bubbles), -18)


def build_fishing_reel(rng):
    """The line drawn in: the reel clicking round fast, and a drip of water off the float."""
    d = 0.6
    times = np.arange(0.0, 0.32, 0.028) + rng.uniform(0, 0.004, 12)
    ratchet = clinks(rng, d, list(times), 2600, 3600, 0.012)
    water = drops(rng, d, 4, 0.3, 0.5)
    return finish(outdoors(rng, ratchet * 0.6 + water), -22)


def main():
    write_ogg("armour/leather_on", build_leather_on)
    write_ogg("armour/leather_off", build_leather_off)
    for metal in METALS:
        write_ogg(f"armour/{metal}_on", build_metal_on, metal)
        write_ogg(f"armour/{metal}_off", build_metal_off, metal)
    write_ogg("armour/break", build_armour_break)
    write_ogg("bow/draw", build_bow_draw)
    write_ogg("bow/loose", build_bow_loose)
    write_ogg("bow/hit_block", build_arrow_hit_block)
    write_ogg("bow/hit_creature", build_arrow_hit_flesh)
    write_ogg("bow/hit_player", build_arrow_hit_player)
    write_ogg("bow/pickup", build_arrow_pickup)
    for i, f0 in enumerate((1760, 1975, 2217), start=1):
        write_ogg(f"player/orb{i}", build_orb, f0)
    write_ogg("player/level_up", build_level_up)
    write_ogg("ui/engraving_open", build_engraving_open)
    write_ogg("ui/enchant", build_enchant)
    write_ogg("ui/enchant_refused", build_enchant_refused)
    write_ogg("ui/anvil_open", build_anvil_open)
    write_ogg("ui/anvil_use", build_anvil_use)
    write_ogg("ui/trade_open", build_trade_open)
    write_ogg("ui/trade_done", build_trade_done)
    write_ogg("ui/trade_refused", build_trade_refused)
    for villager in VILLAGERS:
        for i in (1, 2):
            write_ogg(f"mob/{villager}/cry{i}", build_villager_cry, villager, i)
        write_ogg(f"mob/{villager}/hurt", build_villager_hurt, villager)
    write_ogg("mob/love", build_love)
    write_ogg("mob/birth", build_birth)
    write_ogg("mob/tamed", build_tamed)
    write_ogg("liquid/mob_splash", build_mob_splash)
    write_ogg("ui/depth", build_depth)
    write_ogg("ui/achievement", build_achievement)
    write_ogg("ui/boss_bar", build_boss_bar)
    write_ogg("ui/victory", build_victory)
    write_ogg("ui/credits", build_credits)
    write_ogg("ui/skin_import", build_skin_import)
    write_ogg("combat/critical", build_critical)
    write_ogg("combat/sweep", build_sweep)
    write_ogg("mob/saddle", build_saddle)
    write_ogg("boat/paddle", build_boat_paddle)
    write_ogg("boat/hit", build_boat_hit)
    write_ogg("combat/shield_block", build_shield_block)
    write_ogg("combat/crossbow_load", build_crossbow_load)
    write_ogg("combat/crossbow_shot", build_crossbow_shot)
    write_ogg("fishing/cast", build_fishing_cast)
    write_ogg("fishing/bite", build_fishing_bite)
    write_ogg("fishing/reel", build_fishing_reel)


if __name__ == "__main__":
    main()
