# Combined screenshots by default

- **Status:** Completed
- **Created:** 2026-10-19 00:16 +08:00
- **Last updated:** 2026-10-19 00:22 +08:00
- **Supersedes:** None
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete.

## Goal
Save all connected monitors in one image by default, with an option to split images by screen.

## Implementation Changes
Compose monitor captures in their physical Windows desktop arrangement, with black gaps. Preserve image encoding, cursor, collision protection and failure cleanup. Add quick capture and task options, localized UI, previews and documentation.

## Interfaces and Types
Add schema-v2 boolean settings `manualSplitByMonitor` and task capture `splitByMonitor`, both defaulting to false when absent. Combined filenames use `all-monitors` for `{display}` and `1` for `{displayIndex}`.

## Tests
Rust settings compatibility/persistence and synthetic monitor layouts; native capture checks for combined dimensions and split files; existing Rust and browser suites, formatting, clippy, CSS and diff checks.

## Assumptions
The option applies independently to quick captures and each scheduled task. Missing fields in existing settings adopt the requested combined default. Existing unrelated working tree changes remain intact.

## Milestones
- [x] Implement capture composition and compatible settings.
- [x] Add localized UI, previews, documentation and regression coverage.
- [x] Run verification and review final diff.
- [x] Package the portable executable.

## Progress Log
- 2026-10-19 00:16 +08:00 — Finalized implementation plan; backend owned by primary agent, independent UI/docs/browser changes delegated.
- 2026-10-19 00:22 +08:00 — Capture composition, settings, localized controls and documentation completed. Independent review found a filename preview/validation mismatch; corrected shared preview paths before successful browser recheck. Native capture verified on the available single-monitor desktop; synthetic multi-monitor pixel tests passed. Packaging started.
- 2026-10-19 00:22 +08:00 — Release executable, ZIP and checksums packaged in `artifacts/release-combined`; implementation and required verification complete.

## Verification Results
- `cargo test --locked --all-targets` — passed: 33 tests; interactive native test excluded by default and run separately below.
- `cargo test --locked native_combined_and_split_capture_dimensions_and_files -- --ignored --nocapture` — passed: PNG and JPEG in both modes, with cursor, on one 1920×1080 screen. Temporary screenshots removed by the test fixture.
- `cargo clippy --locked --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo build --locked --example ui-test-host` — passed.
- `npm run test:browser` with `PLAYWRIGHT_BROWSERS_PATH=artifacts/playwright` — initially failed on combined filename preview; passed after correcting the preview API and shared path validation.
- `npm run check:css` — passed; bundled CSS current.
- Desktop task editor and mobile capture settings screenshots — inspected; controls fit and remain readable.
- `git diff --check` — passed.
- `./scripts/package.ps1 -Native -OutputDirectory artifacts/release-combined` — passed.

## Unresolved Issues
- Non-required follow-up: physical multi-monitor hardware verification; this session exposes one monitor. Synthetic tests cover three monitors, negative origins, different sizes, gaps, invalid bounds and capture failure.
