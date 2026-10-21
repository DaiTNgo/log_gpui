# 01: GPUI test harness and correct line count in the status bar

**What to build:** A maintainer can write a test that opens the real `AppView` in a GPUI test window, loads a log, and reads what the person would see. The first test using it proves the status bar shows the line count with real thousands separators: "400 lines", "12,345 lines", "1,200,000 lines". Today it shows "0,400 events" and "1200,000 events".

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] gpui's `test-support` feature is enabled for dev/test builds only; release builds are unchanged.
- [x] `AppView` can be built inside a `VisualTestContext` without touching the user's real config: session, `servers.json` and the scratch store point at a temp dir, and remembered secrets use an in-memory store instead of the OS keychain.
- [x] The way tests do this (one constructor option or an env override) is small and documented in the test helper; it isn't threaded through every call site.
- [x] A shared test helper opens `AppView` with an Imported Log Space holding a given set of lines.
- [x] Status bar line counts use a thousands-separator formatter; the label uses one word consistently (pick "lines" or "events" in line with CONTEXT.md).
- [x] Tests: the formatter on 0, 999, 1,000, 12,345 and 1,200,000; and a harness test asserting the status-bar text for a 400-line Log Space.
- [x] If `AppView` can't be built in the test window (keychain, file watchers, SSH hub threads), stop and record why in this file before falling back to view-model tests.

## Resolution

- `Cargo.toml`: `gpui` with `test-support` added under `[dev-dependencies]` only.
- `AppView::with_services(persistence, RemoteUi, window, cx)` is the one constructor option; `with_persistence` delegates to it. Tests pass `RemoteUi::with_paths(temp servers.json, temp known_hosts.json, MemorySecrets)`.
- `tests/common/mod.rs`: `open_app`, `open_app_with_lines` (Imported Log Space), `sample_lines`. `AppView` builds fine in a `VisualTestContext` (no keychain, watcher or hub problems); no fallback needed.
- Label is "N lines" (CONTEXT.md speaks of lines); `app::format_count` formats it; `AppView::status_bar_labels()` returns the plain left labels the bar renders.
- Tests in `tests/status_bar_test.rs`. `cargo test`: 419 passed, 1 ignored.
- Work lives in worktree `.claude/worktrees/intelligent-dirac-83ff28` (uncommitted), not the main checkout.
