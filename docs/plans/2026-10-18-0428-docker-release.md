# Docker release build

- **Status:** Completed
- **Created:** 2026-10-18 04:28 +08:00
- **Last updated:** 2026-10-18 05:02 +08:00
- **Supersedes:** None
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete.

## Goal
Build and package the portable Windows release using Docker.

## Implementation Changes
Add a Linux Docker build using Rust and cargo-xwin for the existing Windows MSVC target. Export only the EXE, ZIP and checksums. Default package.ps1 to Docker, retain explicit native packaging, update CI and build documentation.

Disable the linker's automatic UAC fragment because the supplied manifest already declares asInvoker. This avoids LLVM 14 corrupting XML namespaces when merging duplicate UAC sections.

Validate the linked PE architecture, manifest namespaces/settings and static CRT imports inside Docker before packaging, using pinned pefile 2024.8.26.

## Interfaces and Types
`./scripts/package.ps1 [-OutputDirectory path]` uses Docker. `-Native` uses the existing Windows toolchain; `-NoBuild` packages the existing native target executable.

## Tests
Run an actual Docker release build. Verify ZIP identity, SHA-256 checksums, x64 PE format, static CRT imports and the embedded Windows manifest, including Windows activation-context acceptance. Check PowerShell syntax and review the workflow. Confirm the packaging validator accepts a valid manifest and rejects the initially malformed one.

## Assumptions
Docker uses Linux containers. The packaged application still runs only on Windows. Checked-in UI assets are embedded. Native desktop smoke requires the running application to be closed and is a non-required follow-up for this build change.

## Milestones
- [x] Add Docker build, packaging entry point and CI wiring.
- [x] Update build documentation.
- [x] Build and verify Docker release artifacts.

## Progress Log
- 2026-10-18 04:28 +08:00 — Implementation started; Docker Linux engine is available.
- 2026-10-18 04:32 +08:00 — Build documentation and CI updated. Container toolchain installed; first build is downloading the CRT/SDK. Native NoBuild compatibility check passed.
- 2026-10-18 04:49 +08:00 — First Docker release compiled successfully. Artifact verification caught an invalid UAC namespace from LLVM 14's manifest merge. Added /MANIFESTUAC:NO, verified it with an isolated linker test, and started Docker/native rebuilds. A cache diagnostic reset the SDK cache, requiring Docker to repeat SDK preparation.
- 2026-10-18 04:51 +08:00 — Native release build and complete artifact verification passed with the manifest fix. Corrected Docker build remains in SDK preparation.
- 2026-10-18 05:02 +08:00 — Corrected Docker build and Windows loader validation passed. Added automatic PE/manifest/CRT validation, confirmed acceptance/rejection behavior, and rebuilt successfully with SDK/Cargo caches reused. All required work is complete.

## Verification Results
- PowerShell AST parse of scripts/package.ps1: passed.
- Python AST parse of scripts/package-release.py: passed.
- `git diff --check`: passed.
- `./scripts/package.ps1 -NoBuild -OutputDirectory artifacts/release-native-package-check`: passed.
- `./scripts/package.ps1 -OutputDirectory artifacts/release-docker`: initial compilation passed; corrected build and final build with automatic validation both passed.
- Initial artifact verification: checksums, ZIP identity, x64 GUI PE and static CRT imports passed; manifest namespace check failed. Subsequent builds with the linker fix passed the manifest check.
- Isolated LLVM 14 linker test with `/MANIFESTUAC:NO` and the supplied manifest: passed asInvoker and XML namespace checks.
- `./scripts/package.ps1 -Native -OutputDirectory artifacts/release-native-manifest`: passed.
- `python artifacts/verify-docker-release.py artifacts/release-native-manifest`: passed checksums, ZIP identity, x64 GUI PE, static CRT imports and embedded manifest checks.
- `cargo fmt --all -- --check`: passed.
- `python scripts/package-release.py`: accepted the corrected native package and rejected the initial Docker package's invalid manifest, as expected.
- `python artifacts/verify-docker-release.py`: passed final Docker ZIP identity, SHA-256, x64 GUI PE, static CRT imports, embedded manifest and Windows activation-context checks. Report: `artifacts/docker-release-verification.json`.
- Workflow review and final whitespace checks passed. GitHub Actions was not run remotely; native desktop smoke and full release qualification remain non-required follow-up for this build change.

## Unresolved Issues
- None currently.
