[CmdletBinding()]
param([switch]$NoBuild, [switch]$Browser)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/rust-env.ps1"
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    cargo test --locked --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'Rust tests failed.' }
    & "$PSScriptRoot/package.ps1" -NoBuild:$NoBuild
    $smokeArguments = @('tests/native/smoke.py', '--exe', 'artifacts/release/ScheduledScreenshot.exe')
    if ($Browser) { $smokeArguments += '--browser' }
    python @smokeArguments
    if ($LASTEXITCODE -ne 0) { throw 'Packaged executable smoke test failed.' }
} finally { Pop-Location }
