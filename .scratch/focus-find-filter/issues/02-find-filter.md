# 02: Find Filter narrows the Focus Pane

**What to build:** Focus Find gains a filter mode. A funnel button in the find bar, and a Keymap command `focus_pane.toggle_find_filter` with no default shortcut, switch it on and off. While on and the find input is non-empty, the Focus Pane keeps only the rows that match Focus Find, on top of what Search (and Filter) already selected, in both Parsed View and Raw View. Rows appended while the log grows also go through the Find Filter. The funnel shows an active state whenever the Find Filter is actually narrowing; with empty find text it narrows nothing even when switched on. In filter mode the counter reads "N of M lines" (rows kept / rows before the Find Filter) and the previous/next buttons step through the kept rows.

The Find Filter is deliberately not persisted, so a user coming back never wonders why lines are missing: it is off whenever the app or the Log Space is reopened. Within a running session each Focus Tab keeps its own on/off state across tab switches. The find text itself keeps being saved as today.

**Blocked by:** None (can start immediately)

**Status:** resolved (focus-find-filter-integration)

- [x] Funnel button and Keymap command toggle the Find Filter for the open Focus Tab
- [x] On with non-empty text: only matching rows remain, in Parsed View and Raw View
- [x] On with empty text: no rows are removed and the funnel is not shown as active
- [x] Rows appended to a live/growing log are filtered too, and the counter updates
- [x] Counter reads "N of M lines" in filter mode; previous/next step through kept rows
- [x] Switching Focus Tabs and back keeps each tab's Find Filter state
- [x] Reopening the app or the Log Space starts with the Find Filter off; the find text is restored
- [x] **Find Filter** is defined in CONTEXT.md
