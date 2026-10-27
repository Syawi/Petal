# External media engine

FFmpeg and FFprobe 9.0.2, Windows x64 essentials build by Gyan Doshi.
The provider is listed at https://ffmpeg.org/download.html .

Archive: https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-9.0.2-essentials_build.zip
SHA-256: 60F467265B1E312373DBCD92200C2618A74850F98D3D078E94296BB3FA2047BA

Build license: GPL v3, included in LICENSE. Build source and configuration:
https://github.com/FFmpeg/FFmpeg/commit/946fcce07b
and README.txt distributed with the build. The executables are used unmodified as separate processes. Build library details: https://www.gyan.dev/ffmpeg/builds/ .

The standalone application installs the engine automatically into %LOCALAPPDATA%/Petal/media/9.0.2 on first audio/video conversion. Source builds can use Install-MediaEngine.ps1. After the one-time setup, no downloads occur during conversion. The application uses its own cached engine (or the adjacent ffmpeg/ directory in source builds); it does not search the system PATH.
