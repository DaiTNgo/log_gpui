# 07: The status bar speaks plainly

**What to build:** The status bar shows only what is meaningful right now. No "Ln 0" when no line is selected; the Focus count shows only while the Focus Pane is open and reads in plain words (e.g. "128 lines in Focus") instead of "Focus 0 of 6405".

**Blocked by:** ui-audit-fixes 01 (GPUI test harness and line count) — same status bar labels.

**Status:** ready-for-agent

- [ ] With no selected line, no line label is shown; with one, "Ln 1,234" (with the file name in a Log Space of several files)
- [ ] The Focus count is hidden while the Focus Pane is closed
- [ ] Numbers use thousands separators
- [ ] Harness test on the status bar labels for: nothing selected, selected line, Focus Pane open/closed
