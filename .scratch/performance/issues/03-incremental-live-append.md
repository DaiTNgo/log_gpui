# 03: A Live batch processes only the new rows

**What to build:** While Following timestamped files, a batch of appended lines costs time proportional to the batch, not to the whole view. For rows appended at the end of the view, only the new rows are evaluated against the Filter, painted by Highlight Rules (and added to rule counts), searched for Matches, given a delta time, and folded into the longest-row width. The Focus Pane extends with new passing rows. Everything earlier in the view is untouched. Full recomputation stays for structural changes and for Filter/Highlight Rule/Search Expression changes.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 02

**Status:** ready-for-agent

- [ ] After N incremental batches, Filter pass, Matches, highlight paints, rule counts, delta times and longest row equal what a full refresh would produce (test compares both).
- [ ] Selection, Focus Find and the top-of-view anchor behave as before.
- [ ] In a merged Log Space, lines whose time lands before the end of the view still take the correct (existing) path.
- [ ] The ticket 01 probe shows the Live batch time no longer scaling with total rows; numbers appended under `## Comments`.
