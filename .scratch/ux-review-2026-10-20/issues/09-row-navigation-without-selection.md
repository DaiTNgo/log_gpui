# 09: Moving between rows by keyboard without selecting one first

**What to build:** Arrow keys work as soon as the view has focus: with no selected row, ↓/↑ select the first/last row in sight. PageUp/PageDown move the selection by one screen, Home/End go to the first/last row (End turns Follow back on in a live Log Space).

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] ↓ with no selection selects the first visible row; ↑ the last visible row
- [x] PageUp/PageDown move by the number of rows in sight and keep the selection in view
- [x] Home/End jump to first/last row; End re-enables Follow on a live Log Space
- [x] None of these fire while typing in a text field
- [x] Harness tests for each key

## Comments

- 2026-10-23: Done. Root cause on the way: the handler matched "ArrowDown"/"ArrowUp", but gpui names the keys "down"/"up" (like "escape", see ui-audit-fixes 06), so ↑/↓ had never moved the selection. `go_to::row_for_key` works out where ↓ ↑ PageDown PageUp Home End land (no selection: ↓ the first row in sight, ↑ the last; a page is the rows in sight); the keys act only while no text field has the keyboard, and End turns Follow on in a live Log Space. Parsed View only; the Raw View keeps its own keys. Tests in `tests/row_keys_test.rs`.
