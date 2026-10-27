param([string]$Destination = (Join-Path $PSScriptRoot "ffmpeg"))
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$version = '9.0.2'
$expectedHash = '60F467265B1E312373DBCD92200C2618A74850F98D3D078E94296BB3FA2047BA'
$tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd([IO.Path]::DirectorySeparatorChar)
$downloadDirectory = [IO.Path]::GetFullPath((Join-Path $tempRoot ('Petal-media-' + [Guid]::NewGuid().ToString('N'))))
$destination = [IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Path $downloadDirectory | Out-Null
try {
    $archive = Join-Path $downloadDirectory 'ffmpeg.zip'
    Write-Host "Downloading FFmpeg $version (about 109 MB)..."
    Invoke-WebRequest -Uri "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-$version-essentials_build.zip" -OutFile $archive -TimeoutSec 300
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expectedHash) {
        throw 'FFmpeg checksum mismatch: installation stopped.'
    }
    $extracted = Join-Path $downloadDirectory 'extracted'
    Expand-Archive -LiteralPath $archive -DestinationPath $extracted
    $package = Join-Path $extracted "ffmpeg-$version-essentials_build"
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    foreach ($name in @('ffmpeg.exe', 'ffprobe.exe')) {
        Copy-Item -LiteralPath (Join-Path (Join-Path $package 'bin') $name) -Destination $destination -Force
    }
    foreach ($name in @('LICENSE', 'README.txt')) {
        Copy-Item -LiteralPath (Join-Path $package $name) -Destination $destination -Force
    }
    Write-Host 'Media engine ready. No service or system PATH was changed.'
} finally {
    $prefix = $tempRoot + [IO.Path]::DirectorySeparatorChar
    if ($downloadDirectory.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -and
        [IO.Path]::GetFileName($downloadDirectory).StartsWith('Petal-media-') -and
        (Test-Path -LiteralPath $downloadDirectory -PathType Container)) {
        Remove-Item -LiteralPath $downloadDirectory -Recurse -Force
    }
}
