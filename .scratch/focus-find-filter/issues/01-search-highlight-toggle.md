# 01: Search Highlight toggle

**What to build:** The user can turn off the painting of Search Matches in the Focus Pane, since every row there already matches the Search and the painting can be noise. The toggle is a button in the Focus Pane header, a switch in Settings next to the Search match color, and a Keymap command `focus_pane.toggle_search_highlight` with no default shortcut. The state is app-wide: flipping it in any Focus Tab or Focus Pane (including one moved to its own window) applies everywhere, defaults to on, and is remembered across launches like the other app-wide view settings. It applies to both Parsed View and Raw View. It does not touch the Canvas, Highlight Rules, or Focus Find painting (Find matches and the current-hit outline stay visible when Search Highlight is off).

**Blocked by:** None (can start immediately)

**Status:** resolved (focus-find-filter-integration)

- [x] Header button, Settings switch and Keymap command all flip the same app-wide state, and each reflects the current state
- [x] Off: Search Matches are not painted in the Focus Pane, in Parsed View and Raw View
- [x] Off: Focus Find matches and current-hit outline still paint; Canvas match painting and Highlight Rules are unchanged
- [x] Defaults to on; a session saved before this feature loads with it on
- [x] State survives an app restart
- [x] **Search Highlight** is defined in CONTEXT.md
- [x] A note on `.scratch/search-keyword-colors/issues/04-focus-pane-colors.md` says this toggle gates per-group colors in the Focus Pane too (no second toggle)
