# Documentation

[Project README](../readme.md) · [Quick start](../readme.md#quick-start)

Scheduled Screenshot runs as a portable Windows executable. The browser manages settings; the executable owns scheduling and capture. Closing the browser leaves the scheduler running.

## Find the right guide

| You want to… | Read |
| --- | --- |
| Take a first screenshot or create a scheduled task | [Quick start](../readme.md#quick-start) and [Create a task](../readme.md#create-a-task) |
| Save settings, import a backup, or move app state | [Apply a configuration](configuration.md) |
| Understand the engine, capture worker, files, or local API | [Architecture and API](architecture.md) |
| Find source files, build tools, or test commands | [Project structure](project-structure.md) |
| Build a Windows executable | [Build and verify](../readme.md#build-and-verify) |
| Review test evidence and remaining release checks | [Verification](verification.md) |
| Understand the migration from the legacy implementation | [Legacy audit](legacy-audit.md) |

## Visual guides

| Diagram | What it explains | Editable source |
| --- | --- | --- |
| [Runtime overview](architecture.md#runtime-overview) | Browser and native controls, the shared engine, worker, and file outputs | [runtime-overview.mmd](diagrams/runtime-overview.mmd) |
| [Configuration import](configuration.md#import-a-configuration-file) | Validation, confirmation, errors, replacement, and resuming scheduling | [configuration-import.mmd](diagrams/configuration-import.mmd) |
| [Settings save](architecture.md#saving-edits-with-revision-checks) | Revision conflicts, persistence order, and applying saved state | [settings-save.mmd](diagrams/settings-save.mmd) |

The diagrams are Mermaid code blocks inside the guides. Use a Mermaid-capable Markdown viewer to render them; each guide also explains the flow in prose. Matching `.mmd` files are retained for editing and validation. No separate HTML pages or image exports are needed.

## Current guidance and historical records

Use the README and the guides above for current commands and behavior. Dated [verification entries](verification.md) describe the specific builds and checks they tested; they are not a claim that every release gate has passed.

Files in `plans/` record implementation decisions and progress. Files in `archive/` preserve pre-Rust changes and recovery metadata. Historical entries retain the paths and procedures used at the time.
