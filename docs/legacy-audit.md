# Legacy parity and preservation

Before removing C#/Swift, all uncommitted changes were read. The full legacy sources/tests/docs are saved locally in `artifacts/legacy-before-rust.zip`. Uncommitted tracked changes and new legacy files are also preserved in the versioned [patch](archive/pre-rust-working-tree.patch), with the original base commit and checksums in [preservation.json](archive/preservation.json). Apply this patch to the recorded original commit in a separate checkout, not the Rust tree.

| Existing change | Rust replacement and verification |
|---|---|
| Consume pending occurrences once | Armed occurrences removed before dispatch; exact-count native smoke |
| Accept expired stop dates | Core regression; independent automatic task completion |
| Correct capture raster flags | Native BitBlt; real JPEG/PNG smoke |
| Native folder selection/cancellation | COM IFileOpenDialog; real cancellation and browser contract tests |
| Save settings from browser | Atomic saves with revisions; CRUD and stale-edit tests |
| Reject malformed types and render names safely | Strict serde types, validation and DOM textContent; malformed-type/markup tests |
| Preserve capture counts and fixedOnce slots | Migration retains IDs, anchors, deadlines, counts and completed slots; migration/restart tests |
| Swift equivalents of stop/count fixes | Windows behavior preserved; macOS platform/build/docs retired |

The new batch-success rule requires every monitor image to save. Manual captures do not consume scheduled counts or fixedOnce slots. Legacy fixedOnce consumes an attempted slot even on failure; busy/unavailable slots remain eligible on a later selected day. New dated one-time tasks become completed, failed or missed without retries.

Current-machine parity verification permits retiring the active legacy sources. Clean-OS, physical sleep/lock/display/DPI matrix and full 24-hour soak remain separate release gates in the handoff and verification record.
