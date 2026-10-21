# 10: Go to line and Jump to time

**What to build:** `⌘L` (and a Command Palette entry) opens a small field: typing a line number goes to that line (in a Log Space, of the current file, or `file:line`), typing a time in any form the app can read goes to the first row at or after it.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] A number selects and scrolls to that line; out of range reports "Last line is N"
- [ ] A time (`10:05`, `2026-10-20 10:05:00`, `20/Oct/2026:10:05:00`) selects the first row at/after it; a time outside the view reports the view's time span
- [ ] Escape closes the field without moving
- [ ] Works in Parsed and Raw View
- [ ] Harness tests for line and time input
