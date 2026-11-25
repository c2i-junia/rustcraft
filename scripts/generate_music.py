#!/usr/bin/env python3
"""Procedurally composes the game's music into data/music/.

Every piece is written and played here, from a fixed seed: the output is reproducible and
entirely original (MIT, like the code). A piece is a chord progression in a key, looped for
two to three minutes, played by a few soft instruments, all additive synthesis: a pad that
holds the chords, a bass on their roots, an arpeggio of plucked notes, bells, and a melody
made of motifs that come back varied, as a tune does. The instruments come in one after the
other and leave in the reverse order, so a piece rises and fades, and the whole goes through
a long stereo reverb. The moods differ by their key, mode, tempo, instruments and density:
the day bright and moving, the night slow and sparse, the underground low drones and drips,
the boss a tense pulse.

Requirements: numpy, scipy, and ffmpeg built with libvorbis.
Usage: python3 scripts/generate_music.py [output_dir] [--only NAME]
"""

import subprocess
import sys
import tempfile
import zlib
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, fftconvolve, sosfilt

SR = 44100
ROOT = Path(__file__).resolve().parent.parent
ONLY = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else ""
ARGS = [arg for arg in sys.argv[1:] if arg not in ("--only", ONLY)]
OUT = Path(ARGS[0]) if ARGS else ROOT / "data" / "music"
SEED = 1000

MAJOR = [0, 2, 4, 5, 7, 9, 11]
MINOR = [0, 2, 3, 5, 7, 8, 10]
DORIAN = [0, 2, 3, 5, 7, 9, 10]
LYDIAN = [0, 2, 4, 6, 7, 9, 11]
PHRYGIAN = [0, 1, 3, 5, 7, 8, 10]
HARMONIC_MINOR = [0, 2, 3, 5, 7, 8, 11]


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


def n_of(seconds):
    return int(seconds * SR)


def t_axis(seconds):
    return np.arange(n_of(seconds)) / SR


def lowpass(x, cutoff, order=2):
    return sosfilt(butter(order, cutoff, "lowpass", fs=SR, output="sos"), x, axis=-1)


def highpass(x, cutoff, order=2):
    return sosfilt(butter(order, cutoff, "highpass", fs=SR, output="sos"), x, axis=-1)


def adsr(length, attack, release, sustain_tau=None):
    """Rises over `attack`, holds (or decays with `sustain_tau`), and falls over `release` at
    the end of `length` seconds."""
    t = t_axis(length)
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    a = 0.5 - 0.5 * np.cos(np.pi * a)
    r = np.clip((length - t) / max(release, 1e-4), 0, 1)
    hold = np.exp(-t / sustain_tau) if sustain_tau else 1.0
    return a * r * hold


# --- instruments ------------------------------------------------------------
# Each returns a stereo note, shape (2, samples).


def pan(mono, position):
    """`position` from -1 (left) to 1 (right), equal power."""
    angle = (position + 1) * np.pi / 4
    return np.stack([mono * np.cos(angle), mono * np.sin(angle)])


def additive(freq, length, partials, rng, detune=0.0):
    """A sum of harmonics, partials = [(ratio, amp, decay_tau or None), ...]."""
    t = t_axis(length)
    out = np.zeros_like(t)
    f = freq * (1 + detune)
    for ratio, amp, tau in partials:
        if f * ratio >= SR / 2.2:
            continue
        partial = amp * np.sin(2 * np.pi * f * ratio * t + rng.uniform(0, 2 * np.pi))
        if tau:
            partial *= np.exp(-t / tau)
        out += partial
    return out


def pad(freq, length, rng, brightness=1.0):
    """A warm held chord tone: two detuned voices spread left and right, slow to rise."""
    partials = [(k, 1 / k ** (2.2 - 0.6 * brightness), None) for k in range(1, 7)]
    env = adsr(length, min(1.8, length / 3), min(1.8, length / 3))
    t = t_axis(length)
    swell = 1 + 0.15 * np.sin(2 * np.pi * 0.2 * t + rng.uniform(0, 6))
    left = additive(freq, length, partials, rng, -0.003) * env * swell
    right = additive(freq, length, partials, rng, 0.003) * env * swell
    return np.stack([left, right]) * 0.5


def bass(freq, length, rng):
    partials = [(1, 1.0, None), (2, 0.25, 0.4), (3, 0.08, 0.2)]
    return pan(additive(freq, length, partials, rng) * adsr(length, 0.03, 0.3), 0) * 0.9


def pluck(freq, length, rng, position=0.0):
    """A plucked string: bright at once, its high harmonics dying first."""
    partials = [(k, 1 / k**1.6, 0.9 / k**0.8) for k in range(1, 8)]
    x = additive(freq, length, partials, rng) * adsr(length, 0.004, 0.15)
    return pan(x, position) * 0.5


def bell(freq, length, rng, position=0.0):
    """A soft bell, its partials inharmonic, ringing long."""
    partials = [(1, 1.0, 2.0), (2.01, 0.35, 1.2), (2.76, 0.25, 0.8), (5.4, 0.08, 0.3)]
    x = additive(freq, length, partials, rng) * adsr(length, 0.003, 0.5)
    return pan(x, position) * 0.35


def lead(freq, length, rng, position=0.0):
    """The melody: a round flute-like tone with breath, its vibrato coming in late."""
    t = t_axis(length)
    vib = 1 + 0.006 * np.sin(2 * np.pi * 5.0 * t) * np.clip((t - 0.25) / 0.4, 0, 1)
    phase = 2 * np.pi * freq * np.cumsum(vib) / SR
    x = np.sin(phase) + 0.22 * np.sin(2 * phase) + 0.06 * np.sin(3 * phase)
    breath = lowpass(highpass(rng.standard_normal(len(t)), 800), 3000) * 0.02
    x = (x + breath) * adsr(length, 0.07, min(0.35, length / 2))
    return pan(x, position) * 0.55


def drum(length, rng, f0=70):
    """A low tom, felt more than heard."""
    t = t_axis(length)
    f = f0 * (1 + 1.5 * np.exp(-t / 0.03))
    x = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.25)
    x += lowpass(rng.standard_normal(len(t)), 500) * np.exp(-t / 0.02) * 0.3
    return pan(x * adsr(length, 0.002, 0.05), 0) * 0.9


# --- composition ------------------------------------------------------------


@dataclass
class Piece:
    name: str
    root: int  # MIDI note of the key's tonic, around the melody's octave
    scale: list
    progression: list  # scale degrees (0-based) of the chords' roots
    bpm: float
    seconds: float
    beats_per_chord: int = 4
    arp: str = "up"  # "up", "updown", "broken", or "" for none
    arp_step: float = 0.5  # beats between notes of the arpeggio
    melody: float = 1.0  # how busy the melody is, 0 for none
    bells: float = 0.0  # bell notes per bar
    drums: bool = False
    drone: bool = False  # the tonic held under everything
    brightness: float = 1.0
    reverb: float = 0.3
    levels: dict = field(default_factory=dict)


def chord(piece, degree, size=3):
    """The notes (MIDI) of the triad, or seventh, built on `degree` of the piece's scale."""
    notes = []
    for i in range(size):
        step = degree + 2 * i
        octave, index = divmod(step, len(piece.scale))
        notes.append(piece.root + piece.scale[index] + 12 * octave)
    return notes


def scale_note(piece, step):
    octave, index = divmod(step, len(piece.scale))
    return piece.root + piece.scale[index] + 12 * octave


RHYTHMS = [
    [1, 1, 2],
    [2, 1, 1],
    [1.5, 0.5, 2],
    [1, 0.5, 0.5, 2],
    [3, 1],
    [2, 2],
    [0.5, 0.5, 1, 2],
    [4],
]


def motif(piece, rng, degree, beats):
    """A melodic idea over one chord: (start beat, length in beats, scale step or None for a
    rest). It starts on a tone of the chord and walks the scale by small steps."""
    notes, at = [], 0.0
    rhythm = list(RHYTHMS[rng.integers(len(RHYTHMS))])
    while sum(rhythm) < beats:
        rhythm += RHYTHMS[rng.integers(len(RHYTHMS))]
    step = degree + 2 * rng.integers(0, 3) + 7
    for length in rhythm:
        if at >= beats:
            break
        length = min(length, beats - at)
        rest = rng.random() < 0.12 and at > 0
        notes.append((at, length, None if rest else step))
        step += int(rng.choice([-2, -1, -1, 1, 1, 2, 0, 3, -3]))
        step = int(np.clip(step, 4, 14))
        at += length
    return notes


def vary(piece, rng, notes):
    """The same idea, a little changed: one note moved, or the end turned."""
    notes = list(notes)
    pitched = [i for i, (_, _, s) in enumerate(notes) if s is not None]
    if pitched:
        i = pitched[rng.integers(len(pitched))]
        at, length, step = notes[i]
        notes[i] = (at, length, int(np.clip(step + rng.choice([-1, 1, 2]), 4, 14)))
    return notes


def compose(piece, rng):
    """The whole piece, stereo, `piece.seconds` long plus its reverb's tail."""
    beat = 60.0 / piece.bpm
    chord_seconds = beat * piece.beats_per_chord
    count = int(piece.seconds / chord_seconds)
    total = count * chord_seconds
    out = np.zeros((2, n_of(total + 6)))
    levels = {"pad": 0.35, "bass": 0.35, "arp": 0.22, "lead": 0.3, "bells": 0.18, "drums": 0.4}
    levels.update(piece.levels)

    def put(note, at, level):
        start = n_of(at)
        end = min(out.shape[1], start + note.shape[1])
        out[:, start:end] += note[:, : end - start] * level

    # The instruments come in and leave in order: the pad first and last, the melody in the
    # middle. Fractions of the piece.
    enters = {"pad": 0.0, "bass": 0.06, "arp": 0.12, "bells": 0.2, "lead": 0.3, "drums": 0.1}
    leaves = {"pad": 1.0, "bass": 0.94, "arp": 0.9, "bells": 0.85, "lead": 0.8, "drums": 0.92}

    def playing(part, i):
        return enters[part] <= i / count < leaves[part]

    # Melody: a phrase of four chords, A A' B A', made again for each pass
    phrase_len = len(piece.progression)
    ideas = {}
    for i in range(count):
        degree = piece.progression[i % phrase_len]
        start = i * chord_seconds
        notes = chord(piece, degree, 4 if piece.brightness < 0.8 else 3)
        if playing("pad", i):
            for n in notes:
                put(pad(hz(n - 12), chord_seconds + 1.5, rng, piece.brightness), start, levels["pad"] / len(notes))
        if piece.drone:
            put(pad(hz(piece.root - 24), chord_seconds + 1.5, rng, 0.4), start, levels["pad"] * 0.5)
        if playing("bass", i):
            if piece.drums:
                for b in range(piece.beats_per_chord * 2):
                    put(bass(hz(notes[0] - 24), beat * 0.45, rng), start + b * beat / 2, levels["bass"] * (1.0 if b % 2 == 0 else 0.6))
            else:
                put(bass(hz(notes[0] - 24), chord_seconds, rng), start, levels["bass"])
        if piece.arp and playing("arp", i):
            pattern = notes + [notes[0] + 12]
            if piece.arp == "updown":
                pattern = pattern + pattern[-2:0:-1]
            elif piece.arp == "broken":
                pattern = [pattern[0], pattern[2], pattern[1], pattern[3]]
            steps = int(piece.beats_per_chord / piece.arp_step)
            for s in range(steps):
                n = pattern[s % len(pattern)]
                position = -0.4 + 0.8 * (s % len(pattern)) / max(len(pattern) - 1, 1)
                put(pluck(hz(n), 1.6, rng, position), start + s * piece.arp_step * beat, levels["arp"])
        if piece.bells and playing("bells", i):
            for _ in range(rng.poisson(piece.bells)):
                n = scale_note(piece, int(rng.integers(7, 16)))
                at = start + rng.integers(0, piece.beats_per_chord * 2) * beat / 2
                put(bell(hz(n + 12), 3.0, rng, rng.uniform(-0.7, 0.7)), at, levels["bells"])
        if piece.drums and playing("drums", i):
            for b in range(piece.beats_per_chord):
                put(drum(beat, rng, 60 if b % 2 == 0 else 80), start + b * beat, levels["drums"] * (1.0 if b == 0 else 0.55))
        if piece.melody and playing("lead", i):
            slot = i % phrase_len
            section = (i // phrase_len) % 4  # A A' B A'
            key = (slot, "B" if section == 2 else "A")
            if key not in ideas:
                ideas[key] = motif(piece, rng, degree, piece.beats_per_chord)
            line = ideas[key] if section != 3 else vary(piece, rng, ideas[key])
            if rng.random() > piece.melody:
                continue
            for at, length, step in line:
                if step is None:
                    continue
                put(lead(hz(scale_note(piece, step)), length * beat * 1.05 + 0.15, rng, 0.1), start + at * beat, levels["lead"])

    # A long fade in and out, then the hall
    t = np.arange(out.shape[1]) / SR
    out *= np.clip(t / 6.0, 0, 1) * np.clip((total + 3 - t) / 10.0, 0, 1)
    return reverb(out, rng, piece.reverb)


def reverb(x, rng, wet):
    """A hall: decorrelated decaying noise for each ear, darker as it dies."""
    length = 4.0
    t = t_axis(length)
    irs = []
    for _ in range(2):
        noise = rng.standard_normal(len(t))
        early = lowpass(noise, 6000) * np.exp(-t / 0.25)
        late = lowpass(noise, 1800) * np.exp(-t / 0.9)
        ir = (early * 0.4 + late) * np.clip(t / 0.02, 0, 1)
        irs.append(ir / np.sqrt(np.sum(ir**2)))
    tail = np.stack([fftconvolve(x[0], irs[0])[: x.shape[1]], fftconvolve(x[1], irs[1])[: x.shape[1]]])
    return x * (1 - wet * 0.5) + tail * wet * 2.0


def master(x, lufs=-20.0):
    """Cleans the lows, sets an even loudness across pieces, keeps the peaks below -1 dBFS,
    and trims the silence at the end."""
    x = highpass(x, 35)
    loud = x[:, np.abs(x).max(axis=0) > 1e-3]
    level = 20 * np.log10(np.sqrt(np.mean(loud**2)) + 1e-12)
    x = x * 10 ** ((lufs - level) / 20)
    peak = np.abs(x).max()
    if peak > 10 ** (-1 / 20):
        x = np.tanh(x / peak * 1.2) / np.tanh(1.2) * 10 ** (-1 / 20)
    audible = np.nonzero(np.abs(x).max(axis=0) > 10 ** (-70 / 20))[0]
    return x[:, : audible[-1] + 1]


PIECES = [
    # The menus: warm and simple
    Piece("title", 60, MAJOR, [0, 5, 3, 4], 72, 150, arp="updown", melody=0.9, bells=0.3, reverb=0.35),
    # Day: bright, moving
    Piece("day1", 67, LYDIAN, [0, 1, 0, 4], 84, 165, arp="up", arp_step=0.5, melody=0.85, brightness=1.2),
    Piece("day2", 62, MAJOR, [0, 3, 5, 4], 76, 160, arp="broken", melody=0.8, bells=0.4, levels={"arp": 0.18}),
    # Night: slow and sparse, minor
    Piece("night1", 57, MINOR, [0, 5, 2, 6], 60, 170, arp="up", arp_step=1.0, melody=0.6, bells=0.8, brightness=0.6, reverb=0.45),
    Piece("night2", 64, DORIAN, [0, 3, 0, 6], 56, 160, arp="", melody=0.7, bells=1.0, brightness=0.6, reverb=0.5, levels={"pad": 0.45}),
    # Underground: low drones, drips of bells, hardly a tune
    Piece("underground1", 50, PHRYGIAN, [0, 1, 0, 6], 50, 170, beats_per_chord=8, arp="", melody=0.25, bells=1.4, drone=True, brightness=0.4, reverb=0.55, levels={"bells": 0.22, "lead": 0.22}),
    Piece("underground2", 54, MINOR, [0, 5, 3, 4], 54, 160, beats_per_chord=8, arp="broken", arp_step=2.0, melody=0.3, bells=1.0, drone=True, brightness=0.5, reverb=0.5, levels={"arp": 0.16}),
    # The boss: a tense pulse
    Piece("boss", 60, HARMONIC_MINOR, [0, 1, 0, 4], 112, 150, arp="up", arp_step=0.25, melody=0.7, drums=True, brightness=0.9, reverb=0.25, levels={"arp": 0.15, "lead": 0.26}),
]


def write_ogg(piece):
    if not piece.name.startswith(ONLY):
        return
    rng = np.random.default_rng([SEED, zlib.crc32(piece.name.encode())])
    x = master(compose(piece, rng))
    path = OUT / f"{piece.name}.ogg"
    path.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(suffix=".wav") as tmp:
        wavfile.write(tmp.name, SR, (np.clip(x.T, -1, 1) * 32767).astype(np.int16))
        subprocess.run(
            ["ffmpeg", "-y", "-loglevel", "error", "-i", tmp.name, "-c:a", "libvorbis", "-q:a", "0", str(path)],
            check=True,
        )
    print(f"{path.relative_to(OUT)}: {x.shape[1] / SR:.0f} s, {path.stat().st_size / 1e6:.1f} MB")


def main():
    for piece in PIECES:
        write_ogg(piece)


if __name__ == "__main__":
    main()
