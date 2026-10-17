# Docker release cache and version fixes

- **Status:** Completed
- **Created:** 2026-10-18 05:20 +08:00
- **Last updated:** 2026-10-18 05:23 +08:00
- **Supersedes:** [Docker release build](2026-10-18-0428-docker-release.md)
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete.

## Goal
Fix the Dockerfile review findings: preserve expensive preparation across CI builds and pin the Microsoft SDK/CRT.

## Implementation Changes
Prepare the pinned SDK 10.0.26100 and CRT 14.44.17.14 before copying sources, copying the checked cache into a normal toolchain layer. Use cargo-xwin's update check so a pre-existing cache cannot silently bypass version selection. Persist Cargo registry and target mounts explicitly in GitHub Actions using a commit-pinned cache-dance action and a toolchain/dependency cache key. Update the README and verification record.

Clear only the application's Cargo package outputs before compiling on context changes. The source-change test exposed stale executable reuse with cached timestamps; dependencies remain cached.

## Interfaces and Types
Existing package.ps1 options and release outputs remain the same.

## Tests
Build the release, verify checksums/ZIP/PE/manifest/Windows activation context, check the Dockerfile and workflow, and build changed sources to confirm the SDK layer is reused. Verify the prepared SDK works without the preparation cache mount.

## Assumptions
The existing tested Microsoft components are the desired pinned versions. CI cache restore/save is reviewed locally; remote GitHub execution is follow-up.

## Milestones
- [x] Docker packaging, CI wiring and original artifact validation completed in the predecessor.
- [x] Implement SDK layer, version pins and CI Cargo mount persistence.
- [x] Verify release and source-change cache reuse.
- [x] Update documentation and complete final checks.

## Progress Log
- 2026-10-18 05:20 +08:00 — Applied both review fixes; validation started.
- 2026-10-18 05:23 +08:00 — Release, offline SDK and workflow checks passed. Changed embedded HTML exposed stale Cargo application outputs; added package-scoped cleaning and will repeat the source-change test, checking executable bytes for the changed content.
- 2026-10-18 05:23 +08:00 — Corrected release and source-change checks passed, including embedded-content freshness and dependency reuse. README and verification record updated; implementation and required verification complete.

## Verification Results
- Initial release and artifact validation: passed; PowerShell's stderr pipeline returned status 1 despite successful Docker completion. Subsequent skill build completed with status 0.
- `docker build --check .`: passed with no warnings.
- Offline SDK preparation without cache mounts: passed.
- `go run github.com/rhysd/actionlint/cmd/actionlint@v1.7.7 .github/workflows/windows-build.yml`: passed.
- Initial source-change build: SDK layer reused, but Cargo reused stale application outputs; failed freshness check. Package-scoped clean added, retest pending.
- `./scripts/package.ps1 -OutputDirectory artifacts/release-docker`: passed after package-scoped clean; application compiled in 21.22 seconds with dependencies reused.
- `python artifacts/verify-docker-release.py artifacts/release-docker`: passed ZIP identity, SHA-256, Windows x64 GUI PE, static CRT imports, embedded manifest and Windows activation context.
- Repeated isolated source-change build: passed, SDK preparation layer cached, only application compiled (20.57 seconds). Binary-content assertions confirmed changed HTML embedded, marker absent from original release, and different executable hashes.
- Docker skill `scripts/verify-build.sh scheduled-screenshot-release-review:local`: passed; artifact-only final image, 3.39 MB. No runtime user is needed for the scratch artifact exporter.
- Final `docker build --check .` and `git diff --check`: passed.

## Unresolved Issues
- Remote GitHub cache restore/save and full desktop release qualification are non-required follow-up; not run during this local Docker cache fix.
