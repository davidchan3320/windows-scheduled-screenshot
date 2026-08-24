[CmdletBinding()]
param(
    [switch]$NoBuild,
    [string]$PackagePath
)

$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path -Parent $PSScriptRoot
$releaseOutput = Join-Path $repositoryRoot 'src\ScheduledScreenshot\bin\x64\Release\net48'
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ('ScheduledScreenshot-production-smoke-' + [Guid]::NewGuid().ToString('N'))
$extractedDirectory = Join-Path $temporaryRoot 'portable'
$primaryProcess = $null

function Assert-Condition {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Test-LogEvent {
    param(
        [string]$Directory,
        [string]$EventId
    )

    $logFiles = @(Get-ChildItem -LiteralPath $Directory -Filter '*.jsonl' -File -ErrorAction SilentlyContinue)
    foreach ($logFile in $logFiles) {
        if (Select-String -LiteralPath $logFile.FullName -SimpleMatch ('"eventId":"' + $EventId + '"') -Quiet) {
            return $true
        }
    }
    return $false
}

try {
    if (-not $NoBuild) {
        Push-Location $repositoryRoot
        try {
            & dotnet restore ScreenCapture.sln
            Assert-Condition ($LASTEXITCODE -eq 0) 'dotnet restore failed.'
            & dotnet build ScreenCapture.sln --configuration Release --no-restore --property:Platform=x64
            Assert-Condition ($LASTEXITCODE -eq 0) 'Release build failed.'
        }
        finally {
            Pop-Location
        }
    }

    New-Item -ItemType Directory -Path $temporaryRoot | Out-Null

    if ([string]::IsNullOrWhiteSpace($PackagePath)) {
        Assert-Condition (Test-Path -LiteralPath $releaseOutput -PathType Container) "Release output was not found: $releaseOutput"
        $PackagePath = Join-Path $temporaryRoot 'ScheduledScreenshot-win-x64.zip'
        Compress-Archive -Path (Join-Path $releaseOutput '*') -DestinationPath $PackagePath
    }
    else {
        if (-not [IO.Path]::IsPathRooted($PackagePath)) {
            $PackagePath = Join-Path $repositoryRoot $PackagePath
        }
        $PackagePath = [IO.Path]::GetFullPath($PackagePath)
    }

    Assert-Condition (Test-Path -LiteralPath $PackagePath -PathType Leaf) "Production package was not found: $PackagePath"
    Expand-Archive -LiteralPath $PackagePath -DestinationPath $extractedDirectory

    $executablePath = Join-Path $extractedDirectory 'ScheduledScreenshot.exe'
    $editorPath = Join-Path $extractedDirectory 'config-editor.html'
    Assert-Condition (Test-Path -LiteralPath $executablePath -PathType Leaf) 'The package does not contain ScheduledScreenshot.exe.'
    Assert-Condition (Test-Path -LiteralPath $editorPath -PathType Leaf) 'The package does not contain config-editor.html.'

    $primaryProcess = Start-Process -FilePath $executablePath -ArgumentList '--smoke-test' -WorkingDirectory $extractedDirectory -PassThru
    $settingsPath = Join-Path $extractedDirectory 'settings.json'
    $logDirectory = Join-Path $extractedDirectory 'logs'
    $startupDeadline = [DateTime]::UtcNow.AddSeconds(6)
    do {
        Start-Sleep -Milliseconds 100
        $primaryProcess.Refresh()
        $started = (Test-Path -LiteralPath $settingsPath -PathType Leaf) -and
            (Test-LogEvent -Directory $logDirectory -EventId 'APP_START')
    } while (-not $started -and -not $primaryProcess.HasExited -and [DateTime]::UtcNow -lt $startupDeadline)

    Assert-Condition (-not $primaryProcess.HasExited) 'The production executable exited during startup.'
    Assert-Condition $started 'The production executable did not create settings and log APP_START.'

    $settings = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
    Assert-Condition ($settings.schemaVersion -eq 1) 'The generated production settings have an unexpected schema version.'

    $secondProcess = Start-Process -FilePath $executablePath -ArgumentList '--smoke-test' -WorkingDirectory $extractedDirectory -PassThru -Wait
    Assert-Condition ($secondProcess.ExitCode -eq 2) "A second instance returned $($secondProcess.ExitCode) instead of 2."
    $primaryProcess.Refresh()
    Assert-Condition (-not $primaryProcess.HasExited) 'The primary instance stopped after the single-instance check.'

    Assert-Condition ($primaryProcess.WaitForExit(12000)) 'The smoke-test instance did not shut down cleanly.'
    Assert-Condition ($primaryProcess.ExitCode -eq 0) "The smoke-test instance returned $($primaryProcess.ExitCode)."
    Assert-Condition (Test-LogEvent -Directory $logDirectory -EventId 'CONFIG_ACCEPTED') 'The production run did not accept its configuration.'
    Assert-Condition (Test-LogEvent -Directory $logDirectory -EventId 'APP_EXIT') 'The production run did not log a clean shutdown.'

    Write-Host 'Production smoke test passed: build/package, portable startup, configuration, logging, single-instance guard, and clean shutdown.'
}
finally {
    if ($null -ne $primaryProcess) {
        $primaryProcess.Refresh()
        if (-not $primaryProcess.HasExited) {
            Stop-Process -Id $primaryProcess.Id -Force -ErrorAction SilentlyContinue
        }
        $primaryProcess.Dispose()
    }

    $resolvedTemporaryRoot = [IO.Path]::GetFullPath($temporaryRoot)
    $resolvedSystemTemporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
    if ($resolvedTemporaryRoot.StartsWith($resolvedSystemTemporaryRoot, [StringComparison]::OrdinalIgnoreCase) -and
        (Test-Path -LiteralPath $resolvedTemporaryRoot)) {
        Remove-Item -LiteralPath $resolvedTemporaryRoot -Recurse -Force
    }
}
