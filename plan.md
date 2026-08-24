# Implementation Plan: Multi-Task Scheduled Screenshot Tool

> Implementation status: the application, offline editor, unit tests, Docker cross-build, and Windows CI packaging workflow are present. Docker compilation succeeds with zero warnings and errors. Interactive Windows 10 capture, session-event, multi-monitor, and resource acceptance tests remain Windows-only verification work.

## 1. Product definition

Create a portable Windows 10 notification-area application that runs up to 100 independent screenshot tasks. A task owns its schedule, output settings, and stop condition. The application captures all connected monitors and performs no remote communication.

### Success criteria

- Multiple interval and fixed-time tasks can run independently.
- The minimum recurring interval is 1 second.
- Each task may run indefinitely, stop at an exact time, or stop after a duration.
- Identical captures due together are coalesced.
- Slow captures never overlap or create an unbounded queue.
- Valid configuration edits are applied without restarting the process.
- Idle usage remains below 0.05% average CPU and 40 MB private working set on the compatibility target.

### Non-goals

- Hidden monitoring, Windows services, or lock-screen capture
- Remote configuration or a local HTTP server
- Cloud storage, synchronization, telemetry, or automatic deletion
- Installer, automatic startup registration, or administrator-only setup
- Overnight active-hour windows in version 1

## 2. Technical architecture

### Application host

- Target .NET Framework 4.8 and Windows 10 22H2 x64.
- Use a windowless WinForms `ApplicationContext` and `NotifyIcon`.
- Enforce a single process with a named mutex.
- Keep the portable executable, `settings.json`, `config-editor.html`, `runtime-state.json`, and capped logs in the extracted application directory.
- Fail visibly if that directory is not writable; do not introduce a second implicit configuration location.

### Core components

| Component | Responsibility |
| --- | --- |
| Application host | Process lifecycle, tray menu, single-instance enforcement |
| Configuration service | Read, validate, watch, and atomically update `settings.json` |
| Scheduler | Calculate the earliest next occurrence across all tasks |
| Runtime-state store | Persist duration deadlines and last-fired occurrences by task ID |
| Session monitor | React to lock, unlock, suspend, resume, and clock changes |
| Capture coordinator | Group due tasks, serialize capture profiles, and skip overruns |
| Monitor capture service | Enumerate and capture physical monitor bounds with Win32 GDI |
| Image writer | Encode JPEG/PNG and atomically move completed files into place |
| Diagnostic logger | Record structured lifecycle, scheduler, capture, and error events |
| Static editor | Create and validate task configuration without a server |

### Resource model

- Maintain one scheduler timer, not one timer per task.
- Set the timer directly for the earliest future occurrence.
- Recalculate only after a due event, configuration change, session/power event, clock change, or task completion.
- Process monitors sequentially and dispose of each bitmap immediately after encoding.
- Keep logging event-driven and capped; do not write periodic health messages.

## 3. Configuration and state

### Settings model

Top-level settings contain:

- `schemaVersion`: initially `1`.
- `paused`: global pause state.
- `logging`: severity level, relative directory, rotation size, and retained-file count.
- `tasks`: zero to 100 task objects.

Every task contains:

- Stable GUID `id`.
- Unique case-insensitive `name`, 1-64 characters.
- Boolean `enabled`.
- `capture` settings: output folder, filename template, `jpeg` or `png`, JPEG quality 50-100, and cursor inclusion.
- Exactly one `schedule` type:
  - `interval`: 1-86,400 seconds, weekdays, and optional same-day active hours.
  - `fixed`: weekdays and one or more unique local `HH:mm:ss` times.
- One `stopCondition`:
  - `none`.
  - `at`, with a future local ISO date/time.
  - `duration`, with 1-31,536,000 seconds.

Use weekday tokens `Mon`, `Tue`, `Wed`, `Thu`, `Fri`, `Sat`, and `Sun`. Expand environment variables in output paths before validating and comparing them.

The per-task `fileNameTemplate` defaults to `{timestamp}_{display}`. The application appends the selected image extension. Support these tokens:

| Token | Expanded value |
| --- | --- |
| `{timestamp}` | Batch timestamp as `yyyyMMdd_HHmmss_fff` |
| `{date}` | Local batch date as `yyyyMMdd` |
| `{time}` | Local batch time as `HHmmss` |
| `{task}` | Sanitized task name |
| `{taskId}` | Full task GUID |
| `{taskId8}` | First eight GUID characters |
| `{display}` | Sanitized Windows display name |
| `{displayIndex}` | One-based display number |

Require `{display}` or `{displayIndex}` so a multi-monitor capture cannot map every image to the same intended name. Permit timestamp-free templates, but never overwrite: atomically append `_001`, `_002`, and so on when the expanded destination already exists.

Logging defaults are:

```json
{
  "level": "info",
  "directory": "logs",
  "maxFileSizeMb": 5,
  "retainedFiles": 5
}
```

Accept levels `error`, `info`, and `debug`, rotation sizes from 1-100 MB, and retention counts from 1-20. Require the log directory to be a relative child path inside the portable application directory; reject rooted paths and traversal segments.

### Validation and reload

- Reject duplicate task IDs or names, invalid enum values, invalid time/range values, missing required properties, unwritable output destinations, and more than 100 tasks.
- Reject filename templates over 180 characters, unknown or malformed tokens, literal path separators, invalid Windows filename characters, missing display tokens, or an expanded full path over 240 characters.
- Sanitize values inserted by tokens, trim trailing spaces/dots, and prefix Windows reserved device names rather than allowing token values to create invalid filenames.
- Reject unsupported log levels, out-of-range rotation settings, rooted log paths, or log paths that escape the portable directory.
- Tolerate unknown properties for forward compatibility.
- Watch `settings.json` and debounce changes for 500 ms to avoid parsing intermediate editor writes.
- Parse and validate a complete candidate before swapping the active immutable configuration snapshot.
- Preserve the last valid in-memory snapshot if a live edit is invalid and notify the user through the tray.
- If startup configuration is invalid, load no active tasks until it is corrected.
- Write application-originated changes to a temporary file in the same directory, flush it, and atomically replace the destination.

### Runtime state

- Key state by task GUID, never by name or array position.
- Persist the calculated UTC deadline for duration tasks so process restarts cannot extend them.
- Persist the last-fired fixed occurrence key to prevent duplicate execution after a restart or repeated local clock hour.
- Clear state when a task is deleted or disabled.
- A duration begins on a disabled-to-enabled transition. Editing the duration restarts its countdown; unrelated edits retain its existing deadline.
- Global pause suppresses capture but does not extend deadlines.
- At a deadline, stop new work for that task and allow an existing capture to complete. Then disable only that task using a compare-and-retry update against the latest valid settings file, preserving unrelated user edits.

## 4. Scheduling and capture behavior

### Occurrence calculation

- Calculate each enabled task's next valid occurrence after the current instant, then arm the single timer for the earliest result.
- Interval tasks retain their scheduled cadence; capture duration does not move the cadence anchor.
- Fixed times use the Windows local timezone and run at most once per local date/time key.
- Stop conditions take precedence over an occurrence at the same timestamp.
- Skip occurrences missed during process shutdown, sleep, lock, or a forward clock jump.
- After resume, unlock, or a clock change, discard past occurrences and calculate the next future one.

### Due batches and collisions

- Take a snapshot of all tasks due in the current scheduler second.
- Normalize each capture profile using the expanded canonical output path, effective filename template, image format, JPEG quality, and cursor setting.
- Group tasks with identical profiles and perform one physical capture set for the group.
- Process distinct groups sequentially in their first task's configuration order.
- Mark every task in a successful coalesced group as completed for that occurrence.
- Isolate failures to their capture group and continue with other groups from the same due batch.
- While a batch is active, skip any newly due occurrences. Never overlap captures, retain an unbounded backlog, or run an immediate catch-up capture.

### Monitor capture and files

- Declare Per-Monitor-V2 DPI awareness before creating UI handles.
- Enumerate every connected display and use its physical pixel bounds, including negative virtual-desktop coordinates.
- Capture displays sequentially through Win32 GDI.
- If enabled, render the current cursor only into the monitor containing its position.
- Give every due batch a common timestamp, expand the task's filename template for each display, and write under the existing date directory. The default resolves as:

```text
yyyy-MM-dd\<timestamp>_<display>.<ext>
```

- Encode JPEG at the configured quality or lossless PNG.
- Append the selected format extension automatically; templates do not control the extension.
- Reserve the destination with create-new semantics. If it exists, append the next available three-digit collision suffix before the extension.
- Write to a temporary file in the destination directory and atomically rename after a successful encode without replacing an existing screenshot.
- Treat each monitor result independently so one disappearing display does not discard successful images from other displays.
- Never attempt capture while the workstation is locked, on a secure desktop, or in a disconnected user session.

### Diagnostic logging

- Write UTF-8 JSON Lines to `screen-capture-YYYYMMDD-NNN.jsonl`, rotating at the configured size or local date boundary and retaining only the newest configured number of files.
- Use a stable entry contract containing `timestampUtc`, `level`, `eventId`, `message`, and optional `taskId`, `taskName`, `batchId`, `display`, `filePath`, `durationMs`, `exceptionType`, `hResult`, `stackTrace`, and structured `context`.
- Define stable event IDs for application start/exit, configuration accepted/rejected, task due/skipped/ended, batch start/complete, monitor capture success/failure, filename collision, session lock/unlock, suspend/resume, clock change, and log rotation.
- At `info`, record lifecycle and state changes, one summary per capture batch, skips, task endings, and every warning/error. Do not log every successful monitor capture.
- At `debug`, additionally record next-occurrence calculations, task grouping/coalescing, and per-monitor capture and output details.
- Include errors at every configured level. Record exception type, HRESULT, message, and stack trace without including image data or serialized settings.
- Keep one log stream open, serialize writes through a lock, and flush after errors, completed batches, configuration changes, and orderly exit. Do not add a periodic flush or logging thread.
- If logging fails, rate-limit a tray warning, disable further writes to that file until the next rotation/reopen attempt, and continue scheduling and capture without recursive logging.

## 5. User experience

### Tray interface

- Show global pause state, the next task, and its next occurrence.
- Provide Capture Now with an enabled-task selector.
- Provide Pause/Resume All, Configure in Browser, Edit JSON in Notepad, Open Output Folder by task, Open Logs, and Exit.
- Surface configuration, disk, and capture errors without sending a notification for every successful capture.

### Static configuration editor

- Distribute a framework-free `config-editor.html` with no remote assets or requests.
- Provide a searchable task list with add, duplicate, rename, reorder, enable/disable, and delete actions.
- Give duplicated tasks a new GUID.
- Organize task fields into Schedule, End condition, Image, and Storage sections, with a separate application-level Logging section.
- Let the Logging section select `error`, `info`, or `debug`, rotation size, and retention count, and warn that debug mode is verbose for one-second tasks.
- Provide a filename-template field with a token picker, inline validation, and a live example for the selected task and a sample display.
- Accept interval input in seconds, minutes, or hours and serialize it as seconds.
- Validate locally before save, while retaining application-side validation as authoritative.
- Use the browser File System Access API for explicit in-place saves when available; otherwise download a replacement `settings.json`.
- Keep Notepad editing as the universal fallback.
- Warn that a one-second task creates 86,400 images per monitor per day and summarize aggregate configured frequency.
- Keep the editor below 50 KB uncompressed and support keyboard navigation, narrow windows, visible focus, and Windows high-contrast mode.

## 6. Implementation phases

1. **Project foundation**
   - Create the WinForms application host, tray lifecycle, single-instance guard, DPI declaration, structured rotating logger, and portable-directory checks.
2. **Configuration**
   - Implement DTOs, schema validation, atomic persistence, file watching, immutable active snapshots, and runtime-state storage.
3. **Scheduler**
   - Implement interval/fixed occurrence calculations, one-shot timer management, deadlines, session/power handling, pause behavior, and overrun skipping.
4. **Capture pipeline**
   - Implement monitor enumeration, GDI capture, optional cursor rendering, image encoding, atomic output, grouping, and error isolation.
5. **User interfaces**
   - Complete tray commands and build the offline static editor with both in-place and download save paths.
6. **Hardening and packaging**
   - Complete automated tests, Windows hardware tests, resource profiling, soak testing, documentation, release ZIP creation, and checksums.

## 7. Test and acceptance plan

### Automated tests

- Interval calculations at 1, 2, 59, 60, and 86,400 seconds.
- Fixed occurrences across weekdays, repeated clock hours, forward jumps, and process restart.
- Earliest-occurrence selection with 1, 10, and 100 tasks.
- Independent exact and duration end conditions, including an end coinciding with a due event.
- Duration persistence, duration edits, global pause, task disable/re-enable, deletion, and corrupt runtime state.
- Configuration defaults, unknown properties, duplicates, malformed/partial writes, atomic updates, and watcher debouncing.
- Profile normalization and coalescing for identical and different output settings, including filename templates.
- Capture overruns proving that later occurrences are skipped without concurrency or queue growth.
- Filename token expansion, sanitization, reserved names, missing display tokens, collision suffixes, path-length limits, monitor-level error isolation, and atomic image writes.
- Log-level filtering, JSON Lines fields, stable event IDs, daily/size rotation, retention cleanup, flush behavior, and log-write failure isolation.

### Windows integration tests

- Windows 10 22H2 x64 with single and multiple monitors.
- Mixed DPI scaling, negative monitor coordinates, and monitor hot-plugging.
- JPEG/PNG output, quality settings, and cursor placement.
- Workstation lock/unlock, sleep/resume, clock changes, and disconnected sessions.
- Unwritable paths, disk-full errors, a disappearing monitor, duplicate application launches, and rapid manual capture.
- Static editor behavior in Edge plus the download fallback in a browser without direct file-save support.
- Log collection across application restart, capture failures, configuration errors, lock/resume events, rotation, and an unwritable log directory.

### Performance and soak tests

- Measure idle CPU for ten minutes with the external editor closed; acceptance is below 0.05% average.
- Measure idle private working set; acceptance is at most 40 MB.
- Run 100 configured but infrequent tasks and confirm idle targets remain satisfied.
- Run sustained one-second multi-monitor tasks and monitor timing, CPU, memory, handles, file counts, disk errors, and coalescing.
- Repeat the one-second soak with `debug` logging enabled and confirm rotation remains bounded and logging does not block capture scheduling.
- Confirm only one monitor bitmap and one capture profile are active at a time.

### Release acceptance

- All valid tasks run independently and stop according to their own condition.
- Simultaneous identical tasks produce one shared physical capture set.
- No capture occurs on locked or secure desktops.
- Invalid live configuration never replaces the active valid snapshot.
- Diagnostic logs collect configured events and errors, remain within retention limits, and cannot terminate capture processing.
- No HTTP listener, telemetry, remote traffic, installer, or administrator permission is required.
- Windows CI produces a versioned portable ZIP and SHA-256 checksum.
