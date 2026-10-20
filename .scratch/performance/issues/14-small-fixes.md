# 14: Small performance fixes

**What to build:** A handful of independent cheap wins:

- Building the filter index looks up a field's column with a map instead of scanning the column list per field per row.
- A Live buffer keeps a running total of held bytes instead of summing every line when asked (it can be reached from render).
- Jumping to a Match from the ruler uses binary search over the sorted Matches.
- Quitting waits for remote sessions with an async timer, not a blocking sleep on the main thread.
- A Live batch that touches no file of the open view does not re-render the whole app, unless something shown (sidebar status, counts) changed.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Each item has unchanged behaviour (existing tests pass).
- [ ] The sidebar still updates Live status for background files after the notify change.
