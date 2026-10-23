# 10: Go to line and Jump to time

**What to build:** `⌘L` (and a Command Palette entry) opens a small field: typing a line number goes to that line (in a Log Space, of the current file, or `file:line`), typing a time in any form the app can read goes to the first row at or after it.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] A number selects and scrolls to that line; out of range reports "Last line is N"
- [x] A time (`10:05`, `2026-10-20 10:05:00`, `20/Oct/2026:10:05:00`) selects the first row at/after it; a time outside the view reports the view's time span
- [x] Escape closes the field without moving
- [x] Works in Parsed and Raw View
- [x] Harness tests for line and time input

## Comments

- 2026-10-23: Done. Go to Line (cmd-l, cd4731e) now also takes `file:line` (`go_to::split_file_line`, a file name of the view, case aside) and a time (`go_to::parse_time`: any form a log line has, or `10:05`/`10:05:30` on the day of the selected line, else of the first timed row), landing on the first row at or after it; a time outside the view keeps the bar open with "Outside the view: <first> – <last>". A number past the end keeps its existing behaviour (last line, with a note) rather than an error. The time jump reads the Parsed rows, so in the Raw View it lands on the same line; it is tested in the Parsed View only. CONTEXT.md *Go to Line* updated. Tests in `tests/go_to_test.rs` and `src/go_to.rs`.
