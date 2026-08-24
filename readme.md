# Scheduled Screenshot Tool

A lightweight Windows 10 tray application for running multiple scheduled screenshot tasks. Each task can use its own timing, output directory, filename template, image format, cursor preference, and stop condition.

> Status: implemented, cross-compiled with Docker, and covered by an automated package-to-runtime smoke test on Windows. Interactive capture, session-event, multi-monitor, and resource acceptance testing still require Windows hardware or a Windows VM.

## Goals

- Support up to 100 independently enabled screenshot tasks.
- Capture every connected monitor into a separate image.
- Allow interval schedules from 1 second to 24 hours.
- Allow fixed local times with weekday selection.
- Stop a task manually, at an exact date/time, or after a duration.
- Keep idle CPU and memory usage low through a single event-driven scheduler.
- Store all configuration in a human-editable JSON file.
- Collect application events and errors in bounded local diagnostic logs.
- Provide an optional static, offline HTML editor without running a web server.
- Remain portable: no installer, administrator rights, telemetry, or network access.

## Intended platform

- Windows 10 22H2 x64
- .NET Framework 4.8
- WinForms notification-area application

Windows 10 22H2 includes .NET Framework 4.8, avoiding a separately installed application runtime.

## Quick start

1. Download the portable `ScheduledScreenshot-win-x64.zip` or build it using the instructions below.
2. Extract every file to a writable directory, such as `%LOCALAPPDATA%\ScheduledScreenshot`. Do not run the executable from inside the ZIP.
3. Run `ScheduledScreenshot.exe`. Windows may place its icon in the notification-area overflow menu.
4. Right-click the tray icon and select **Configure in Browser**. The editor is a local HTML file and does not start a web server or use the network.
5. In the editor, open the `settings.json` created beside the executable, add or edit a task, enable it, and save. If the browser downloads a replacement file, copy it over the original `settings.json`.
6. Leave the application running. Valid configuration changes are detected automatically, and enabled tasks start on their next scheduled occurrence.

Use **Capture Now** from the tray menu to test a task immediately. By default, screenshots are written under `%USERPROFILE%\Pictures\Scheduled Screenshots`, grouped into date directories, and diagnostic logs are stored in the `logs` directory beside the executable. Both locations can be changed in `settings.json`.

To stop all scheduled captures temporarily, select **Pause All**. Select **Exit** to close the application completely. To launch it automatically when you sign in, place a shortcut to `ScheduledScreenshot.exe` in the current user's Windows Startup folder (`shell:startup`).

## Operation

The application runs visibly in the Windows notification area. Its menu provides:

- Next scheduled task and occurrence
- Capture Now, with task selection
- Pause or resume all tasks
- Configure in Browser
- Edit JSON in Notepad
- Open a task's output folder
- Open Logs
- Exit

Only one application instance will run. The application will watch `settings.json` and apply valid edits without requiring a restart.

## Configuration

Each task has exactly one schedule type: `interval` or `fixed`.

```json
{
  "schemaVersion": 1,
  "paused": false,
  "logging": {
    "level": "info",
    "directory": "logs",
    "maxFileSizeMb": 5,
    "retainedFiles": 5
  },
  "tasks": [
    {
      "id": "491e5e22-748b-41c8-9df6-4a5b816f8c02",
      "name": "Workday capture",
      "enabled": true,
      "capture": {
        "outputFolder": "%USERPROFILE%\\Pictures\\Scheduled Screenshots\\Workday",
        "fileNameTemplate": "{timestamp}_{display}",
        "imageFormat": "jpeg",
        "jpegQuality": 85,
        "includeCursor": false
      },
      "schedule": {
        "type": "interval",
        "intervalSeconds": 60,
        "weekdays": ["Mon", "Tue", "Wed", "Thu", "Fri"],
        "activeHoursEnabled": true,
        "activeStart": "09:00:00",
        "activeEnd": "17:00:00"
      },
      "stopCondition": {
        "mode": "duration",
        "endAtLocal": null,
        "durationSeconds": 28800
      }
    }
  ]
}
```

Key rules:

- `id` is a stable GUID and task names are unique.
- `intervalSeconds` accepts values from 1 through 86,400.
- Fixed times and active-hour boundaries use local `HH:mm:ss` values.
- Stop modes are `none`, `at`, and `duration`.
- Duration values accept 1 second through 365 days.
- Active-hour windows must begin and end on the same day.
- `fileNameTemplate` controls the base filename; the selected image extension is appended automatically.
- Logging levels are `error`, `info`, and `debug`; `info` is the default.
- Unknown future properties are tolerated, but invalid required values reject the edit.

The static configuration editor will load a user-selected `settings.json`. In browsers that support direct file access it can save in place; otherwise, it downloads a replacement file. Editing the JSON in Notepad remains the universal fallback.

## Scheduling behavior

- All tasks share one scheduler and one one-shot timer.
- Interval cadence follows scheduled timestamps rather than capture completion.
- Tasks due at the same second are grouped by output folder, filename template, format, quality, and cursor preference.
- Tasks with identical capture settings share one physical capture.
- Different capture groups are processed sequentially.
- If a new occurrence becomes due while a capture batch is active, that occurrence is skipped rather than queued.
- Missed occurrences during shutdown, sleep, workstation lock, or system-clock changes are not replayed.
- The lock screen, secure desktop, and disconnected sessions are never captured.

A one-second task can create 86,400 images per monitor per day. The editor will warn about high-frequency schedules, but it will not silently impose a longer interval.

## Output

Each monitor is written separately under a date directory. The default filename template is:

```text
{timestamp}_{display}
```

Supported filename tokens are:

- `{timestamp}`: `yyyyMMdd_HHmmss_fff`
- `{date}`: `yyyyMMdd`
- `{time}`: `HHmmss`
- `{task}`: sanitized task name
- `{taskId}`: full task GUID
- `{taskId8}`: first eight GUID characters
- `{display}`: sanitized Windows display name
- `{displayIndex}`: one-based display number

The template must include `{display}` or `{displayIndex}` because every monitor is saved separately. It cannot contain path separators; subdirectories remain controlled by `outputFolder` and the application's date-directory layout. The application appends the format extension and adds a numeric collision suffix rather than overwriting an existing file.

Formats:

- JPEG, quality 50-100; default 85
- PNG, lossless

Screenshots are retained until the user deletes them. They inherit the selected directory's normal Windows permissions and are not encrypted or uploaded.

## Diagnostic logs

The application writes UTF-8 JSON Lines files under the configured directory, defaulting to `logs` beside the executable. Logs rotate daily or after 5 MB, whichever comes first, and retain the newest five files by default.

Each entry contains an ISO-8601 UTC timestamp, severity, stable event ID, message, and relevant task/batch/error context. Normal `info` logging records lifecycle, configuration changes, task completion/skip/end events, capture-batch summaries, session/power changes, and errors. `debug` additionally records scheduler decisions and per-monitor capture/file details.

Logs never contain screenshot pixels or configuration-file contents. Debug logging can grow quickly with one-second tasks, so it is opt-in and remains bounded by rotation. Log-write failures never stop scheduled capture.

## Resource targets

With the external configuration page closed:

- Less than 0.05% average idle CPU
- At most 40 MB private working set
- No polling loops, heartbeat writes, or timer per task
- Event-driven, size-capped diagnostic logging
- One monitor bitmap held in memory at a time
- One capture profile processed at a time

Continuous one-second capture is an active workload and is not expected to remain at idle CPU usage.

## Development

The application targets .NET Framework 4.8 and contains Windows-only WinForms/GDI code. Source editing and cross-compilation can happen on macOS or Linux, but execution and desktop integration tests require Windows.

### Docker build on macOS or Linux

The pinned multi-stage SDK image restores and compiles the application and test assembly:

```sh
docker build --target test-build -t scheduled-screenshot-build .
```

Export the compiled portable files without creating a runnable Linux image:

```sh
docker build --target artifacts --output type=local,dest=artifacts/docker .
```

The Linux container cross-compiles the .NET Framework application and executes eight linked core tests against the same scheduler, validation, and filename-template source files. It cannot execute WinForms, access an interactive Windows desktop, or verify Windows session/GDI behavior.

### Native Windows build and tests

With the .NET 10 SDK or Visual Studio installed:

```powershell
dotnet restore ScreenCapture.sln
dotnet build ScreenCapture.sln --configuration Release --no-restore --property:Platform=x64
dotnet test tests/ScheduledScreenshot.Tests/ScheduledScreenshot.Tests.csproj --configuration Release --no-build --property:Platform=x64
```

Run the end-to-end production smoke test to restore, build, package, extract, and launch the real portable executable in a clean temporary directory:

```powershell
.\scripts\test-production.ps1
```

The smoke test verifies:

- Required executable and editor files are present in the portable ZIP.
- The packaged application starts and remains running.
- Default `settings.json` is generated with schema version 1 and accepted.
- Diagnostic logs contain `APP_START`, `CONFIG_ACCEPTED`, and `APP_EXIT`.
- A second process is rejected by the single-instance guard without stopping the first.
- The application shuts down cleanly with exit code 0.

The smoke configuration contains no enabled capture tasks, so this check does not take a screenshot or replace the Windows monitor/session acceptance tests. Temporary files and processes are cleaned up even when the test fails.

To test an already-built production ZIP, use the same mode as CI:

```powershell
.\scripts\test-production.ps1 -NoBuild -PackagePath artifacts\ScheduledScreenshot-win-x64.zip
```

Release output is written to `src\ScheduledScreenshot\bin\x64\Release\net48`. The GitHub Actions workflow checks the offline editor, builds the release, runs unit tests, creates the portable ZIP and SHA-256 checksum, runs the production smoke test against that exact ZIP, and uploads both release artifacts.

### Offline editor check

If Node.js is installed:

```sh
node scripts/check-editor.js
```

This parses the editor's JavaScript and checks required offline configuration features.

## Repository layout

```text
src/ScheduledScreenshot/           WinForms application and offline editor
tests/ScheduledScreenshot.Tests/   Scheduler, validation, and filename tests
scripts/check-editor.js             Static editor verification
scripts/test-production.ps1         Build-to-production Windows smoke test
.github/workflows/                  Windows build, test, and packaging
Dockerfile                          Cross-platform restore/build environment
```

## Development plan

See [plan.md](plan.md) for the architecture, implementation phases, validation rules, and test matrix.
