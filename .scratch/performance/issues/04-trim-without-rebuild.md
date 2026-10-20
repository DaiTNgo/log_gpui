# 04: A Live Window trim of a Remote File does not rebuild the view

**What to build:** When the Live Window drops the oldest lines of a Followed Remote File, those rows are removed from the front of the view and its filter index in place. No clone of the Log Space, no timeline re-merge, no re-run of Extraction Rules over every row. Scroll position, selection, Markers and Matches stay correct (indices shift by the number of dropped rows). The Raw View of that File Section updates without re-joining every line.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] A test Follows a Remote File past its Live Window and checks the view, Filter pass, Matches and selection equal a from-scratch build.
- [ ] In a merged Log Space only the trimmed file's rows leave; other files' rows remain in order.
- [ ] The ticket 01 trim timing drops to roughly the cost of the removed rows; numbers appended under `## Comments`.
