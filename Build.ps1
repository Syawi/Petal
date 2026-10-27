$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    if (-not (Test-Path -LiteralPath 'ffmpeg/ffmpeg.exe') -or -not (Test-Path -LiteralPath 'ffmpeg/ffprobe.exe')) {
        & (Join-Path $PSScriptRoot 'Install-MediaEngine.ps1')
    }
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'Install Rust from https://rustup.rs, then reopen PowerShell.'
    }
    $hostInfo = rustc -vV
    if ($hostInfo -match 'host: x86_64-pc-windows-gnu') {
        $sysroot = rustc --print sysroot
        $bundledLinker = Join-Path $sysroot 'lib/rustlib/x86_64-pc-windows-gnu/bin/self-contained/x86_64-w64-mingw32-gcc.exe'
        if (Test-Path -LiteralPath $bundledLinker) {
            $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = $bundledLinker
            $env:PATH = "$(Split-Path $bundledLinker);$env:PATH"
        }
        if (-not (Get-Command as.exe -ErrorAction SilentlyContinue)) {
            throw 'GNU: add MinGW-w64 binutils (as.exe) to the build PATH, or use Rust MSVC with Visual Studio C++ tools.'
        }
    }
    cargo test --locked
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed.' }
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    Copy-Item -LiteralPath 'target/release/petal.exe' -Destination 'Petal.exe' -Force
    Write-Host 'Petal.exe is ready.'
} finally { Pop-Location }
