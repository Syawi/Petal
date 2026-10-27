# Formats and limitations

| Source family | Six choices | Replacement for the current format |
| --- | --- | --- |
| Image | JPG, PNG, WebP, GIF, BMP, PDF | TIFF |
| Audio | MP3, WAV, FLAC, M4A, OGG, MP4 | OPUS |
| Video | MP4, MKV, WebM, MOV, MP3, WAV | AVI |
| Document | TXT, DOCX, MD, HTML, RTF, PDF | ODT |

The current format is omitted; an alternative takes its place. PDF is last for images and documents, except when PDF is the source. Aliases and case are recognized.

## Images
Inputs: JPG/JPEG, PNG, WebP, GIF, BMP, TIFF/TIF and ICO. Animated and multipage inputs are refused to preserve all frames. Orientation is applied; transparency is flattened on white for JPG, BMP and image PDF. Image PDFs are a single proportionate page, with lossless RGB compression and no printer dependency. Metadata and ICC profiles are not preserved.

## Audio and video
Audio inputs: MP3, WAV/WAVE, FLAC, M4A/M4B, AAC, OGG/OGA, OPUS, WMA, AIF/AIFF. Video inputs: MP4/M4V, MKV, WebM, MOV/QT, AVI, WMV, MPEG/MPG, M2TS/MTS/TS, OGV, 3GP. Actual codecs must be readable by FFmpeg.

MP4/MKV/MOV output: H.264 and AAC; WebM: VP9 and Opus; AVI: MPEG-4 and MP3. Audio to MP4 creates a 1280x720 black video with sound. Video to MP3/WAV extracts sound. MP3 uses 192 kbit/s stereo, M4A AAC 192 kbit/s, WAV PCM 16-bit, FLAC lossless, OGG Vorbis and OPUS 160 kbit/s. Lossy conversion may affect quality.

Video dimensions are retained, with at most one padding pixel to make them even; maximum 8192x8192. Unknown duration, multiple audio tracks, and missing audio for audio extraction are refused. Multiple video tracks and subtitles are refused for video output. Metadata and cover art are not guaranteed.

The source stays locked while a local temporary copy is processed. After sync, track, duration and dimension checks, the complete output is decoded before publication. Allow space for both the temporary source copy and result. A forced shutdown can leave a .petal-media-* directory.

## Text documents
Inputs: TXT/TEXT/LOG/CSV/TSV, Markdown MD/MARKDOWN, HTML/HTM, RTF, DOCX, ODT and text PDFs. Binary DOC and Pages are unsupported. Plain text must be UTF-8, or UTF-16 LE/BE with BOM. RTF code pages are decoded by Windows RichEdit. CSV/TSV are treated as raw text.

Conversions retain text and paragraphs, not formatting, layout, styles or metadata. DOCX/ODT are real ZIP/XML containers. HTML/Markdown output escapes special characters. No Office suite is required.

Text PDF output is searchable A4 with margins, line wrapping and pagination. The Windows Tahoma font is embedded with Unicode mapping. Missing glyphs and scripts requiring specialized shaping are refused. Images, embedded objects, scanned/mixed PDFs, forms, annotations and unsupported complex structures are refused. Links, automatic lists, notes, headers and footers may also trigger refusal. No OCR.

Limits: 64 MiB input, 2 MiB extracted text, 2000 PDF pages and bounded decompressed XML/streams. Output is parsed again and all non-whitespace characters checked before publication; whitespace and line breaks can change. A forced shutdown can leave a .petal-text-*.tmp file.

## Replacement and integration
The source is removed permanently only after verified output is synced and published. Existing destinations are never overwritten, including files created during conversion. If source deletion fails, both files are retained. Read-only files, links and online-only cloud placeholders are refused.

File Explorer and the Windows desktop are supported, one file at a time. Third-party file managers and file-picker dialogs are not intercepted. Shift/Ctrl/Alt + right-click keeps the Windows menu. No automatic startup is installed. Closing the UI does not cancel conversion.
