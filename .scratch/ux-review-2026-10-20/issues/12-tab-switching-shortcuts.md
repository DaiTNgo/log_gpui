# 12: Switching tabs by keyboard

**What to build:** `⌘1`…`⌘8` go to the Nth tab, `⌘9` to the last, `⌃Tab`/`⌃⇧Tab` to the next/previous tab, Pinned Tabs included in order.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] `⌘N` activates tab N if it exists; `⌘9` the last tab
- [x] `⌃Tab` / `⌃⇧Tab` cycle with wrap-around
- [x] Shortcuts listed in the Keymap settings
- [x] Harness test

## Comments

- 2026-10-23: Done. Most of it was already there (⌘1–⌘9 and ⌃Tab/⌃⇧Tab with wrap-around, as Fixed Keys); ⌘9 now goes to the last tab whatever the count. The Keymap page named "⌘Tab", which macOS keeps for switching apps: it now names ⌃Tab / ⌃⇧Tab and says ⌘9 is the last tab. These are Fixed Keys, so they are listed in that sentence, not as Commands. Tests in `tests/tab_keys_test.rs` and `tests/workspace_test.rs`.
