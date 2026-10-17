# Project structure

```text
.cargo/                   Windows target and static-runtime configuration
.github/workflows/        CI verification and packaging
src/                      Rust application and library modules
ui/                       Embedded HTML, JavaScript, CSS, and language packs
resources/                Windows application manifest
tests/
  core.rs                 Rust integration tests discovered by Cargo
  browser/
    run.cjs               Starts the isolated UI host and runs the suite
    suite.cjs             Playwright tests, also used by native smoke tests
  native/
    smoke.py              Packaged-app checks and shared Windows test helpers
    hotkey-smoke.py        Real Windows shortcut checks
    soak.py               Long-running capture/resource checks
examples/
  ui-test-host.rs          Isolated HTTP host; Cargo example, never distributed
scripts/                  Build, packaging, stylesheet, notices, and cleanup tools
docs/
  configuration.md        Applying, importing, and troubleshooting configuration
  architecture.md         Runtime design and API
  verification.md         Dated results and outstanding release qualification
  plans/                  Implementation handoffs and progress records
  archive/                Preserved pre-Rust changes and recovery metadata
Cargo.toml, Cargo.lock    Rust package and pinned dependencies
build.rs                  Embeds resources/app.manifest in the Windows EXE
package*.json             UI build and browser-test tools
THIRD_PARTY_NOTICES.txt    Generated dependency notices embedded in the EXE
```

Rust library discovery uses `src/lib.rs`; the binary entry is `src/main.rs`, named `ScheduledScreenshot` in Cargo. Module responsibilities are described in [architecture](architecture.md). The separate `ui/` directory is embedded at compile time, so it is not needed beside the packaged executable. `ui/tailwind.css` is the stylesheet source; `ui/style.css` is generated and versioned so Cargo-only builds work without Node.js. Refresh notices with `python scripts/notices.py` after dependency changes.

Use the [README build instructions](../readme.md#build-and-verify) for prerequisites. From the repository root:

| Purpose | Command |
| --- | --- |
| Rust tests, including the UI-host target | `cargo test --locked --all-targets` |
| Build isolated browser host | `cargo build --locked --example ui-test-host` |
| Run isolated browser tests after building the host | `npm run test:browser` |
| Verify bundled stylesheet | `npm run check:css` |
| Package the standalone app | `./scripts/package.ps1` |
| Test and package, then run native smoke | `./scripts/test-production.ps1 -Browser` |
| Native smoke against an existing package | `python tests/native/smoke.py --exe artifacts/release/ScheduledScreenshot.exe` |
| Native shortcut checks | `python tests/native/hotkey-smoke.py --exe artifacts/release/ScheduledScreenshot.exe` |
| Full-day capture/resource check | `python tests/native/soak.py` |

Native checks need an unlocked Windows desktop and the normal app closed. The isolated browser host does not capture the desktop or launch the tray app.

`target/`, `node_modules/`, and `artifacts/` are ignored local build/dependency/output directories. Test reports and screenshots go under `artifacts/`; packages go under `artifacts/release/`. Optional local toolchains live under `artifacts/toolchain/`. Portable `data/` and `captures/` contain runtime state and user images, not source. This reorganization does not delete any of those directories. The existing `scripts/clean-artifacts.ps1` previews cleanup by default; review its list before applying because it includes historical reports and backups outside its preserved directories.

Earlier verification entries, plans, and archived patches retain their original paths as historical evidence. For current commands, use this page and the README. The structure cleanup moved `rust/` to `src/`, the root manifest to `resources/`, browser checks to `tests/browser/`, and native Python checks from `scripts/` to `tests/native/`.
