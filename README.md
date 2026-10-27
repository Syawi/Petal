<p align="center"><strong>English</strong> · <a href="docs/i18n/README.zh-CN.md">简体中文</a> · <a href="docs/i18n/README.es.md">Español</a> · <a href="docs/i18n/README.ar.md">العربية</a> · <a href="docs/i18n/README.hi.md">हिन्दी</a> · <a href="docs/i18n/README.fr.md">Français</a></p>

<p align="center">
  <img src="docs/media/hero.png" alt="Petal — a native Windows radial menu for file conversion" width="100%">
</p>

<p align="center">
  <a href="https://github.com/Syawi/Petal/actions/workflows/windows.yml"><img src="https://github.com/Syawi/Petal/actions/workflows/windows.yml/badge.svg" alt="Windows build"></a>
  <img src="https://img.shields.io/badge/Windows-11-0078D4?style=flat" alt="Windows 11">
  <img src="https://img.shields.io/badge/Rust-1.89%2B-DEA584?style=flat" alt="Rust 1.89 or newer">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-79C99E?style=flat" alt="MIT license"></a>
</p>

<p align="center"><strong>Right-click a file. Pick a format. Done.</strong></p>

Petal is a native Windows 11 utility that puts image, audio, video and text-document conversion in a compact radial menu. It works in File Explorer and on the desktop, follows your Windows theme and accent color, and keeps the original Windows context menu one click away.

## See it in motion

![Petal's animated radial menu, hover movement, loading spinner and completion check](docs/media/demo.gif)

<sub>Preview generated with Petal's native renderer using a sample filename. [How the visuals are made](docs/MEDIA.md).</sub>

## What Petal does

- **Six relevant choices.** Formats adapt to the selected file; the current format is replaced with an alternative.
- **Images, media and documents.** Convert to WebP, extract a soundtrack, turn text into a searchable PDF, and more.
- **Windows styling.** Light/dark themes, full accent-color hover, moving petals and crisp Segoe UI Bold labels.
- **Clear feedback.** A spinner, a completion check, a branded Windows notification and an optional sound.
- **Native and local.** Rust with Win32/GDI+, no browser runtime, no file uploads; the audio/video engine requires a one-time download.

> **Your original file stays unchanged.** Petal verifies the converted file and saves it alongside the original, using the same name with the new extension (for example, `photo.jpg` → `photo.webp`). Existing destinations are never overwritten. Metadata and layout in the converted copy may differ.

## Formats

| Selected file | Default choices | Alternative for the current format |
| --- | --- | --- |
| Image | JPG · PNG · WebP · GIF · BMP · PDF | TIFF |
| Audio | MP3 · WAV · FLAC · M4A · OGG · MP4 | OPUS |
| Video | MP4 · MKV · WebM · MOV · MP3 · WAV | AVI |
| Text document | TXT · DOCX · MD · HTML · RTF · PDF | ODT |

For example, a JPG offers TIFF in its place. Audio → MP4 creates a video with a black background and the original sound; video → MP3 extracts its soundtrack. Document conversions preserve text and paragraphs rather than the original layout. Scanned PDFs require OCR and are refused.

Read [FORMATS.md](FORMATS.md) for supported inputs, codecs, limits and preservation rules.

## Get started

**Requirements:** Windows 11 x64. The interface is in English.

### Build from source

Install [Rust](https://rustup.rs/) and the Visual Studio C++ build tools/Windows SDK for the MSVC toolchain. Then:

```powershell
git clone https://github.com/Syawi/Petal.git
cd Petal
./Build.ps1
./Petal.exe
```

The build script prepares the pinned FFmpeg/FFprobe tools, runs tests and creates Petal.exe. The one-time media download is SHA-256 verified. GNU builds also need MinGW-w64 binutils; see [CONTRIBUTING.md](CONTRIBUTING.md).

### Download v1.2

Download **[Petal.exe](https://github.com/Syawi/Petal/releases/download/v1.2/Petal.exe)** and run it directly. Windows 11 x64 is required. No ZIP, adjacent assets folder, administrator access or Office installation is needed.

The logo, completion sound and license notices are embedded. Petal stores its small runtime files under `%LOCALAPPDATA%/Petal`. On the first audio/video conversion, it automatically downloads and verifies the pinned FFmpeg engine (about 109 MB); later conversions work offline. Images and documents work without this download.

Launch at sign-in is enabled on first run and can be disabled in **Settings**. Keep the executable in a stable location for startup. Open **Settings → Licenses** to read the included notices.

### Download a CI build

A successful [Windows workflow run](https://github.com/Syawi/Petal/actions/workflows/windows.yml) also provides **Petal-Windows-x64**. Sign in to GitHub, download the artifact and extract `Petal.exe`.

## Using the wheel

1. Launch Petal.exe; its blue icon appears in the system tray, possibly under hidden icons.
2. Hold **Shift** and right-click one supported file in Explorer or on the desktop.
3. Click a format. Petal writes and checks the converted copy, then displays **Conversion complete**.

The center opens the **Windows menu**. A normal **right-click** opens the Windows menu. The default **Shift + right-click** binding is customizable. Click outside or press **Esc** to dismiss the wheel; an active conversion continues in the background.

| Shortcut | Action |
| --- | --- |
| `1`–`6` | Convert using the corresponding petal |
| `Tab`, then `Enter` / `Space` | Select and activate an option |
| `M` | Open the Windows context menu |
| `Esc` | Hide the wheel or feedback |

Open **Settings…** from the tray icon to capture any keyboard key or mouse button with optional Ctrl/Shift/Alt/Win modifiers, toggle launch at sign-in, or choose and preview a custom WAV completion sound. Settings save automatically. **Reset** restores Shift + right-click. The tray also offers **Mute sound** and **Quit Petal**; quitting is unavailable during conversion.

## Fits your desktop

![Petal in Windows light and dark themes](docs/media/themes.png)

The app follows your Windows accent color and reduced-motion preference. Hover fills the selected petal or center with the accent color and turns its text black. Reduced motion disables movement and uses static feedback.

## Development

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

Windows CI checks formatting, runs the tests and builds the portable core. Interactive desktop checks and the README-frame exporter are opt-in. See [TESTING.md](TESTING.md), [CONTRIBUTING.md](CONTRIBUTING.md) and [docs/MEDIA.md](docs/MEDIA.md).

```text
src/                 Native UI, conversions, notifications and sound
assets/              App mark, icon resources and completion sound
tools/               Reproducible sound and README-media generators
docs/media/          README images and animated preview
ffmpeg/              Media-engine notices; executables are downloaded locally
.github/workflows/   Windows build and test pipeline
```

## License and credits

Petal is [MIT licensed](LICENSE), copyright © 2026 Syawi. Third-party tools and assets retain their own licenses: [third-party notices](THIRD_PARTY_NOTICES.md), [asset sources](assets/SOURCES.md) and [media-engine sources](ffmpeg/SOURCES.md).
