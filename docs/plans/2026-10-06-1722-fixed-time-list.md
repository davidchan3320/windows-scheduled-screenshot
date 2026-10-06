# Execute a list of clock times once

- **Status:** Completed
- **Created:** 2026-10-06 17:22 +08:00
- **Last updated:** 2026-10-06 17:22 +0800
- **Planning model:** Unknown
- **Supersedes:** [Capture count stop mode](2026-10-06-capture-count.md)
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete.

## Goal

Allow one task to contain many specific clock times, execute each time once, and automatically disable after the list is complete. Retain the capture-count stop mode implemented before the user clarified the requested schedule behavior.

## Implementation Changes

- Add `Fixed local times — run once` using `schedule.type = fixedOnce` and the existing clock-times and weekday inputs, with no separate limit required. Each listed time uses its next eligible occurrence. Past times run on the next selected weekday.
- Persist `completedFixedTimes`, exclude those slots from future occurrences, and disable the task when every slot is consumed. A failed execution consumes its slot; a busy or unavailable session leaves it pending. Cosmetic changes preserve progress; schedule edits or disable/re-enable reset the list.
- Protect asynchronous completion and task disabling against changed schedules, capture limits, and task generations. Preserve existing Windows duration signatures when introducing the optional count field.

- Add an `After a number of captures` stop mode and whole-number input to the existing offline editor; preserve import/export and reject missing, fractional, zero, negative and out-of-range counts.
- Extend Windows models, validation and scheduler, and the equivalent Swift configuration and scheduler.
- Count one successful scheduled execution per task, irrespective of display count or shared capture profile. Manual captures, capture failures and busy/session skips do not consume the quota.
- Persist progress across restart, pause, and cosmetic or schedule edits. Reset when the count limit changes or the task is disabled and enabled again.
- Persist completion before disabling tasks; recover an exhausted enabled task after restart. Ignore results for a task whose configuration changed during an asynchronous capture.
- Document the configuration contract and usage in `docs/capture-count.md`.

## Interfaces and Types

- `schedule.type`: add `fixedOnce`, validated as fixed local times with unique `HH:mm:ss` values and selected weekdays.
- Runtime `completedFixedTimes`: a list of consumed clock-time slots.
- Swift `CaptureBatchResult`: files, successful task IDs, and busy skip indication.

- `stopCondition.mode`: add `count`.
- `stopCondition.captureCount`: optional integer, required in count mode, range 1–2147483647 on both platforms.
- Runtime state: `completedCaptures` and `countSignature`.
- Swift capture coordinator exposes successful task IDs for the scheduler while retaining its existing files return value for manual captures.

## Tests

Use E2E cases under `tests/cases/`. Exercise the real offline editor import/edit/export and native process configuration validation, pause/restart persistence, limit changes, exhausted-state recovery and reset after disable/re-enable. Exercise real successful fixed-time and interval captures when Screen Recording permission is available. Retain repeatable application artifacts in the case folder. Build the native Mac application, check editor syntax and compile Windows code if the local toolchain permits. Do not add unit tests or write a testing-results document.

## Assumptions

The user clarified: “a task with many specific clock times, stop after executed these times.” The one-time schedule consumes each clock time once; it has no date input and uses each time’s next eligible weekday. Optional capture counts measure successful scheduled task executions, not individual display images.

## Milestones

- [x] Implement Windows count and one-time clock list modes.
- [x] Implement Swift count and one-time clock list modes with success reporting.
- [x] Implement editor controls and validation.
- [x] Add repeatable E2E coverage and usage documentation.
- [x] Complete available verification and identify any platform limitations.

## Progress Log

- 2026-10-06 — Plan finalized; implementation started. Windows changes delegated with separate file ownership.

- 2026-10-06 17:22 +08:00 — User clarified one-time clock list behavior; implemented it alongside the optional quota stop mode. Completed implementation and documentation milestones. Final native verification is underway.

- 2026-10-06 17:22 +0800 — Implementation complete; required verification complete. No testing-results document created, per user instructions.

## Unresolved Issues

- None currently.
