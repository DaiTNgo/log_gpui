# 06: Live, Pause and Follow in the status bar show their state

**What to build:** Someone glancing at the status bar can tell whether Follow is on, whether the Log Space is paused, and what the connection is doing. Today "Live", "Pause" and "Follow" are three identical plain labels.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Follow looks clearly different on vs off (toggle styling plus icon), with a tooltip saying what it does
- [ ] While paused the control reads "Resume" and is visibly highlighted; lines arriving while paused are counted ("Resume · 42 new")
- [ ] The connection label carries an icon/colour per state (live, connecting, reconnecting, error) and is styled as status, not as a button identical to Pause/Follow
- [ ] Test in the GPUI harness for each state's label
