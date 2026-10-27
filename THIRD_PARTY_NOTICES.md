# Third-party notices

Petal's original code and artwork are licensed under MIT; third-party components retain their own licenses.

## Completion sound

The embedded Paid sound is adapted from [Bencho Sounds](https://github.com/cabralorenzo/bencho/blob/main/sounds/sounds.ts), copyright (c) 2026 Lorenzo Cabra, under MIT. The full notice is in [assets/BENCHO-LICENSE.txt](assets/BENCHO-LICENSE.txt). The local PCM rendering script is [tools/render_paid.py](tools/render_paid.py).

## Media tools

FFmpeg and FFprobe are separate, unmodified executables downloaded automatically on first audio/video conversion. Source builds can also prepare them with Install-MediaEngine.ps1. The pinned Gyan essentials build is GPL v3; its license, source references and archive checksum are in [ffmpeg/](ffmpeg/). Media binaries are not committed to this repository or included in the core CI artifact. They are downloaded directly from the provider with the pinned archive checksum, and their license is retained in the user data folder.

## Rust dependencies

Cargo.toml and Cargo.lock identify the direct and transitive Rust dependencies. Their respective licenses apply; this project's MIT license does not replace them. Complete Rust dependency and standard-library notices are embedded in assets/third-party-licenses.txt.gz and can be opened from Settings → Licenses.

## Windows fonts and rendering

Segoe UI Bold is used from the Windows installation. Text PDF export embeds the installed Tahoma font only when its embedding permissions allow it. No Windows font files are distributed with the project. Loading-animation inspiration is credited in [assets/SOURCES.md](assets/SOURCES.md).
