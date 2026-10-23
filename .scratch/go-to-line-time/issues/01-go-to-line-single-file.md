# 01: Go to Line in a single file

**What to build:** The tracer bullet. Add the Go to bar and the Keymap command `navigate.go_to_line` (`Navigate: Go to Line…`, default `cmd-l`). The command opens the bar in Line mode with a hint `1–N` for the target file. Typing a positive whole number and pressing Enter makes that line the current line of the open Focus Tab through `set_current_line(…, Origin::Jump, …)`, so the line is selected, centred and flashed in both the Parsed and the Raw View, and the Focus Pane selects it when it lists it. The bar then closes. A number past the last line lands on the last line with a note. Invalid text shows an inline error and does not jump. Escape closes the bar without moving. Search, Filter and Time Range stay untouched. Scope here is a File View, or a Log Space with exactly one file.

Put the parsing and clamping in a new pure module `src/go_to.rs` (`parse_line`, `resolve_line`).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] `navigate.go_to_line` exists, is listed in the Command Palette and is rebindable in the Keymap; `cmd-l` by default
- [x] Bar opens in Line mode with the `1–N` hint; Escape closes it without moving
- [x] Enter on a valid number lands on that line in the Parsed View and in the Raw View, flashed
- [x] The Focus Pane selects the line when it lists it
- [x] Past the end: lands on the last line and shows a note
- [x] `0`, negative, non-numeric or empty input: inline error, no jump
- [x] Search, Filter, Time Range and Focus Tabs are unchanged after a jump
- [x] Unit tests on `src/go_to.rs`; app test in `tests/go_to_test.rs`
- [x] **Go to Line** is defined in CONTEXT.md, with the rule that a jump never changes what is shown

## Comments

- 2026-10-23: Implemented in `src/go_to.rs` (parse/resolve), `src/app/go_to.rs` (bar, target file, jump) and `tests/go_to_test.rs`. The target-file and Generation defaults from issue 02 are already in `go_to_line_file`; 02 still owes the file picker and the live cases. The bar was checked only through tests, not by eye in the running app.
