# 06: Escape closes every dialog, and "Find in Log" focuses the Search field

**What to build:** Escape works as Cancel in every dialog: the host key dialog (as reject), the secret dialog, the Server form and the Servers dialog, innermost first. It doesn't leak through and clear the Search field behind the dialog. The Command Palette's "Find in Log" puts the cursor in the Search field, the same as ⌘F, and no longer opens the Filter panel.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness).

**Status:** resolved

- [x] Escape closes the host key dialog as reject, the secret dialog as Cancel.
- [x] With the Server form open, Escape closes the form first; a second Escape closes the Servers dialog.
- [x] New Log Space and Confirm keep closing on Escape.
- [x] Escape in a dialog leaves the Search Expression unchanged.
- [x] "Find in Log" focuses the Search field and doesn't change the Filter panel's visibility.
- [x] Harness tests for each dialog's Escape, the Search field staying intact, and Find in Log focus.

## Comments

- 2026-10-19: Done. Root cause found on the way: gpui (macOS) reports the key as "escape", while `handle_key_down` matched "Escape", so Escape had never worked anywhere in the app (the audit only read the code). The key is normalized at the top of the handler, which also fixes the existing Escape arms (Project Switcher, Raw View, context menu).
- `close_top_dialog` closes, topmost first: Confirm, host key (as reject), secret dialog, Server form then Servers dialog, New Log Space; Escape then stops, so the Search field behind is untouched. Cmd-F and the palette's Find in Log call `activate_search` (focus the Search field); the old `toggle_search`, which opened the Filter panel, is removed.
- Tests: `tests/escape_dialogs_test.rs` sends real keystrokes through the window (6 tests).
- Noticed, not fixed: the Project Switcher matches "ArrowUp"/"ArrowDown", while gpui names them "up"/"down", so its arrow keys likely don't work either.
