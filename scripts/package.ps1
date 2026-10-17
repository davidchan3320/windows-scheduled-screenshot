[CmdletBinding()]
param([switch]$NoBuild, [switch]$Native, [string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $release = if ($OutputDirectory) {
        [IO.Path]::GetFullPath($OutputDirectory)
    } else {
        Join-Path $root 'artifacts/release'
    }
    if (-not $Native -and -not $NoBuild) {
        if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
            throw 'Docker is required. Start Docker with Linux containers, or use -Native with a Windows Rust/MSVC toolchain.'
        }
        docker build --platform linux/amd64 --target release --output "type=local,dest=$release" .
        if ($LASTEXITCODE -ne 0) { throw 'Docker release build failed. Check that Docker is running with Linux containers.' }
        Write-Output "Portable artifacts: $release"
        return
    }
    if (-not $NoBuild) {
        . "$PSScriptRoot/rust-env.ps1"
        cargo build --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }
    }
    $exe = Join-Path $root 'target/x86_64-pc-windows-msvc/release/ScheduledScreenshot.exe'
    if (-not (Test-Path $exe)) { throw 'Release executable is missing.' }
    New-Item -ItemType Directory -Force $release | Out-Null
    Copy-Item -LiteralPath $exe -Destination (Join-Path $release 'ScheduledScreenshot.exe') -Force
    $zip = Join-Path $release 'ScheduledScreenshot-win-x64.zip'
    Compress-Archive -LiteralPath (Join-Path $release 'ScheduledScreenshot.exe') -DestinationPath $zip -Force
    $lines = foreach ($file in @('ScheduledScreenshot.exe', 'ScheduledScreenshot-win-x64.zip')) {
        (Get-FileHash -LiteralPath (Join-Path $release $file) -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + $file
    }
    $lines | Set-Content -LiteralPath (Join-Path $release 'SHA256SUMS.txt') -Encoding ascii
    Write-Output "Portable artifacts: $release"
} finally { Pop-Location }
