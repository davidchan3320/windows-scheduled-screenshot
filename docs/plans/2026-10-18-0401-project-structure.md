# Project structure cleanup

- **Status:** Completed
- **Created:** 2026-10-18 04:01 +08:00
- **Last updated:** 2026-10-18 04:03 +08:00
- **Supersedes:** None
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete

## Goal

Make the Rust project's source, tests, build tooling, and resources easy to locate without changing application behavior.

## Implementation Changes

- Use Cargo's conventional `src/` directory and default library discovery.
- Group browser tests in `tests/browser/` and Windows native smoke/soak checks in `tests/native/`.
- Keep build/maintenance commands in `scripts/`, and move the Windows manifest to `resources/`.
- Update executable references, CI, and current documentation; document generated outputs and historical records.

## Interfaces and Types

Application APIs, settings, executable name, and package layout stay the same. Test file paths change; `npm run test:browser`, Cargo commands, and PowerShell entry points remain available.

## Tests

- Rust formatting, Clippy, all-target tests, and browser host build.
- CSS freshness and existing isolated browser suite.
- Python syntax and relocated import/root resolution; PowerShell syntax.
- Release packaging in a separate output folder and ZIP identity.
- Reference/link and whitespace checks.

## Assumptions

The existing uncommitted migration is intentional. Preserve it, archived sources, user data, and existing artifacts. Historical verification commands and earlier handoffs describe their original layout and remain unchanged. Native capture behavior is unchanged; desktop-interacting smoke tests and the full-day release qualification are outside this structural cleanup.

## Milestones

- [x] Review current layout and path dependencies.
- [x] Relocate files and update active references.
- [x] Document the resulting layout.
- [x] Complete required verification.

## Progress Log

- 2026-10-18 04:01 +08:00 — Reviewed layout; finalized cleanup and started implementation.
- 2026-10-18 04:03 +08:00 — Relocated sources, native/browser tests, and manifest; updated Cargo, npm, CI, production runner, and current docs. Added the project structure guide. Rust and syntax checks passed. Initial browser attempt failed because the pinned Playwright browser was not installed; reran with the supported installed-Chrome override.
- 2026-10-18 04:03 +08:00 — Browser suite, release package, manifest embedding, checksum/ZIP identity, documentation links, and reference checks passed. Cleanup completed without deleting existing artifacts or changing application behavior.

## Verification Results

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked --all-targets`: passed, 28 tests (27 integration tests and one library test).
- `cargo build --locked --example ui-test-host`: passed.
- `npm run check:css`: passed; generated stylesheet unchanged.
- `node --check tests/browser/run.cjs` and `node --check tests/browser/suite.cjs`: passed.
- Python AST parsing of `scripts/*.py` and `tests/native/*.py`, plus importing smoke/soak/hotkey modules and asserting their repository roots: passed.
- PowerShell parser check of `scripts/*.ps1`: passed.
- Initial `npm run test:browser`: failed before test execution because Playwright's browser binary was missing. Rerun with `CHROME_PATH=C:/Program Files/Google/Chrome/Application/chrome.exe`: passed.
- `./scripts/package.ps1 -OutputDirectory artifacts/release-structure`: passed. ZIP contains exactly the byte-identical EXE; SHA-256 checksums match; manifest markers for DPI awareness, execution level, and long paths are embedded.
- Current README/architecture/structure documentation links and active old-path search: passed. Historical paths remain only in explicitly historical records and the move summary.
- `git diff --check`: passed.
- Native capture/shortcut smoke, full-day soak, and clean-OS/device qualification: not rerun; outside this file-structure-only task. Existing release gates remain in `docs/verification.md`.

## Unresolved Issues

- None currently.
