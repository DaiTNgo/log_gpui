# 16: Recent Search Expressions

**What to build:** The Search field remembers recent Search Expressions. With the field empty (or with the history opened), ↑/↓ step through them; a small dropdown lists them for picking. The history is kept per Project across restarts.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] An expression is added when it was searched (Enter or a pause after typing), not for every keystroke; duplicates move to the top; capped (e.g. 20)
- [x] ↑/↓ in an empty field cycle the history; Escape restores what was typed
- [x] A dropdown lists recent expressions with a way to remove one
- [x] History persists with the session, per Project
- [x] Harness test for adding, cycling and persisting

## Comments

- 2026-10-23: Done. `search_history::SearchHistory` (newest first, no repeats, 20 at most) is kept per Project in the session. An expression is recorded when the typing pauses long enough to search, or on Enter; while the user goes on typing it, the next pause replaces it, so "err" then "error" leaves only "error". ↑/↓ in an empty Search field step through it (↓ past the newest gives back what was typed) and Escape restores the typed text. A clock button beside the field lists them; click one to search for it, × to forget it. The dropdown was not checked in the running app. Tests in `tests/search_history_test.rs` and `src/search_history.rs`.
