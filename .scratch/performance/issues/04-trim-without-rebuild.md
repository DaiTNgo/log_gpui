# 04: A Live Window trim of a Remote File does not rebuild the view

**What to build:** When the Live Window drops the oldest lines of a Followed Remote File, those rows are removed from the front of the view and its filter index in place. No clone of the Log Space, no timeline re-merge, no re-run of Extraction Rules over every row. Scroll position, selection, Markers and Matches stay correct (indices shift by the number of dropped rows). The Raw View of that File Section updates without re-joining every line.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** done

- [ ] A test Follows a Remote File past its Live Window and checks the view, Filter pass, Matches and selection equal a from-scratch build.
- [ ] In a merged Log Space only the trimmed file's rows leave; other files' rows remain in order.
- [ ] The ticket 01 trim timing drops to roughly the cost of the removed rows; numbers appended under `## Comments`.

## Comments

`AppView::trim_open_view` (`src/app/live_trim.rs`) takes the rows of the trimmed file older than its first kept line out of the view, the filter index (`FilterIndex::drop_rows`), the pass, paints, rule counts, Matches, Focus Pane rows, Focus Find hits and selection, and fixes the delta times and Break at the new first row; `apply_deltas` then adds the batch's new lines at the end. The Raw View's editors cut `Trimmed::text_bytes` from the start of their text (`pending_trims`) instead of being built again. A trim that left a Continuation Line without its Parent Line first still rebuilds. `tests/live_trim_test.rs` compares a single-file and a two-file Log Space with a rebuild after each trim, and the Raw View's text with the lines held.

Probe, 1M lines, trim of ~100k:

```
                                   before      after
app Live Window trim              452.58ms    129.9ms
```
About 100 ms of what is left is `Vec::drain` moving the 900k rows that stay (once in the file's cache, once in the view); a trim happens once per 10% of the window, not per batch.
