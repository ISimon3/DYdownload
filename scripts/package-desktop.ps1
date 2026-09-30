param([string]$FFmpegPath = '')
& (Join-Path $PSScriptRoot '../build-release.ps1') -FFmpegPath $FFmpegPath
