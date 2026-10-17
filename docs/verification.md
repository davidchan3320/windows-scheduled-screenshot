# Verification — 2026-10-18

Implementation and current-machine checks are complete. **Full release qualification is still pending** a completed 24-hour soak and the clean-OS/physical-device matrix below. The earlier soak stopped before completion; see the 03:04 update below.

## Docker cache fixes, 2026-10-18 05:23 +08:00

- Pinned Windows SDK 10.0.26100 and CRT 14.44.17.14. SDK preparation checks version selection and copies the prepared files into a reusable toolchain layer before source changes can invalidate it. An offline SDK check without the preparation cache mount passed.
- GitHub Actions now explicitly restores/saves Cargo registry and target mounts, using a commit-pinned cache-dance action and a cache key covering the Dockerfile, Cargo configuration, manifest and lockfile. Docker's `type=gha,mode=max` cache retains the SDK layer. `actionlint` 1.7.7 passed; remote GitHub cache restore/save has not been executed.
- The first source-change test exposed stale application reuse from cached Cargo timestamps. The build now cleans only the application's package outputs before compiling, retaining dependency outputs. The repeated test reused the SDK layer, compiled only `scheduled-screenshot` in 20.57 seconds, and embedded the changed HTML in the executable. The production export excludes the test marker and has a different checksum. Test context and output are isolated under `artifacts/`; repository sources were not changed for this test.
- `./scripts/package.ps1 -OutputDirectory artifacts/release-docker` passed. ZIP identity, SHA-256 checksums, x64 GUI PE, static CRT imports, embedded manifest and Windows activation-context checks passed. `docker build --check .` reported no warnings, the Docker skill's build verification passed, and `git diff --check` passed.
- Desktop smoke, soak and clean-OS qualification were not rerun for this build-only change.

## Docker release build, 2026-10-18 05:02 +08:00

- `./scripts/package.ps1 -OutputDirectory artifacts/release-docker` passed using Docker Linux containers, Rust 1.90.0 and cargo-xwin 0.23.1. The output contains the Windows x64 executable, executable-only ZIP and SHA-256 checksums. Packaging now defaults to Docker; CI builds the release on Linux after Windows checks.
- The initial build exposed LLVM 14 corrupting UAC XML namespaces when merging a generated UAC fragment with the supplied manifest. `/MANIFESTUAC:NO` fixes this while retaining the supplied `asInvoker` declaration. Docker packaging now rejects invalid manifests, architecture and dynamic CRT imports; the check accepted a valid package and rejected the original malformed manifest.
- Final artifact checks passed: ZIP identity, both SHA-256 checksums, x64 GUI PE, static CRT imports, UAC/DPI/long-path manifest settings, and Windows `CreateActCtxW` acceptance of the embedded manifest. Report: `artifacts/docker-release-verification.json`. Final executable size: 1,495,552 bytes.
- Native release packaging with the manifest fix, native artifact checks, `-NoBuild` compatibility, Rust formatting, script syntax and whitespace checks passed. The final Docker rebuild reused the SDK and Cargo caches; Cargo completed in 0.08 seconds after installing the packaging validator.
- Native desktop smoke, the full soak and clean-OS qualification were not rerun. The existing application remained running; loader validation did not launch another app instance. The GitHub workflow was updated but not executed remotely.

## Project structure cleanup, 2026-10-18 04:03 +08:00

- Rust sources now use `src/`; native Python checks use `tests/native/`; browser runner/suite use `tests/browser/`; the manifest lives in `resources/`. Cargo, npm, CI, embedded-manifest paths, and production smoke references were updated. See [project structure](project-structure.md) for current commands; earlier entries below retain historical paths.
- Formatting, Clippy, all 28 Rust tests, CSS freshness, JavaScript/Python/PowerShell syntax, native-test import/root resolution, current documentation links, and whitespace checks passed.
- The initial browser run could not find Playwright's browser binary. The complete isolated browser suite passed using the supported `CHROME_PATH` override with installed Chrome.
- Release packaging passed in `artifacts/release-structure/`; ZIP identity, SHA-256 checksums, and embedded Windows manifest markers were verified. Existing artifacts and archived sources were preserved. Native desktop smoke and full release qualification were not rerun for this structural change.

## Single-instance update

- All portable copies now share one app instance per Windows session. A duplicate launch reopens the running manager and exits before loading settings.
- Package: `artifacts/release-single-instance/ScheduledScreenshot.exe` (1,316,864 bytes); SHA-256: `5e2621d4763da43c681a21a894a6e8f2150b3997e9aec6af9f842ba1ddb11ac4`.
- `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets -- -D warnings`, and all 28 Rust tests passed.
- `python scripts/smoke.py --exe artifacts/release-single-instance/ScheduledScreenshot.exe` passed, including duplicate launches from the same and different folders, no data creation for the second copy, and restart after exit. Report: `artifacts/portable smoke ?? d8bedc63 moved/smoke-results.json`.
- Exit older builds before using this package; older builds used independent per-folder guards. Native smoke/soak tests also require the app to be closed first.

## Earlier packaged executable

- Earlier update: `artifacts/release-save-folder/ScheduledScreenshot.exe`, 1,295,872 bytes. Earlier packages were left untouched.
- SHA-256: `b01c0f2f291b6f8743b08ff8d873df6d77cb114eba8ec1e589fc43a9f017a9f3`.
- ZIP: 559,873 bytes; verified to contain only the byte-identical executable.
- `dumpbin /dependents` shows only Windows system DLLs. No MSVC redistributable, .NET, Node or WebView import.
- MSVC CRT static linkage is set in `.cargo/config.toml`; asInvoker, PerMonitorV2 and long-path awareness are embedded in the manifest.

## Passed

Test machine: Windows 10 Pro 22H2, build 19045, one active monitor. This machine has developer software and is not a clean VM.

| Command/check | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --locked --all-targets -- -D warnings` | Passed |
| `cargo test --locked --all-targets` | 23 passed |
| `node --check ui/app.js` | Passed |
| `python -m py_compile scripts/smoke.py scripts/soak.py scripts/notices.py` | Passed |
| `cargo build --locked --example ui-test-host` and `node scripts/browser-test.cjs` | Passed browser suite with desktop capture disabled |
| `scripts/package.ps1` | Release EXE, ZIP and SHA-256 generated |
| `python scripts/smoke.py --browser --exe artifacts/release/ScheduledScreenshot.exe` | Passed on final packaged executable |
| `git diff --check` | Passed |

Core tests cover schema/type rejection, defaults, expired deadlines, interval cadence and long intervals, active hours/weekdays, DST gaps/folds, missed/claimed once tasks, legacy fixedOnce, count/duration limits, state reconciliation, filename tokens/reserved names/collisions, relative/non-ASCII/percent-containing paths, atomic writes, stale revisions, last-good recovery, migration, corrupt runtime refusal, interrupted-once recovery, HTTP Host/Origin/token checks, unwritable-path errors and per-folder single-instance guards.

Browser/native smoke covers create/edit/duplicate/delete, disabled defaults, previews, safe rendering of markup-like names, stale-edit draft preservation, closing/reopening all browser tabs, no external assets, real JPEG/PNG/cursor capture, exact successful counts, independent output failure, simultaneous dated once and legacy fixedOnce, manual-capture independence, native folder-picker cancellation, Startup shortcut creation/removal, Unicode target/arguments, and moving the executable with data. Session lock/unlock and clock notifications were simulated on the test window; the user's session was not physically locked. Desktop/mobile/editor screenshots were inspected.

Pre-UI-update native report/screenshots: `artifacts/portable smoke 截圖 3df344a1 moved/`. The final browser suite also verifies the embedded dependency-notices link and content. Legacy sources are locally preserved in both `artifacts/legacy-before-rust.zip` and `artifacts/retired-legacy/`; the original uncommitted patch is versioned under `docs/archive/`.

## UI update verification, 02:58 +08:00

- Replaced the long editor with accessible Schedule / Files & image / Stop conditions tabs, a live capture summary and a persistent action footer. Added interval/duration units, individual capture-time controls, weekday presets, filename-token insertion and unsaved-draft protection. Task rows have clearer hierarchy and stack on narrow windows.
- `cargo build --locked --example ui-test-host` and `node scripts/browser-test.cjs`: passed. New coverage checks unit persistence, second-level time preservation, weekday presets, token insertion, keyboard tab navigation, validation in a hidden tab, empty time-list validation and accepting/declining discard.
- Save remains visible at 1280x720 and 390x844. Desktop, compact and mobile editor/list screenshots were visually inspected under `artifacts/browser-ci/`; narrow task lists have no horizontal document overflow.
- `node --check ui/app.js`, `node --check tests/browser.cjs` and `git diff --check`: passed. An initial browser test failed because its removal loop retained an obsolete positional locator; the loop was corrected and the complete suite passed.
- Release build succeeded. Default packaging could not overwrite the running previous EXE; `./scripts/package.ps1 -NoBuild -OutputDirectory artifacts/release-ui` succeeded. ZIP contents and byte identity verified.
- `python scripts/smoke.py --browser --exe artifacts/release-ui/ScheduledScreenshot.exe`: passed. Report/screenshots: `artifacts/portable smoke 截圖 99667926 moved/`.
- No Rust scheduler or native capture source changed in this UI update. The original 23-test Rust result remains applicable; native smoke was rerun on the new package. The running full-day soak below still targets the earlier binary, not this updated package.

## Task status alignment, 03:14 +08:00

Enable/disable controls now occupy a consistent column beside the task name. Checkbox and text share a centered baseline, with paths on a separate line across desktop and mobile layouts. The existing browser suite, JavaScript syntax and diff checks passed. Desktop/mobile screenshots were visually checked. Release packaging and ZIP byte identity passed.

## Traditional Chinese language pack, 03:22 +08:00

- English and zh-Hant JSON packs are embedded in the EXE. The language selector saves to portable settings, defaults old settings to English, and uses revision checks. UI/help/common errors/results, dynamic schedule summaries, accessible labels, tray menu and picker title are localized. User data/tokens remain unchanged; Windows-owned controls and original diagnostic details follow the OS/source language.
- `cargo fmt --all`, `cargo clippy --locked --all-targets -- -D warnings`, all 25 core tests, both JavaScript syntax checks and diff checks passed. The strengthened language persistence test also passed after restarting the App instance. Pack tests enforce matching keys/placeholders and native label lookup.
- Existing and expanded browser tests passed in English and Traditional Chinese. Coverage includes saving/reloading the language, a fresh browser context, Chinese task names, validation, preserved times/weekdays/tokens, switching back to English and mobile document width. Desktop/mobile screenshots under artifacts/browser-ci/zh-Hant-*.png were visually inspected.
- `python scripts/smoke.py --browser --exe artifacts/release-zh-Hant/ScheduledScreenshot.exe` passed the packaged native/browser suite. Report: the artifacts portable-smoke directory ending in `c381115b moved`.
- `python artifacts/check-locale-native.py` passed actual packaged EXE restart persistence, all six visible native tray-menu labels, translated native picker title and cancellation. Report: `artifacts/locale-native-9b86f6cd/locale-result.json`.
- Package ZIP byte identity passed. Full-day/clean-OS/physical-device release gates remain pending as described below.

## Schedule wording, 03:29 +08:00

Changed the schedule label to "At specific times" and updated the Traditional Chinese translation. Schedule behavior is unchanged. Matching language-pack keys/placeholders test, release packaging, ZIP identity and diff checks passed.

## Standalone Capture now, 03:34 +08:00

- Browser and tray Capture now no longer select a saved task. The action uses default JPEG quality 85, no cursor, date subfolders and timestamp/display filenames under captures beside the executable. The browser has one global action, latest-result feedback and an Open captures folder button. Task rows contain no capture action. English/zh-Hant labels were updated.
- Formatting, Clippy, 25 core tests, JavaScript/Python syntax and diff checks passed. Expanded browser suite verified a global capture request with no task ID and no tasks, removed row actions, bilingual layout and the existing task-editing flows. Desktop/mobile screenshots were visually checked.
- `python scripts/smoke.py --browser --exe artifacts/release-standalone/ScheduledScreenshot.exe` passed. New native assertions cover no-task capture while paused, manual output failure/recovery, default JPEG/date output despite a saved task having a broken PNG destination, preservation of all task settings/progress, rejection of task IDs and rejection while the desktop is unavailable. Existing capture/migration/picker/session smoke checks also passed. Report: portable-smoke folder ending in `eb36bd32 moved`.
- `python artifacts/check-standalone-tray.py` verified the actual Chinese tray Capture now command with zero tasks and scheduling paused. A first attempt using posted keyboard messages did not activate the menu item and timed out; clicking the native menu item's reported rectangle passed. Report: `artifacts/standalone-tray-fa58044a/tray-result.json`. The test restored the cursor and closed its own instance.
- Release packaging and ZIP byte identity passed. Required full-day, clean-OS and physical-device release gates remain pending.

## Resource measurement and running soak

**03:09 Open folder update:** Added a button below each task's output path and `/api/open-folder`, accepting a saved task ID under the existing token/origin protection. It creates missing output folders, then opens the root in File Explorer. The tray shares the same implementation. Formatting, Clippy, all 23 core tests, JavaScript syntax and browser smoke passed. Direct browser/native verification checked the actual Explorer location for a relative Unicode path with spaces, folder creation, missing-task rejection and rejection of a path pointing to a file. Test Explorer windows were closed. Mobile layout was visually checked. New package destination: `artifacts/release-folder`.

**03:04 update:** The prior soak is no longer active. Its report has `complete=false`, 19 successful captures and 1,156.99 elapsed seconds. Driver stderr reports `Scheduler exited unexpectedly`; the cause is not established. The running-status notes below describe its earlier launch, not its current state. A fresh full-day run remains required.

The latest small UI change makes **Add time** use the current local hours/minutes/seconds when clicked. `node --check ui/app.js`, the existing browser suite, a direct browser clock comparison, and release packaging passed. Existing saved times are retained when opening an editor.

A short diagnostic on the same implementation before embedding dependency notices measured:

| Measurement | Observed |
|---|---:|
| Browser-closed idle duration | 60.0003 seconds |
| Average idle CPU, one-core basis | 0.00% at Windows timer resolution |
| Idle private working set | 1,368,064 bytes (1.30 MiB) |
| Idle handles | 164 |
| Observed successful scheduled captures over two minutes | 23 |
| Private working set after capture activity | 1,765,376 bytes (1.68 MiB) |
| Handles after capture activity | 177 |

Both idle targets passed this short measurement. Handle growth includes lazy initialization; this duration does not establish long-term absence of leaks. Source report: `artifacts/soak-6a26211e/soak-results.json`.

The requested **24-hour soak of the pre-UI-update EXE is running**, started 2026-10-18 02:40:05 +08:00. Its SHA-256 is `d0ceaf51886a523139726d8f1bc4f5eccd0e971e588f7787845b2c0ee56d8edf`. It runs hidden, takes one test capture per minute when the desktop is available, removes generated test captures, and exits its test instance when complete. Startup idle sampling adds approximately one minute before the 24-hour interval.

- Driver PID at launch: `47576`; test application PID: `52196`.
- Session metadata: `artifacts/soak-24h-session.json`.
- Progress/results: `artifacts/soak-5cf684ef/soak-results.json`.
- Driver output/error: `artifacts/soak-24h.stdout.log`, `artifacts/soak-24h.stderr.log`.
- Expected earliest completion: approximately 2026-10-19 02:41 +08:00, provided the machine remains running. Do not treat an absent error or elapsed wall-clock time as a pass; inspect `complete`, `full24Hours`, samples, counts, desktop availability and memory/handle trends.

An initial soak started at 02:36 was deliberately stopped after correcting the missing notices footer link, then restarted against the pre-UI-update checksum recorded in this section. Its report is `artifacts/soak-475adb7a/soak-results.json`; its "Scheduler exited unexpectedly" assertion reflects the test instance being stopped for repackaging, not an application crash. That initial run independently measured 1,409,024 bytes idle private working set (1.34 MiB) and 0.00% CPU over 60 seconds. Initial driver logs/session metadata are preserved with the `initial-soak-24h` prefix.

## Outstanding release gates

- Finish and inspect the 24-hour soak; verify scheduling reliability and post-warmup memory/handle trends.
- Run on clean Windows 10 22H2 and Windows 11 x64 without developer tools or separate runtimes.
- Physical lock/unlock, sleep/resume, remote disconnect/reconnect, secure desktop, and real clock/timezone changes.
- Multiple monitors, mixed DPI, negative monitor coordinates, hot-plug/reconfiguration during capture.
- Real ACL-denied application/output folders, disk-full behavior and failure during atomic replacement. The current unwritable-directory test uses an obstructing file, not an ACL change.

No clean VM or physical display configuration was provisioned, and no full-day result is claimed. The handoff remains `In progress` until required release gates pass.

## Editable standalone capture folder, 2026-10-18 03:41 +08:00

- Added a persisted `manualOutputFolder` setting, defaulting to `captures` for existing configurations. The browser includes an editable path, native Browse, Save folder, saved/unsaved feedback, and the resolved saved destination. Capture now and Open captures folder save pending edits first. Tray capture/open actions use the same preference; accepted requests retain their destination even if the preference changes later.
- Clippy and all 27 Rust tests passed (26 core tests and one configuration-transfer test). New tests cover backward-compatible defaults, invalid/stale save rejection, relative paths after moving the application, absolute paths and restart persistence.
- Expanded bilingual browser suite passed against both the test host and packaged EXE: typed paths survive polling, picker cancellation preserves edits, selected paths persist on reload, capture/open save the visible destination, and a rejected save prevents capture while retaining the draft. English/zh-Hant desktop and mobile screenshots reviewed.
- Packaged native smoke passed, including real JPEG captures to relative Unicode and absolute destinations, restart persistence, and unchanged task settings/progress. Report: `artifacts/portable smoke 截圖 08f370d8 moved/smoke-results.json`.
- ZIP contains only the byte-identical EXE. JavaScript/Python syntax and diff checks passed. Clean-OS/device checks and the completed 24-hour soak remain pending.

## Configurable capture shortcut, 2026-10-18 03:46 +08:00

- Added selectable modifier/key controls, enable/disable and Save shortcut, with English/zh-Hant text and registration status. Default Ctrl+Shift+S works globally while the app runs and uses the saved manual output folder. The browser and native menu display the active shortcut. Changes apply immediately; conflicts are reported without taking over another registration; repeat keydown events do not produce repeated captures.
- `cargo clippy --locked --all-targets -- -D warnings` and `cargo test --locked --all-targets` passed (28 tests). New core coverage verifies old-settings defaults, allowed/invalid combinations, rejected edits and restart persistence.
- `node scripts/browser-test.cjs` passed, including editing/saving/reloading shortcut preferences, retaining a draft across polling, visible registration conflicts and disabling the shortcut. English/zh-Hant desktop/mobile screenshots reviewed.
- `python scripts/hotkey-smoke.py --exe artifacts/release-shortcut/ScheduledScreenshot.exe` passed actual Windows SendInput tests without a browser: global capture while paused with zero tasks, saved Unicode destination, held-key repeat suppression, competing registration, live reassignment, simulated unavailable-desktop rejection, restart persistence, disable and exit cleanup. Report: `artifacts/hotkey-smoke-11f5c778/hotkey-results.json`.
- `python scripts/smoke.py --browser --exe artifacts/release-shortcut/ScheduledScreenshot.exe` passed the complete packaged regression suite, including session-wide single-instance behavior. Report: `artifacts/portable smoke 截圖 fb8d6f98 moved/smoke-results.json`.
- JavaScript/Python syntax and diff checks passed; ZIP contains exactly the byte-identical EXE. The full 24-hour soak and clean-OS/physical-device qualification remain pending.

## Tailwind UI refresh, 2026-10-18 03:53 +08:00

- Rebuilt the embedded static interface using Tailwind CSS 4.3.3, with a 16px base type size, task-focused page layout, compact capture bar, expandable folder/shortcut settings, aligned task status pills, clearer editor tabs and a contrasting filename preview. Capture settings remember their disclosure state within the browser session and show unsaved/conflict indicators when collapsed. English/zh-Hant and existing task behavior are preserved.
- `npm run build:css` and `npm run check:css` passed. The 42,677-byte compiled stylesheet is committed and embedded; cargo-only builds and end users require no Node.js. CI uses pinned npm tools and verifies generated CSS freshness. Tailwind's MIT license is embedded in the third-party notices.
- The first npm audit reported four transitive watcher-related vulnerabilities. Pinning `@parcel/watcher` to 2.6.0 resolved them; final `npm audit --audit-level=high` reports zero vulnerabilities. JavaScript/Python syntax and diff checks passed.
- Rust formatting, Clippy and all 28 Rust tests passed. The expanded pinned-Playwright browser suite passed on the test host and packaged EXE: compiled stylesheet serving, no remote assets, keyboard disclosure, saved disclosure state, collapsed unsaved indicators, all existing task/config/folder/shortcut flows, bilingual rendering and 320px layout/visible Save checks. Desktop, expanded settings, mobile and editor screenshots visually reviewed.
- `python scripts/smoke.py --browser --exe artifacts/release-tailwind/ScheduledScreenshot.exe` passed the packaged capture/scheduling/picker/migration/single-instance regression suite. Report: `artifacts/portable smoke 截圖 8bff352a moved/smoke-results.json`.
- ZIP contains exactly the byte-identical executable. Required full-day soak and clean-OS/physical-device qualification remain pending.
