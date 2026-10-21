# 14: Remove error-prone shortcuts

**What to build:** Two shortcuts are easy to hit by mistake. Clear (of the current file) no longer sits on `⌘⇧K`, right next to the Command Palette's `⌘K`; it stays reachable from the Command Palette and the file's menu. Space no longer toggles the context panel when not typing (Space usually scrolls a page in viewers).

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] `⌘⇧K` does nothing; Clear is in the Command Palette and the file's menu
- [ ] Space does not toggle the context panel (Enter still does, or the user's chosen key)
- [ ] Keymap settings and any help text reflect the change
- [ ] Harness test that `⌘⇧K` and Space leave state unchanged

> Decision taken without explicit confirmation: Clear moves to palette/menu only and Space is unbound. Revisit if the user prefers another key.
