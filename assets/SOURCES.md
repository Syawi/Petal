# Embedded assets

## Completion sound
paid.wav is a PCM rendering of the Paid notes, envelopes and filters adapted from the MIT-licensed Bencho source:
https://github.com/cabralorenzo/bencho/blob/main/sounds/sounds.ts

Copyright (c) 2026 Lorenzo Cabra. License: BENCHO-LICENSE.txt.
Reference: https://bencho.dev/sounds?s=paid

Recreate locally with python tools/render_paid.py. Mono, 44100 Hz, 16-bit PCM, 0.6 seconds. Embedded in the executable; no runtime download.

## Type
The interface uses Windows system Segoe UI Bold. Text PDF export uses Windows Tahoma with Unicode mapping. No Windows font file is redistributed.

## Loading-animation reference
The spinner is a native Rust implementation inspired by Watch Spinner, by Amicro / Syed Subhan: https://amicro.dev/loaders/watch-spinner . No React runtime or website code is bundled.

## Application mark
Approved blue three-petal ribbon concept 1A. Vector master: petal.svg. petal.ico contains nine transparent PNG sizes. petal-icon.res and petal-icon.o embed those exact images for MSVC and GNU builds. Resource source: petal.rc. petal.png provides the explicit notification image. No external logo font or reference image is needed at runtime.
