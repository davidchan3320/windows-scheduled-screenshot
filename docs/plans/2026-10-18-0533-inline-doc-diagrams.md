# Inline documentation diagrams

- **Status:** Completed
- **Created:** 2026-10-18 05:33 +08:00
- **Last updated:** 2026-10-18 05:34 +08:00
- **Supersedes:** [Documentation readability](2026-10-18-0529-doc-readability.md)
- **Current blocker:** None
- **Next action:** None — implementation and required verification complete

## Goal

Display the existing diagrams directly in the Markdown guides following the user's request to update Markdown files.

## Implementation Changes

- Export the two approved HTML diagrams as SVGs using the diagram-design helper.
- Embed SVGs with descriptive alt text in architecture and configuration guides.
- Keep full-size and HTML links, and update README navigation and the project structure reference.

## Interfaces and Types

None. Documentation only.

## Tests

- Check exported SVGs render as images at Markdown body width.
- Validate local links and whitespace.

## Assumptions

- Existing light style, diagram content, and HTML sources stay authoritative.
- SVG exports use system-font fallbacks because embedded images cannot depend on remote font loading.

## Milestones

- [x] Existing HTML diagrams and readable guides completed and verified in predecessor.
- [x] Export SVGs and embed them in Markdown.
- [x] Verify images, links, and whitespace.

## Progress Log

- 2026-10-18 05:33 +08:00 — Extended completed documentation work to include inline Markdown images and exported both diagrams.
- 2026-10-18 05:34 +08:00 — Corrected the preview harness to load an HTML document before inserting image markup; SVG documents do not support `document.open`. Both embedded images, 49 local links, and whitespace checks passed. Completed the Markdown update.

## Verification Results

- SVG export helper — passed for both diagrams with `--system-fonts`.
- `node artifacts/docs-readability/check-embeds.cjs` — initial harness failed on `page.setContent` against an SVG document; corrected harness passed. Both SVG images load without network fonts, preserve the 960 × 600 viewBox, and have no overflowing node text. Embedded previews at 840px were visually reviewed.
- `python artifacts/docs-readability/check-links.py` — passed, 49 local links across six files.
- `git diff --check -- docs readme.md` — passed.

## Unresolved Issues

- None currently.
