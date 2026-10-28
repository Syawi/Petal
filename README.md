<p align="center"><strong>English</strong> · <a href="docs/i18n/README.zh-CN.md">简体中文</a> · <a href="docs/i18n/README.es.md">Español</a> · <a href="docs/i18n/README.ar.md">العربية</a> · <a href="docs/i18n/README.hi.md">हिन्दी</a> · <a href="docs/i18n/README.fr.md">Français</a></p>

<p align="center">
  <img src="docs/media/hero.png" alt="Petal — a liquid glass radial menu for file conversion on Windows" width="100%">
</p>

<p align="center">
  <a href="https://github.com/Syawi/Petal/actions/workflows/windows.yml"><img src="https://github.com/Syawi/Petal/actions/workflows/windows.yml/badge.svg" alt="Windows build"></a>
  <a href="https://github.com/Syawi/Petal/releases/latest"><img src="https://img.shields.io/github/v/release/Syawi/Petal?style=flat" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/Windows-11-0078D4?style=flat" alt="Windows 11">
  <img src="https://img.shields.io/badge/Rust-1.89%2B-DEA584?style=flat" alt="Rust 1.89 or newer">
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-79C99E?style=flat" alt="MIT license"></a>
</p>

<p align="center"><strong>Shift + right-click a file. Pick a format. Done.</strong></p>

Petal is a native Windows 11 utility that puts image, audio, video and text-document conversion in a compact radial menu, right in File Explorer and on the desktop. Its default **liquid glass** theme frosts and bends whatever is behind it; a **classic** theme follows your Windows accent colour instead. The center button always opens the original Windows context menu.

## See it in motion

![Petal's liquid glass wheel bubbling open, a petal lighting up on hover, then the conversion spinner and check](docs/media/demo.gif)

<sub>Rendered by Petal's native engine over a synthetic wallpaper. [How the visuals are made](docs/MEDIA.md).</sub>

## What Petal does

- **Six relevant choices.** Formats adapt to the selected file; its current format is replaced with an alternative.
- **Images, media and documents.** Convert to WebP, extract a soundtrack, turn text into a searchable PDF, and more.
- **Liquid glass.** Petals and the center are frosted glass that refracts the desktop near every edge. Hover lights the glass up instead of painting a colour over it.
- **Bubble animation.** The center pops first, then the petals burst out from under it with a soft overshoot, all in under half a second.
- **Clear feedback.** A spinner, a completion check, a branded Windows notification and an optional sound.
- **Native and local.** Rust with Win32/GDI+, no browser runtime and no uploads. The audio/video engine is a one-time, verified download.

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

### Download v1.2

Download **[Petal.exe](https://github.com/Syawi/Petal/releases/download/v1.2/Petal.exe)** and run it directly. No ZIP, adjacent assets folder, administrator access or Office installation is needed.

The logo, completion sound and license notices are embedded. Petal stores its small runtime files under `%LOCALAPPDATA%/Petal`. On the first audio/video conversion it downloads and verifies the pinned FFmpeg engine (about 109 MB); later conversions work offline. Images and documents never need it.

Launch at sign-in is enabled on first run and can be turned off in **Settings**. Keep the executable in a stable location so startup keeps working.

### Build from source

Install [Rust](https://rustup.rs/) and the Visual Studio C++ build tools/Windows SDK for the MSVC toolchain. Then:

```powershell
git clone https://github.com/Syawi/Petal.git
cd Petal
./Build.ps1
./Petal.exe
```

The build script prepares the pinned FFmpeg/FFprobe test tools, runs the tests and creates Petal.exe. GNU builds also need MinGW-w64 binutils; see [CONTRIBUTING.md](CONTRIBUTING.md).

### Download a CI build

A successful [Windows workflow run](https://github.com/Syawi/Petal/actions/workflows/windows.yml) also provides **Petal-Windows-x64**. Sign in to GitHub, download the artifact and extract `Petal.exe`.

## Using the wheel

1. Launch Petal.exe. Its blue icon appears in the system tray, possibly under hidden icons.
2. Hold **Shift** and right-click one supported file in File Explorer or on the desktop.
3. Click a format. Petal writes and checks the converted copy, then shows **Conversion complete**.

A normal right-click still opens the Windows menu, and so does the wheel's center (the Windows logo). Click outside or press **Esc** to dismiss the wheel; an active conversion continues in the background.

| Shortcut | Action |
| --- | --- |
| `1`–`6` | Convert using the corresponding petal |
| `Tab`, then `Enter` / `Space` | Select and activate an option |
| `M` | Open the Windows context menu |
| `Esc` | Hide the wheel or feedback |

## Themes

![Petal in liquid glass and classic themes, dark and light](docs/media/themes.png)

| Theme | Surfaces | Hover |
| --- | --- | --- |
| **Liquid glass** (default) | Frosted glass that refracts what is behind the menu. Colourless: every hue comes from your desktop. | The glass lights up with a white sheen and a brighter rim; labels stay readable. |
| **Classic** | Opaque Windows 11 surfaces. | The petal fills with your exact Windows accent colour and its label turns black. |

Both follow the Windows light/dark setting. Liquid glass takes its snapshot as the wheel opens, so moving content behind it, such as a video, does not update while the wheel is visible.

## Settings

Petal has two settings surfaces, both reached from its tray icon. Every change is saved immediately; there is no Apply button.

### Quick settings

![The liquid glass quick settings panel in dark and light](docs/media/quick-settings.png)

**Left-click** the tray icon. A glass panel bubbles in above the tray:

| Switch | What it does |
| --- | --- |
| **Liquid glass** | Turns the glass theme on, or returns to Classic. |
| **Animations** | Bubble opening, petal hover movement and animated feedback. This is Petal's own setting: it stays on even when Windows' *Animation effects* are off. |
| **Sound** | The chime that plays with the completion check. |

**All settings…** opens the full window. Close the panel with **Esc**, its **×** button or a click anywhere else. Arrow keys or **Tab** move between switches, and **Space** or **Enter** flips one.

### All settings

![The Settings window: theme, shortcut, launch at sign-in and completion sound](docs/media/settings.png)

Open it from **All settings…** in the quick panel, or **right-click** the tray icon and choose **Settings…**.

| Section | Controls |
| --- | --- |
| **Theme** (top right) | **Liquid glass** or **Classic** for the wheel. |
| **Open Petal** | Click the shortcut button, then press any key or mouse button, with optional Ctrl, Shift, Alt or Windows modifiers. **Esc** cancels the capture. Keyboard shortcuts act on the file under the pointer while File Explorer or the desktop is in front. Windows may reserve some combinations. **Reset** restores Shift + right-click. |
| **Launch at sign-in** | Starts Petal quietly in the tray when you sign in to Windows. On after the first launch. |
| **Completion sound** | Turns the chime on or off. **Choose WAV…** picks your own WAV file, **Preview** plays the current sound and **Use default** returns to the built-in *Paid* chime. If a chosen file disappears, Petal falls back to the built-in sound. |
| **Licenses** | Opens the third-party notices embedded in Petal.exe. |

### Tray menu

| Action | Result |
| --- | --- |
| Left-click the icon | Quick settings |
| Right-click → **Mute sound** | Toggles the completion sound |
| Right-click → **Settings…** | The full Settings window |
| Right-click → **Quit Petal** | Exits Petal. Unavailable while a conversion is running. |

Settings live per user under `HKEY_CURRENT_USER\Software\Petal`. Launch at sign-in uses the standard `Run` key. Nothing requires administrator rights.

## Development

```powershell
./Install-MediaEngine.ps1
cargo fmt --check
cargo test --locked
cargo build --release --locked
```

Windows CI checks formatting, runs the tests and builds the portable executable. Interactive desktop checks and the README-media exporter are opt-in. See [TESTING.md](TESTING.md), [CONTRIBUTING.md](CONTRIBUTING.md), [DESIGN.md](DESIGN.md) and [docs/MEDIA.md](docs/MEDIA.md).

```text
src/                 Native UI, glass renderer, conversions, notifications and sound
assets/              App mark, icon resources and completion sound
tools/               Reproducible sound generator
docs/media/          README images and animated preview
ffmpeg/              Media-engine notices; executables are downloaded locally
.github/workflows/   Windows build and test pipeline
```

## License and credits

Petal is [MIT licensed](LICENSE), copyright © 2026 Syawi. Third-party tools and assets retain their own licenses: [third-party notices](THIRD_PARTY_NOTICES.md), [asset sources](assets/SOURCES.md) and [media-engine sources](ffmpeg/SOURCES.md).
