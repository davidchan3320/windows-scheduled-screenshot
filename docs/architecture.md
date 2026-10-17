# Architecture

`src/config.rs` defines strict schema-v2 settings and validates typed edits. `schedule.rs` contains pure timezone-parameterized occurrence calculations. `storage.rs` owns portable paths, migration, atomic file replacement and four bounded 1 MiB log files. `output.rs` expands filename tokens, reserves collision-free files using `CREATE_NEW`, and encodes JPEG/PNG.

Schema 2 also accepts a `language` preference (`en` or `zh-Hant`), defaulting to `en` when absent. It is saved through the existing revision-checked settings API. `ui/i18n.js` loads the two embedded JSON packs, binds static text/accessible labels without replacing controls, and formats dynamic text and dates. User content and persisted schedule values are never translated. `src/localization.rs` shares the Traditional Chinese pack with native tray labels and the picker title. Unknown diagnostic strings retain the original details under a localized explanation.

`engine.rs` owns one scheduler thread waiting on its event channel until the next deadline. It arms occurrences once and sends a bounded batch to one sequential capture worker. A busy worker causes new recurring occurrences to be skipped. Interval anchors remain independent of encoding time. Runtime progress is persisted before dispatching one-time attempts, so interrupted one-time captures cannot retry after restart. Completion checks task generations; edits invalidate stale work. Session/display epochs invalidate captures spanning a desktop change. A duration/deadline is exclusive: an occurrence at the deadline does not run.

`platform.rs` contains the Win32 boundaries: session-wide named mutex shared by all portable copies, tray message loop, session/power/time/display notifications, COM folder picker and Startup shortcut, and GDI capture. Desktop availability requires both an active Windows session and the input desktop named `Default`. Capture checks availability before and after each monitor, and after saving the batch. A batch counts only after every image is saved; failed batches remove their own partial output. GDI, icon, desktop, handle, and COM ownership has deterministic cleanup.

`http.rs` binds `127.0.0.1:0` directly, avoiding a probe/bind port race. Four fixed request workers and an eight-connection queue bound resource use. Headers are limited to 16 KiB, bodies to 1 MiB, and reads have deadlines. APIs require an exact Host, matching Origin, and a random per-launch token; cross-site fetches, duplicate headers, chunked transfer and CORS are rejected. UI assets and third-party notices are compiled into the EXE. A restrictive content-security policy disallows inline scripts, framing and external assets. The launch URL places the token in the fragment; the UI moves it into tab session storage and removes the fragment from browser history. Tokens and images are never logged.

All APIs use POST with `Content-Type: application/json` and `X-App-Token`:

| Path | Request | Result |
|---|---|---|
| `/api/status`, `/api/config` | `{}` | Settings, per-task progress/next execution, availability, errors, default task, manual capture running/result/output folder |
| `/api/save` | `{settings, revision}` | Updated snapshot; HTTP 409 for stale revision |
| `/api/export` | `{}` | Saved schema-v2 settings only, without runtime state or session credentials |
| `/api/validate-import` | `{settings}` | Validated settings with scheduling paused; no mutation |
| `/api/import` | `{settings, revision}` | Replaces settings through the normal save path with scheduling paused; HTTP 409 for stale destination revision |
| `/api/pause` | `{paused, revision}` | Updated snapshot |
| `/api/capture` | `{}` | Standalone capture using app defaults; accepts no task ID; busy/unavailable requests rejected |
| `/api/open-captures` | `{}` | Opens/creates the saved standalone capture folder |
| `/api/preview` | `{task}` | Expanded full path for display 1 |
| `/api/folder` | `{}` | Selected path, or `null` on cancellation |
| `/api/open-folder` | `{id}` | Opens the saved task's output root in File Explorer, creating it if missing; rejects unknown task IDs |

No external filesystem edits are watched: edit through the manager, or exit before editing JSON and restart. Corrupt settings recover the last valid copy with scheduling paused. Corrupt runtime progress stops startup instead of silently resetting capture limits. Save failures preserve the in-memory configuration and surface an error; runtime persistence failures pause scheduling. Individual files are atomically replaced with flush/write-through semantics. Configuration and runtime are separate files, not a cross-file database transaction.

Manual captures use the same bounded, sequential capture worker as schedules, with a transient default capture configuration and the persisted `manualOutputFolder` preference (default `captures`). The folder is copied into the request when accepted, so subsequent preference edits do not redirect an in-flight capture. Browser and tray capture/open actions share this preference. Relative paths resolve from the executable directory; absolute paths are retained. They do not look up a saved task and never add an entry to runtime task progress. Pause, task edits and task deletion do not invalidate them; desktop/session changes still do. The latest manual result/files/time are kept in application memory and bounded logs; screenshot files remain after restart. The tray and browser call the same standalone operation.

The normal entry point uses the Windows GUI subsystem. `--background` suppresses browser launch, as used by the optional Startup shortcut. `--print-url` writes the local control URL to redirected stdout for tests; treat that output as a session secret. The test host in `examples/` has no capture access and is not distributed.

Global capture shortcuts use the persisted `captureShortcut` preference (`enabled`, `modifiers`, `key`; default Ctrl+Shift+S). The tray thread registers the selected chord with Windows `RegisterHotKey` and `MOD_NOREPEAT`; `WM_HOTKEY` invokes the same standalone capture worker. Settings saves post a refresh to the window thread, so reassignment needs no restart. The old registration is released before the new one is applied; failures appear separately as `shortcutError`, and `shortcutActive` is only populated after successful registration. Disable/exit unregister the key. Browser drafts do not affect shortcut capture until saved. Registration semantics follow [Microsoft's RegisterHotKey documentation](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey).

The UI uses [Tailwind's build-time CLI](https://tailwindcss.com/docs/installation/tailwind-cli), pinned in the npm lockfile. `ui/tailwind.css` defines theme tokens, accessible component recipes and responsive rules; explicit sources restrict class scanning to the bundled HTML/JS. `npm run build:css` produces the committed `ui/style.css`, which Rust embeds unchanged. `npm run check:css` compiles independently and rejects stale output in CI. No stylesheet compiler, CDN or browser runtime dependency ships to users. The Tailwind MIT license is included in the embedded third-party notices. The capture settings disclosure remembers its open state within the browser session; unsaved edits and shortcut failures remain indicated when collapsed.
