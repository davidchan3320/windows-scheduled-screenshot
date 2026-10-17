# Configuration export and import

- **Status:** Completed
- **Created:** 2026-10-18 03:34 +08:00
- **Last updated:** 2026-10-18 03:37 +08:00
- **Supersedes:** None
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete

## Goal
Export portable JSON settings and import them from the task manager.

## Implementation Changes
Add export/download and import/file controls, replacement confirmation, validation, localized feedback and documentation. Import pauses scheduling and uses the existing revision-checked save path.

## Interfaces and Types
Schema-v2 settings JSON; authenticated POST export, validate-import and import APIs. Import requires an explicit schema version and task array. Runtime progress and screenshots are not exported.

## Tests
Rust validation/persistence/conflict tests, browser download/upload/cancel/error/localization tests, formatting and clippy checks, release packaging.

## Assumptions
Import replaces tasks and preferences. Matching task IDs use normal save reconciliation; new tasks start with fresh progress. Paths retain their values. Existing unrelated workspace changes remain intact.

## Milestones
- [x] Implement APIs and localized UI.
- [x] Add regression tests and documentation.
- [x] Verify and package release.

## Progress Log
- 2026-10-18 03:34 +08:00 — Implementation started after inspecting configuration storage, revision checks and UI conventions.
- 2026-10-18 03:37 +08:00 — Added authenticated export/validation/import APIs, localized controls and confirmation, JSON download/upload, size checks and stale-revision protection. Documented replacement, pause and progress semantics.
- 2026-10-18 03:37 +08:00 — Rust and browser verification passed; inspected desktop English and mobile Traditional Chinese import screenshots with no clipping or overflow. Release packaging started.
- 2026-10-18 03:37 +08:00 — Portable EXE, ZIP and checksum manifest generated in `artifacts/release-config`; implementation and required verification completed.

## Verification Results
- `cargo test --locked --all-targets` — passed, 26 tests including validation, persistence, progress preservation and revision-conflict checks.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo build --locked --example ui-test-host` — passed.
- `node scripts/browser-test.cjs` with existing Playwright installation and Chrome — passed, including download/upload round trips, cancellation, malformed/oversized/duplicate-task rejection, cross-window conflicts, BOM handling, language changes and empty configuration replacement.
- Desktop/mobile screenshot inspection — passed.
- `git diff --check` — passed.
- `./scripts/package.ps1 -OutputDirectory artifacts/release-config` — passed.

## Unresolved Issues
- None currently.
