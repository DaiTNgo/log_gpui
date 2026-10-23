# 06: Live, Pause and Follow in the status bar show their state

**What to build:** Someone glancing at the status bar can tell whether Follow is on, whether the Log Space is paused, and what the connection is doing. Today "Live", "Pause" and "Follow" are three identical plain labels.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Follow looks clearly different on vs off (toggle styling plus icon), with a tooltip saying what it does
- [ ] While paused the control reads "Resume" and is visibly highlighted; lines arriving while paused are counted ("Resume · 42 new")
- [x] The connection label carries an icon/colour per state (live, connecting, reconnecting, error) and is styled as status, not as a button identical to Pause/Follow
- [x] Test in the GPUI harness for each state's label
- [x] Live: Toggle Follow and Live: Toggle Pause have default Shortcuts, shown in the controls' tooltips

## Comments

- 2026-10-23: Re-checked: "Pause" and "Follow" still look the same on and off (only Pause changes its word, to "Paused"). Live: Toggle Follow and Live: Toggle Pause have no default Shortcut either — added a criterion. The connection label also showed "Connecting (2 of 2)" for local files, see issue 17. Still open — P1.
- 2026-10-23: Done, except the count of lines arriving while paused. Follow reads "Following"/"Follow" with an arrow icon and a tooltip; Pause reads "Pause"/"Resume" with a pause/play icon, turns warning-coloured while paused, and says what it does in its tooltip. Both tooltips name their Shortcut: Live: Toggle Follow is now `⌥⌘F` and Live: Toggle Pause `⌥⌘P` by default. The connection label has an icon and colour per `SourceStatus::tone()` (Good/Busy/Warning/Bad). Not done: "Resume · 42 new" — a paused source stops reading, so the app does not know how many lines wait; that needs the sources to report the size of the file while paused. Tests in `tests/live_controls_test.rs`; the keymap tests that used Toggle Follow as a Command without a Shortcut now use Focus Pane: Move to Window.
