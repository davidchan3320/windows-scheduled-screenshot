# Project structure

[Documentation index](README.md) · [Build instructions](../readme.md#build-and-verify) · [Architecture and API](architecture.md)

[Source layout](#source-layout) · [Where to make changes](#where-to-make-changes) · [Commands](#commands) · [Generated files](#generated-files) · [Historical paths](#historical-paths)

## Source layout

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
  README.md               Documentation index and reading paths
  configuration.md        Applying, importing, and troubleshooting configuration
  architecture.md         Runtime design and API
  diagrams/               Mermaid sources matching diagrams in Markdown guides
  verification.md         Dated results and outstanding release qualification
  plans/                  Implementation handoffs and progress records
  archive/                Preserved pre-Rust changes and recovery metadata
Cargo.toml, Cargo.lock    Rust package and pinned dependencies
Dockerfile, .dockerignore Container release build and restricted source context
build.rs                  Embeds resources/app.manifest in the Windows EXE
package*.json             UI build and browser-test tools
THIRD_PARTY_NOTICES.txt    Generated dependency notices embedded in the EXE
```

Rust library discovery uses `src/lib.rs`; the binary entry is `src/main.rs`, named `ScheduledScreenshot` in Cargo. Module responsibilities are described in [architecture](architecture.md). The separate `ui/` directory is embedded at compile time, so it is not needed beside the packaged executable. `ui/tailwind.css` is the stylesheet source; `ui/style.css` is generated and versioned so Cargo-only builds work without Node.js. Refresh notices with `python scripts/notices.py` after dependency changes.

## Where to make changes

| Change | Start here | Related check |
| --- | --- | --- |
| Schedule calculations or capture limits | `src/schedule.rs`, `src/engine.rs` | `cargo test --locked --all-targets` |
| Settings schema or persistence | `src/config.rs`, `src/storage.rs` | `cargo test --locked --all-targets` |
| Browser layout or interaction | `ui/index.html`, `ui/app.js`, `ui/tailwind.css` | Rebuild CSS, then run the browser suite |
| Translations | `ui/locales/`, `src/localization.rs`, `ui/i18n.js` | Rust tests and bilingual browser checks |
| Native capture, shortcut, or tray | `src/platform.rs`, `src/output.rs` | Packaged native smoke and shortcut checks |
| Documentation diagrams | Mermaid blocks in the guides and matching `docs/diagrams/*.mmd` | Render with Mermaid CLI and check source synchronization and links |

### Maintaining diagrams

Edit the Mermaid block in the guide and its matching `.mmd` file together. Keep diagram labels and prose aligned with the source code. Use flowcharts for component relationships and decisions, and sequence diagrams for ordered API interactions.

Validate a changed source with an installed Mermaid CLI before publishing:

```powershell
mmdc -i docs/diagrams/runtime-overview.mmd -o artifacts/runtime-overview.svg
```

Use the same command with `configuration-import` or `settings-save` for the other diagrams. Rendered previews belong in ignored `artifacts/`; the maintained documentation consists of Markdown and `.mmd` sources. No HTML documentation pages are required.

## Commands

Use the [README build instructions](../readme.md#build-and-verify) for prerequisites. Run commands from the repository root:

| Purpose | Command |
| --- | --- |
| Rust tests, including the UI-host target | `cargo test --locked --all-targets` |
| Build isolated browser host | `cargo build --locked --example ui-test-host` |
| Run isolated browser tests after building the host | `npm run test:browser` |
| Verify bundled stylesheet | `npm run check:css` |
| Build and package the standalone app in Docker | `./scripts/package.ps1` |
| Build and package with the native Windows toolchain | `./scripts/package.ps1 -Native` |
| Test and package, then run native smoke | `./scripts/test-production.ps1 -Browser` |
| Native smoke against an existing package | `python tests/native/smoke.py --exe artifacts/release/ScheduledScreenshot.exe` |
| Native shortcut checks | `python tests/native/hotkey-smoke.py --exe artifacts/release/ScheduledScreenshot.exe` |
| Full-day capture/resource check | `python tests/native/soak.py` |

Native checks need an unlocked Windows desktop and the normal app closed. The isolated browser host does not capture the desktop or launch the tray app.

Release packaging defaults to Docker Linux containers and cross-compiles the Windows MSVC target. `scripts/package-release.py` creates the executable-only ZIP and checksums inside the container. The final Docker stage exports the three release files into `artifacts/release/`; it is an artifact export, not a runnable application image. `scripts/test-production.ps1` still requires the host Rust toolchain and Python for tests, and uses Docker for packaging unless `-NoBuild` selects an existing native build.

## Generated files

| Path | Contents | Versioned? |
| --- | --- | --- |
| `ui/style.css` | Generated stylesheet embedded in the executable | Yes; rebuild after UI class or stylesheet changes |
| `target/` | Native Rust build outputs | No |
| `node_modules/` | UI and browser-test dependencies | No |
| `artifacts/` | Packages, test reports, screenshots, and optional local toolchains | No |
| `data/`, `captures/` | Portable app state and default screenshot output | No; user data |

Packages normally go under `artifacts/release/`; optional local toolchains live under `artifacts/toolchain/`. The existing `scripts/clean-artifacts.ps1` previews cleanup by default. Review its list before applying because it includes historical reports and backups outside its preserved directories.

## Historical paths

Earlier verification entries, plans, and archived patches retain their original paths as historical evidence. For current commands, use this page and the README. The structure cleanup moved `rust/` to `src/`, the root manifest to `resources/`, browser checks to `tests/browser/`, and native Python checks from `scripts/` to `tests/native/`.
