# 07: The status bar speaks plainly

**What to build:** The status bar shows only what is meaningful right now. No "Ln 0" when no line is selected; the Focus count shows only while the Focus Pane is open and reads in plain words (e.g. "128 lines in Focus") instead of "Focus 0 of 6405".

**Blocked by:** ui-audit-fixes 01 (GPUI test harness and line count) — same status bar labels.

**Status:** resolved

- [x] With no selected line, no line label is shown; with one, "Ln 1,234" (with the file name in a Log Space of several files)
- [x] The Focus count is hidden while the Focus Pane is closed
- [x] Numbers use thousands separators
- [x] Every line count in the app (status bar, Focus count, File Section header, "lines held") uses the same formatter
- [x] "UTF-8" shows only when it tells something (e.g. a file read in another encoding), or is dropped
- [x] Harness test on the status bar labels for: nothing selected, selected line, Focus Pane open/closed

## Comments

- 2026-10-23: Re-checked: the bar reads "Ln 0  4,861 lines  Focus 0 of 4861  UTF-8" with nothing selected and the Focus Pane hidden — the Focus count still lacks separators while the line count has them. Same inconsistency in the Raw View: a File Section header says "3245 lines" and the line under it "3,245 lines held". Added criteria for both, and for the always-on "UTF-8" label. Still open — P1.
- 2026-10-23: Done. `status_bar_labels` shows "Ln 1,234" only with a current line ("b.log · Ln 1" in a Log Space of several files), "N lines", and "N lines in Focus" only while `focus_pane_shows_lines()` (docked and open, or in the Focus Window). "UTF-8" is dropped: the app reads every file as UTF-8, so it never told anything. One `crate::count::format_count` replaces the four digit groupers, and the Raw View File Section header and the Find Filter counter use it too. Tests in `tests/status_bar_test.rs`.
