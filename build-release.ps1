param([string]$FFmpegPath = '')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$projectRoot = $PSScriptRoot
$releaseDirectory = Join-Path $projectRoot 'release'
$packageDirectory = Join-Path $releaseDirectory 'Shiguang'

try {
    Get-Command cargo -ErrorAction Stop | Out-Null
    Get-Command npm.cmd -ErrorAction Stop | Out-Null
    if (!$FFmpegPath) {
        if ($env:SHIGUANG_FFMPEG) { $FFmpegPath = $env:SHIGUANG_FFMPEG }
        elseif (Test-Path -LiteralPath (Join-Path $packageDirectory 'ffmpeg.exe')) {
            $FFmpegPath = Join-Path $packageDirectory 'ffmpeg.exe'
        } else { $FFmpegPath = (Get-Command ffmpeg.exe -ErrorAction Stop).Source }
    }
    $FFmpegPath = (Resolve-Path -LiteralPath $FFmpegPath).Path
    $packageExe = Join-Path $packageDirectory 'Shiguang.exe'
    if (Get-Process -Name Shiguang -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $packageExe }) {
        throw 'Close the packaged Shiguang application before building.'
    }
    Push-Location $projectRoot
    try {
        if (!(Test-Path -LiteralPath 'node_modules')) {
            & npm.cmd ci
            if ($LASTEXITCODE -ne 0) { throw 'npm ci failed.' }
        }
        & npm.cmd run build
        if ($LASTEXITCODE -ne 0) { throw 'Frontend build failed.' }

        # custom-protocol embeds dist/ instead of requiring a Vite server.
        & cargo build --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol --bin dy-download
        if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
        $metadataJson = & cargo metadata --manifest-path src-tauri/Cargo.toml --format-version 1 --no-deps
        if ($LASTEXITCODE -ne 0) { throw 'Cannot locate Cargo output.' }
        $metadata = $metadataJson | ConvertFrom-Json
        $builtExe = Join-Path $metadata.target_directory 'release/dy-download.exe'
        if (!(Test-Path -LiteralPath $builtExe)) { throw "Release executable missing: $builtExe" }

        New-Item -ItemType Directory -Path $packageDirectory -Force | Out-Null
        Copy-Item -LiteralPath $builtExe -Destination $packageExe -Force
        $packagedFFmpeg = Join-Path $packageDirectory 'ffmpeg.exe'
        if ([IO.Path]::GetFullPath($FFmpegPath) -ne [IO.Path]::GetFullPath($packagedFFmpeg)) {
            Copy-Item -LiteralPath $FFmpegPath -Destination $packagedFFmpeg -Force
        }
        $ffmpegRoot = Split-Path (Split-Path $FFmpegPath -Parent) -Parent
        foreach ($name in @('LICENSE', 'README.txt')) {
            $source = Join-Path $ffmpegRoot $name
            if (Test-Path -LiteralPath $source) {
                Copy-Item -LiteralPath $source -Destination (Join-Path $packageDirectory "FFmpeg-$name") -Force
            }
        }
        Set-Content -LiteralPath (Join-Path $packageDirectory 'README.txt') -Encoding UTF8 -Value @'
Run Shiguang.exe. Keep ffmpeg.exe beside it.
New downloads are saved as JPEG images / MP4 videos.
Existing downloads are not converted or modified.
FFmpeg is used for new-download conversion and cached thumbnails.
'@
        $zipPath = Join-Path $releaseDirectory 'Shiguang-release.zip'
        Compress-Archive -LiteralPath $packageDirectory -DestinationPath $zipPath -Force
        Write-Host "Release ready: $packageExe" -ForegroundColor Green
        Write-Host "ZIP: $zipPath" -ForegroundColor Green
    } finally { Pop-Location }
} catch {
    Write-Error $_
    exit 1
}
