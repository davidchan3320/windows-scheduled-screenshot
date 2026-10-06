# Capture count stop mode

- **Status:** In progress
- **Created:** 2026-10-06
- **Last updated:** 2026-10-06
- **Planning model:** Unknown
- **Supersedes:** None
- **Current blocker:** None
- **Next action:** See superseding plan [Execute a list of clock times once](2026-10-06-1722-fixed-time-list.md).

## Goal

Allow a task to execute at configured fixed local clock times or recurring intervals and automatically stop after a chosen number of successful scheduled captures.

## Implementation Changes

- Add an `After a number of captures` stop mode and whole-number input to the existing offline editor; preserve import/export and reject missing, fractional, zero, negative and out-of-range counts.
- Extend Windows models, validation and scheduler, and the equivalent Swift configuration and scheduler.
- Count one successful scheduled execution per task, irrespective of display count or shared capture profile. Manual captures, capture failures and busy/session skips do not consume the quota.
- Persist progress across restart, pause, and cosmetic or schedule edits. Reset when the count limit changes or the task is disabled and enabled again.
- Persist completion before disabling tasks; recover an exhausted enabled task after restart. Ignore results for a task whose configuration changed during an asynchronous capture.
- Document the configuration contract and usage in `docs/capture-count.md`.

## Interfaces and Types

- `stopCondition.mode`: add `count`.
- `stopCondition.captureCount`: optional integer, required in count mode, range 1–2147483647 on both platforms.
- Runtime state: `completedCaptures` and `countSignature`.
- Swift capture coordinator exposes successful task IDs for the scheduler while retaining its existing files return value for manual captures.

## Tests

Use E2E cases under `tests/cases/`. Exercise the real offline editor import/edit/export and native process configuration validation, pause/restart persistence, limit changes, exhausted-state recovery and reset after disable/re-enable. Exercise real successful fixed-time and interval captures when Screen Recording permission is available. Retain repeatable application artifacts in the case folder. Build the native Mac application, check editor syntax and compile Windows code if the local toolchain permits. Do not add unit tests or write a testing-results document.

## Assumptions

Specific execution times mean the existing fixed local times schedule. The user has been offered clarification between this and a future start time plus interval. Count means successful scheduled task executions, not individual display images.

## Milestones

- [ ] Implement Windows count mode.
- [ ] Implement Swift count mode and success reporting.
- [ ] Implement editor input and validation.
- [ ] Add repeatable E2E coverage and usage documentation.
- [ ] Complete available verification and identify any platform limitations.

## Progress Log

- 2026-10-06 — Plan finalized; implementation started. Windows changes delegated with separate file ownership.

- 2026-10-06 17:22 +0800 — Superseded after user clarification by [Execute a list of clock times once](2026-10-06-1722-fixed-time-list.md).

## Unresolved Issues

- None currently.
