# 14: Remove error-prone shortcuts

**What to build:** Two shortcuts are easy to hit by mistake. Clear (of the current file) no longer sits on `⌘⇧K`, right next to the Command Palette's `⌘K`; it stays reachable from the Command Palette and the file's menu. Space no longer toggles the context panel when not typing (Space usually scrolls a page in viewers).

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] `⌘⇧K` does nothing; Clear is in the Command Palette and the file's menu
- [x] Space does not toggle the context panel (Enter still does, or the user's chosen key)
- [x] Keymap settings and any help text reflect the change
- [x] Harness test that `⌘⇧K` and Space leave state unchanged

> Decision taken without explicit confirmation: Clear moves to palette/menu only and Space is unbound. Revisit if the user prefers another key.

## Comments

- 2026-10-23: Done. Live: Clear File has no default Shortcut any more (it was `⌘⇧K`, next to the Command Palette's `⌘K`); it stays in the Command Palette, the file's menu and the status bar, and the user can give it a Shortcut on the Keymap page. Space had already lost its binding when the Keymap became editable (0cf963b); a test now holds both. Tests in `tests/error_prone_keys_test.rs`.
