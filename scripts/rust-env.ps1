# Optional repository-local toolchain, used when Visual Studio/Rust are not installed.
# Standard developer/CI environments simply use their existing PATH and SDK.
$repoRoot = Split-Path -Parent $PSScriptRoot
$localTools = Join-Path $repoRoot 'artifacts/toolchain'
if (Test-Path (Join-Path $localTools 'cargo/bin/cargo.exe')) {
    $env:CARGO_HOME = Join-Path $localTools 'cargo'
    $env:RUSTUP_HOME = Join-Path $localTools 'rustup'
    $env:PATH = "$env:CARGO_HOME\bin;$env:PATH"
}
if (Test-Path (Join-Path $localTools 'msvc/VC/Tools/MSVC')) {
    $compiler = Get-ChildItem (Join-Path $localTools 'msvc/VC/Tools/MSVC') -Directory | Select-Object -First 1
    $sdk = Get-ChildItem (Join-Path $localTools 'msvc/Windows Kits/10/Lib') -Directory | Select-Object -First 1
    $sdkBin = Join-Path $localTools ('msvc/Windows Kits/10/bin/' + $sdk.Name + '/x64')
    $env:PATH = "$($compiler.FullName)\bin\Hostx64\x64;$sdkBin;$env:PATH"
    $env:LIB = "$($compiler.FullName)\lib\x64;$($sdk.FullName)\ucrt\x64;$($sdk.FullName)\um\x64"
}
