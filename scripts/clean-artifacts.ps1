<#
.SYNOPSIS
Preview or remove obsolete files and folders from this repository's artifacts directory.
.DESCRIPTION
Preserves artifacts/toolchain and artifacts/release in full, including release data.
Everything else directly under artifacts is selected for removal, including old
releases, screenshots, test output, temporary scripts, and legacy backups.
The default is a preview. Use -Apply to delete, or -Apply -WhatIf to preview.
Run ./scripts/package.ps1 afterward if you also want to rebuild the release.
.EXAMPLE
./scripts/clean-artifacts.ps1
.EXAMPLE
./scripts/clean-artifacts.ps1 -Apply
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param([switch]$Apply)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$artifactRoot = Join-Path $repoRoot 'artifacts'

if (-not (Test-Path -LiteralPath $artifactRoot)) {
    Write-Output 'No artifacts directory to clean.'
    return
}

$rootItem = Get-Item -LiteralPath $artifactRoot -Force
if (-not $rootItem.PSIsContainer -or ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
    throw 'The artifacts directory must be a regular directory, not a link or junction.'
}
$artifactRoot = (Resolve-Path -LiteralPath $artifactRoot).ProviderPath
$artifactPrefix = $artifactRoot.TrimEnd('\') + '\'
$targets = @(Get-ChildItem -LiteralPath $artifactRoot -Force |
    Where-Object { $_.Name -notin @('toolchain', 'release') })

# Validate every descendant before deleting anything. Walk one level at a time
# so a junction or symbolic link is rejected before it can be traversed.
$pending = New-Object 'System.Collections.Generic.Stack[System.IO.FileSystemInfo]'
foreach ($target in $targets) { $pending.Push($target) }
while ($pending.Count -gt 0) {
    $item = $pending.Pop()
    $resolved = (Resolve-Path -LiteralPath $item.FullName).ProviderPath
    if (-not $resolved.StartsWith($artifactPrefix, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Cleanup target is outside artifacts: $resolved"
    }
    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "Cleanup stopped: inspect this link or junction manually: $resolved"
    }
    if ($item -is [IO.DirectoryInfo]) {
        foreach ($child in Get-ChildItem -LiteralPath $resolved -Force) {
            $pending.Push($child)
        }
    }
}

Write-Output "Keeping: $(Join-Path $artifactRoot 'toolchain')"
Write-Output "Keeping: $(Join-Path $artifactRoot 'release') (including settings and captures)"
if ($targets.Count -eq 0) {
    Write-Output 'No obsolete artifacts to remove.'
    return
}

$removed = 0
foreach ($target in $targets) {
    if (-not $Apply) {
        Write-Output "Would remove: $($target.FullName)"
    } elseif ($PSCmdlet.ShouldProcess($target.FullName, 'Recursively delete artifact')) {
        Remove-Item -LiteralPath $target.FullName -Recurse -Force
        $removed++
        Write-Output "Removed: $($target.FullName)"
    }
}
if (-not $Apply) {
    Write-Output "Preview: $($targets.Count) entries selected. Run with -Apply to delete them."
} else {
    Write-Output "Removed $removed of $($targets.Count) selected entries."
}
