"""Render Bencho's MIT-licensed Paid notes to PCM, without runtime dependencies.

Adapted from https://github.com/cabralorenzo/bencho/blob/main/sounds/sounds.ts
Copyright (c) 2026 Lorenzo Cabra. See assets/BENCHO-LICENSE.txt.
"""
import math
import struct
import wave
from pathlib import Path


def render():
    rate = 44100
    samples = [0.0] * int(rate * 0.6)
    # hz, duration, gain, lowpass, attack, delay (milliseconds).
    notes = [(587, 170, .05, 2600, 5, 0),
             (880, 460, .046, 2800, 6, 95),
             (1760, 300, .009, None, 8, 100)]
    for hz, ms, gain, cutoff, attack, delay in notes:
        if cutoff:
            omega = 2 * math.pi * cutoff / rate
            cosine = math.cos(omega)
            # Web Audio lowpass Q defaults to 1 dB.
            alpha = math.sin(omega) / (2 * 10 ** (1 / 20))
            a0 = 1 + alpha
            b0 = (1 - cosine) / 2 / a0
            b1 = (1 - cosine) / a0
            b2 = b0
            a1 = -2 * cosine / a0
            a2 = (1 - alpha) / a0
        x1 = x2 = y1 = y2 = 0.0
        for index in range(math.ceil((ms + 30) * rate / 1000)):
            t = index / rate * 1000
            value = math.sin(2 * math.pi * hz * index / rate) if t < ms + 20 else 0.0
            if cutoff:
                filtered = b0 * value + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2
                x2, x1, y2, y1 = x1, value, y1, filtered
                value = filtered
            if t < attack:
                envelope = .0001 * (gain / .0001) ** (t / attack)
            elif t < ms:
                envelope = gain * (.0001 / gain) ** ((t - attack) / (ms - attack))
            else:
                envelope = .0001
            offset = round(delay * rate / 1000) + index
            if offset < len(samples):
                samples[offset] += value * envelope
    assert .03 < max(abs(x) for x in samples) < .12
    path = Path(__file__).resolve().parents[1] / "assets" / "paid.wav"
    with wave.open(str(path), "wb") as output:
        output.setparams((1, 2, rate, len(samples), "NONE", "not compressed"))
        output.writeframes(b"".join(struct.pack("<h", round(x * 32767)) for x in samples))
    print(f"{path.name}: {len(samples) / rate:.3f} s, mono PCM 16-bit, {rate} Hz")


if __name__ == "__main__":
    render()
