# Scheduled Screenshot on macOS

The native menu-bar app supports macOS 14 Sonoma or newer on Intel and Apple Silicon. It uses AppKit and ScreenCaptureKit and needs no .NET runtime or third-party packages. Windows continues to use the existing WinForms application.

## Build and launch

Install Xcode or Apple's Command Line Tools, then run from the repository root:

```sh
bash scripts/build-macos.sh --arch universal
```

Use `--arch arm64` or `--arch x86_64` to build one architecture. `--output /path/to/output` changes the output directory. The script builds and ad-hoc signs `artifacts/ScheduledScreenshot-macos-universal/Scheduled Screenshot.app`, packages it as a ZIP and generates a SHA-256 checksum. The macOS GitHub Actions workflow builds the universal bundle.

Copy the app to Applications and open it. These development bundles do not have a Developer ID signature or Apple notarization. For downloaded bundles, macOS may require an explicit **Open Anyway** approval in System Settings → Privacy & Security. Managed devices may require an administrator's approval. Public notarized distribution requires an Apple developer signing identity.

## Configure capture

1. Click the camera icon in the menu bar and choose **Grant Screen Recording Access**. Allow Scheduled Screenshot in System Settings → Privacy & Security → Screen Recording (called Screen & System Audio Recording on some versions). Restart the app if macOS asks you to.
2. Choose **Configure in Browser** to open the shared offline editor, then **Open Settings Folder** to find `settings.json`.
3. Open that file in the editor, add or edit tasks, enable them and save. When the browser downloads a replacement, replace the original `settings.json` in the settings folder. Valid edits apply automatically; invalid edits retain the last valid configuration.
4. Choose **Capture Now** and a task to capture immediately. Screen Recording permission is required for manual and scheduled captures.

The initial task is disabled. The menu also provides pause/resume, JSON editing, task output folders, diagnostic logs and quit. To start at login, add the app through System Settings → General → Login Items.

## Files and compatibility

Configuration, `runtime-state.json` and the default `logs` folder are stored in:

```text
~/Library/Application Support/ScheduledScreenshot/
```

Screenshots default to `~/Pictures/Scheduled Screenshots`, with separate images for each display under dated folders. Relative output/log directories resolve from the settings folder. Native captures use physical display pixels, PNG or JPEG, configured JPEG quality and optional cursor inclusion. Existing screenshots are never overwritten.

The app shares the Windows schema-version-1 JSON contract: interval and fixed schedules, weekdays, same-day active hours, pause, exact stop dates, duration stops and filename tokens. The Mac app accepts the Windows default `%USERPROFILE%` path syntax, mapping it to the current Mac home directory and converting its backslashes. Other Windows drive paths should be replaced with Mac paths. Unknown configuration fields survive app writes. Duration deadlines persist across restarts and continue while paused; occurrences missed during sleep or unavailable sessions are skipped.

Screen Recording permission is requested only through the explicit menu action. Captures are blocked while permission is denied, displays are asleep or the session is inactive/locked. Re-enable tasks after their stop condition has disabled them to start a new duration.

## Repeatable native verification

Run the process E2E case against a built executable:

```sh
python3 tests/cases/macos/e2e.py 'artifacts/ScheduledScreenshot-macos-universal/Scheduled Screenshot.app/Contents/MacOS/ScheduledScreenshot'
```

The case retains the app's actual settings, runtime state and logs under `tests/cases/macos/artifacts/`. It exercises configuration creation and atomic replacement, invalid edits, simultaneous exact stop times, duration termination across restart, unknown-field preservation and single-instance ownership. It does not request Screen Recording access.

Append `--capture` to exercise real PNG capture after granting Screen Recording access to the launching executable or terminal. Images are retained in the same case folder. Interactive menu, Retina/multiple-monitor, cursor, lock/unlock and sleep/wake checks need a visible Mac session.

For supervised headless operation, the executable accepts `--headless`, `--run-for SECONDS` and `--data-directory PATH`. An alternate data directory isolates configuration and single-instance ownership. Headless operation never prompts for recording permission.
