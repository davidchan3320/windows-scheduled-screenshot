# Portable Windows Screenshot Scheduler

- **Status:** In progress
- **Created:** 2026-10-18 02:01 +08:00
- **Last updated:** 2026-10-18 03:53 +08:00
- **Supersedes:** None; implements the user-supplied handoff.
- **Current blocker:** Required release qualification is pending: the soak exited before completion; clean Windows VMs and the physical device/session matrix are not available in this session.
- **Next action:** Investigate the interrupted soak, rerun the 24-hour check on the latest package, then run the outstanding clean-OS and physical-device checks in docs/verification.md.

## Goal
Deliver one portable `ScheduledScreenshot.exe` for Windows 10/11 x64. End users need no installer, admin rights, .NET, Node, Rust, WebView, or asset downloads. A bundled local browser task manager opens in the default browser; scheduling continues when it closes.

## Implementation Changes
- Embed HTML/CSS/JS/icons/defaults. Statically link the MSVC runtime; use supported Windows system libraries. Distribute EXE, optional ZIP, and SHA-256.
- Keep settings, runtime state and bounded logs in `data` beside the executable. Report unwritable folders with instructions to move the application. Default captures to `captures`; resolve relative paths against the executable, preserve absolute paths.
- No startup/registry changes by default. Optional per-user Startup shortcut, with moving instructions. Tray actions: manager, pause/resume, capture, output/logs, exit.
- Browser task list: enabled, schedule, next run, last result, output. Create/edit/duplicate/delete/enable/capture now. Editor groups: When, Save to, Filename; native folder picker, date subfolders, tokens, format, full-path preview. Segoe UI and keyboard focus, no remote assets.
- Schedules: dated local once, interval, selected weekday times. Preserve JPEG/PNG, quality, cursor, per-monitor images, active hours, stop deadline/duration/success count. New tasks disabled, JPEG 85, `{timestamp}_{display}`, date subfolders.
- One scheduler timer and sequential capture worker, bounded work, deterministic GDI cleanup. Skip missed recurring runs; cadence independent of capture duration. Once tasks completed/failed/missed without retries. Recalculate on clock/timezone changes; reject nonexistent local once times, skip recurring gaps, execute DST folds once. No locked/secure/disconnected captures. Success only after all monitor files save.
- Atomic configuration/runtime persistence, last valid settings after rejected edits, collision suffixes without overwrite, Windows filename validation, automatic extensions.
- Schema v2: dated once, folder layout, startup. Back up/migrate legacy settings/runtime preserving IDs, fixedOnce behavior, and resolved output paths. Account for all existing uncommitted fixes before removal.
- Replace CI with Rust checks/portable packaging. Remove C#/macOS and obsolete tooling only after parity verification.

## Interfaces and Types
Separate configuration, scheduling, capture/output, Windows integration, HTTP/UI. Ephemeral loopback server with per-launch token, Host/Origin validation, limits, no CORS. APIs: config read/save with revision conflicts, status, preview, folder picker, capture, pause/resume.

## Tests
- Automated: schedules, DST, missed runs, stops, filename collisions, migration, atomic saves, stale edits.
- Browser: task CRUD, picker cancellation, previews, close/reopen.
- Windows: monitors/DPI, lock/resume, display changes, failures, Startup shortcut, single instance; spaces/non-ASCII paths, portable moves, relative paths, unwritable folder.
- Clean Windows 10 22H2 and Windows 11 x64 without developer runtimes.
- 24-hour soak; measure idle private working set target <=40 MB and average CPU <0.05% with browser closed.

## Assumptions
Signed-in session only. No service, wake, updates or installation. Single EXE refers to the distributed file; runtime data and captures are created during use. No external clean VMs or completed 24-hour soak are assumed available.

## Milestones
- [x] Audit and preserve uncommitted legacy fixes.
- [x] Rust core, migration, persistence and regression tests.
- [x] Windows capture, tray, notifications, picker and startup integration.
- [x] Embedded browser task manager and protected APIs.
- [x] Release build and automated/browser/native verification on the current machine.
- [x] Remove legacy implementations/tooling after parity verification.
- [x] Improve readability and task editing with tabbed sections, live summary, readable units and responsive layouts; verify and package the UI update.
- [x] Add embedded zh-Hant localization, portable preference, bilingual browser and native verification.
- [x] Make Capture now independent of tasks in browser/tray, with default output and separate result; verify empty-list/paused capture and unchanged task progress.
- [ ] Clean Windows matrix and 24-hour soak measurements.

## Progress Log
- 2026-10-18 03:44 +08:00 - Added user-requested configurable global capture shortcut: default Ctrl+Shift+S, selectable modifiers/key, enable/disable, immediate registration refresh, conflict reporting, no key-repeat captures, and English/zh-Hant controls. Clippy, all 28 Rust tests and browser suite passed; native key injection/reassignment/conflict verification is pending.

- 2026-10-18 02:01 +08:00 — Saved supplied plan; implementation started. Found uncommitted fixes and tests in C#, Swift and browser editor; preserving them during parity work.
- 2026-10-18 02:23 +08:00 — Implemented Rust modules and embedded UI. Release built successfully. All 21 core tests, Clippy, browser smoke and native capture smoke passed on Windows 10 22H2. Resumed after interruption; final display-change cancellation hardening still needs rebuilding.
- 2026-10-18 02:37 +08:00 — Final hardening, 23 tests, browser-only CI harness, packaged native smoke, Unicode startup shortcut, dependency notices and Rust packaging/CI passed. Archived all legacy uncommitted work before retiring old active sources. Automatic deletion review rejected the recursive delete, so the sources were safely moved into artifacts/retired-legacy instead. Short resource measurement met idle targets. Started the required 24-hour soak on final package (driver PID 55828, report artifacts/soak-475adb7a/soak-results.json). Completion deferred until required release gates pass.
- 2026-10-18 02:41 +08:00 — Fixed the missing notices footer link and added a browser assertion for its endpoint/content. Rebuilt and reran packaged native/browser smoke successfully; final EXE SHA-256 is d0ceaf51886a523139726d8f1bc4f5eccd0e971e588f7787845b2c0ee56d8edf. Deliberately stopped the initial soak and restarted the 24-hour test at 02:40:05 against this exact EXE (driver PID 47576, application PID 52196, artifacts/soak-5cf684ef/soak-results.json). Required release validation remains pending.

- 2026-10-18 02:58 +08:00 — Completed the requested UI readability/editing improvement: three editor tabs, live summary, persistent Save footer, readable time units, weekday shortcuts, individual time controls, token insertion and responsive task rows. Expanded browser tests and packaged Windows native smoke passed. Packaged artifacts/release-ui separately because the earlier EXE is running. Preserved the ongoing soak on the unchanged Rust engine; full release qualification remains pending.

- 2026-10-18 03:04 +08:00 - Changed Add time to the current local clock by default; browser verification and release packaging passed. Found the earlier soak had exited before completion and recorded the remaining release gate.

- 2026-10-18 03:09 +08:00 - Added task-specific Open folder buttons, verified native Explorer behavior and failure handling, and packaged artifacts/release-folder. EXE SHA-256: aa221af5c41896dab60b205b1ca6663c5495eab787b3226436f75125f8280e18. Full release qualification remains pending.

- 2026-10-18 03:14 +08:00 - Aligned task enable/disable status beside the task name; browser checks and visual review passed. Rebuilt artifacts/release. SHA-256: 504868856d9cb21abf7e09c1284ffc8fbc01ad13065d391476b26964758b4203.

- 2026-10-18 03:22 +08:00 - Completed English/zh-Hant localization with persisted preferences. Bilingual browser checks, 25 core tests, actual native menu/picker/restart checks and packaged smoke passed. Package: artifacts/release-zh-Hant; SHA-256 608f56454e763503aec3f0d168289069222ba1b71702fc53b0cffbdc6909efb7. Full release qualification remains pending.

- 2026-10-18 03:29 +08:00 - Updated the fixed-time option wording in both language packs. Language-pack verification and release packaging passed. Latest EXE: artifacts/release; SHA-256 605994fb3a24252303b946d6bb87c7d7a3ac946d43128d3c59d3c0733db00b45.

- 2026-10-18 03:34 +08:00 - Completed standalone Capture now. Bilingual browser, 25 core tests, packaged native regression suite and actual tray-menu capture passed. The initial tray keyboard simulation did not activate the command; native item-click verification passed. Package: artifacts/release-standalone; SHA-256 52592cf55777418a630ab13f5f0b0580307d9337d851fd785977695da67a8c1b. Full release qualification remains pending.

- 2026-10-18 03:46 +08:00 - Completed configurable global capture shortcut. Clippy, all 28 Rust tests, bilingual browser checks, actual native key injection/conflict/reassignment/repeat/cleanup tests and packaged browser/native regression suite passed. Package: artifacts/release-shortcut; SHA-256 caa6d5a34bfd989d485904b6e81f70bc46fbb12be5af0b1eeb82e15c73e692e8. Full release qualification remains pending.

- 2026-10-18 03:51 +08:00 - Implementing the requested Tailwind UI refresh: compiled offline styles, task-focused layout, expandable capture preferences, improved status controls and editor preview. Pinned npm tooling, generated-CSS checks and CI integration added. Initial bilingual browser suite and visual review passed; final typography, packaging and verification in progress.

- 2026-10-18 03:53 +08:00 - Completed Tailwind UI refresh with compiled offline CSS, pinned development tools, CI freshness checks, embedded license, task-focused layout and responsive editor. All 28 Rust tests, Clippy, bilingual/320px browser checks, visual review and packaged native regression suite passed. npm audit reports zero vulnerabilities after updating the watcher dependency. Package: artifacts/release-tailwind; SHA-256 b01c0f2f291b6f8743b08ff8d873df6d77cb114eba8ec1e589fc43a9f017a9f3. Full release qualification remains pending.

## Verification Results
- 2026-10-18 03:39 +08:00: Added an editable, remembered standalone capture folder with native Browse, explicit Save folder, and save-before-capture/open behavior. Browser and tray use the preference; pending captures retain their accepted destination. Drafts survive polling and failed saves. English/zh-Hant labels, 26 core tests plus configuration-transfer test, Clippy and expanded bilingual browser suite passed. Packaged native verification is in progress.

- 2026-10-18 03:33 +08:00: Changed Capture now to a global, task-independent operation in the browser/tray. Uses default JPEG capture settings/output, works without tasks and while paused, stores a separate latest result, and never updates task progress. Added native regression checks for empty task lists, output failure/recovery, saved-task settings independence and locked desktop rejection. Clippy, 25 existing core tests and expanded bilingual browser checks passed; packaged native verification is running.
- 2026-10-18 03:28 +08:00: Removed "each week" from the fixed-time schedule option, now "At specific times" / "於指定時間執行". Matching language-pack keys/placeholders test and diff checks passed; schedule behavior is unchanged.
- 2026-10-18 03:21 +08:00: Added embedded English/zh-Hant language packs, a portable language preference, localized browser content and native tray/picker labels. All 25 core tests passed, including matching pack keys/placeholders and language persistence/default/rejection. English/Traditional Chinese browser switching, fresh sessions, Chinese names, validation, unchanged tokens/times, and desktop/mobile visual review passed. Final packaged native verification is in progress.
- 2026-10-18 03:14 +08:00: Aligned enable/disable controls in a fixed-width column beside each task name. Output paths remain on their own line at all widths. Existing browser suite, JavaScript syntax and diff checks passed; desktop and 390px mobile screenshots visually verified.
- 2026-10-18 03:09 +08:00: Open folder button added beneath each task's output path. The authenticated endpoint accepts a saved task ID and shares folder opening with the tray. Formatting, Clippy, all 23 Rust tests and browser suite passed. Direct native verification checked a relative path with spaces/Unicode, automatic folder creation and the actual Explorer location; unknown tasks and paths pointing at files returned errors. The verification closed only its own Explorer window.
- 2026-10-18 03:04 +08:00: Add time now defaults to the local clock at click time, including seconds. JavaScript syntax check, existing browser suite and a direct browser comparison against the local clock passed. Rebuilt artifacts/release/ScheduledScreenshot.exe; SHA-256 a99e0174615bf24542006718a008a42a6c45e14df1fdcd34c43ebe2f04907f5b.
- 2026-10-18 02:58 +08:00 UI update: JavaScript syntax checks, browser suite, responsive screenshot review, release build, ZIP identity check and packaged native/browser smoke passed. Expanded browser coverage includes units, time controls, keyboard tabs, validation, discard protection and always-visible Save controls. Initial test locator failure was fixed. Default packaging was blocked by the running old EXE; the new optional output directory allowed safe packaging under artifacts/release-ui.
- `cargo test --all-targets`: 21 passed. An initial test-link failure from applying GUI subsystem flags to the test harness was fixed by keeping GUI subsystem selection on the application binary.
- `cargo clippy --all-targets -- -D warnings`: passed after correcting two lint findings.
- `cargo build --release`: passed; first release was 1,003,008 bytes.
- `python scripts/smoke.py --browser`: passed browser CRUD/duplicate/delete, previews, stale-edit rejection, markup safety, browser close/reopen, native JPEG/PNG captures, count completion, independent output failures, dated once/fixedOnce, manual capture, simulated session/clock events, real folder-picker cancellation, and moving executable/data with non-ASCII paths.
- `dumpbin /dependents`: only Windows system DLL imports; no MSVC runtime DLL, .NET, Node or WebView dependency.
- Screenshots of desktop/mobile task manager and editor visually inspected.
- Final `cargo test --locked --all-targets`: 23 passed. Final formatting, Clippy, JavaScript/Python syntax checks passed.
- Final `node scripts/browser-test.cjs` and packaged `python scripts/smoke.py --browser --exe artifacts/release/ScheduledScreenshot.exe`: passed. An intermediate shortcut verification failed because WScript.Shell converted Unicode to the ANSI code page; direct IShellLinkW verification passed without changing the valid shortcut implementation.
- Final EXE 1,199,616 bytes; ZIP contains only that EXE. SHA-256 recorded in artifacts/release/SHA256SUMS.txt and docs/verification.md. Only Windows system DLL imports.
- `python scripts/soak.py --minutes 2 --interval 5`: short diagnostic passed; 60-second idle CPU 0.00%, private working set 1.30 MiB. Full 24-hour soak started 2026-10-18 02:36:15 +08:00 and is pending.

## Unresolved Issues
- At 03:04, the earlier soak was no longer running. Its report records complete=false, 19 captures and approximately 1,157 seconds; stderr reports "Scheduler exited unexpectedly". Cause not established; no full-day pass is claimed.
- 2026-10-18 UI package SHA-256 is 60c4cc06e47f7d886dd31a51865be514b7cb20e2da33d677cb6f5914ad7b29fe. The running soak remains on the prior binary with unchanged Rust scheduler/capture code; it does not qualify this exact new package for a full-day result.
- Clean Windows 10/11 environments, physical lock/sleep/display/DPI combinations and real ACL/disk-full cases remain unrun. Current machine is Windows 10 Pro 22H2 build 19045 with developer software; it is not a clean runtime test. The running 24-hour soak is not yet a passed verification. Full details and continuation instructions are in docs/verification.md.

- 2026-10-18 03:41 +08:00 - Completed editable standalone capture folder. Clippy, all 27 Rust tests, bilingual browser checks and packaged native regression suite passed. Relative Unicode/absolute capture destinations, restart persistence and task-progress independence verified; desktop/mobile reviewed. Package: artifacts/release-save-folder; SHA-256 b7f2e123a5e572d075bd5841db9c90f25808d3591a50e7c753b77dac5da0ee0c. Full release qualification remains pending.
