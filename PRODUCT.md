# Petal

<!-- impeccable:product-schema 1 -->

## Platform
Native Windows 11 desktop application, x64.

## Stack
Rust and Win32. GDI/GDI+ rendering with no web runtime. FFmpeg and FFprobe run only during media conversion. Native Rust handles images and text documents.

## Product Purpose
Convert a selected file directly from a six-option radial right-click menu in File Explorer and on the Windows desktop.

## Operating Context
A small background process. The center opens the original Windows context menu. The tray menu provides Settings, a persistent Mute sound setting and Quit Petal. The settings window configures startup, a captured keyboard or mouse shortcut, and completion audio. The default shortcut is Shift + right-click. Startup is enabled for the current user on first launch and can be disabled.

## Capabilities and Constraints
The original is always kept unchanged. Verified output is saved alongside it with the new extension. Never overwrite an existing destination. Unsupported animations, multiple media tracks and document content are refused when conversion would silently discard them.

Images: JPG/PNG/WebP/GIF/BMP/PDF, with TIFF replacing the current format. Audio: MP3/WAV/FLAC/M4A/OGG/MP4, with OPUS as the replacement. Video: MP4/MKV/WebM/MOV/MP3/WAV, with AVI as the replacement. Documents: TXT/DOCX/MD/HTML/RTF/PDF, with ODT as the replacement. At most six options; the current format is omitted.

Documents are converted as text and paragraphs, without retaining the original layout. Audio to MP4 produces a black video with sound; video to an audio format extracts the soundtrack. Scanned PDFs require OCR and are refused.

## Identity
Petal uses the approved blue three-petal ribbon mark. The same mark identifies the executable, tray and Windows notifications. The shipped interface, messages, documentation and build scripts are in English.

## Standalone distribution
v1.1 ships as one Petal.exe. Its logo, completion sound and license notices are embedded; runtime files use LOCALAPPDATA/Petal. The first audio/video conversion downloads the pinned, checksum-verified FFmpeg build to the user cache; subsequent conversions are offline. Images and text documents need no network. No files need to sit beside the executable.
