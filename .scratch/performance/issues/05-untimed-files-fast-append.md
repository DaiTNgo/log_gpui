# 05: Files without timestamps use the fast Live append

**What to build:** A Log Space containing a file with no timestamps (for example a plain-text Remote Command) no longer rebuilds the whole view on every Live batch. New lines of an untimed file are appended in file order through the same incremental path as ticket 03.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** done

- [ ] Following a Remote Command with plain-text output in a Log Space with timestamped files performs no full rebuild per batch.
- [ ] The resulting row order matches what a full rebuild would produce (test).

## Comments

`append_to_open_view` no longer gives up when `no_timestamp_files` is not empty: rows are keyed `(class, time, file)`, with class 1 for a file with no time (its rows come last, file by file), so the batch's rows go where a full merge puts them. `FilterIndex::append_marking_untimed` keeps those rows without a time. A line with a time in such a file still rebuilds. `AppView::refreshes.rebuilds` counts rebuilds; `tests/live_untimed_test.rs` checks no rebuild per batch and the same rows as a rebuild (the untimed file last or first in file order, and alone). New timed lines go before the untimed rows, so those batches refresh the view with `apply_filters_and_refresh` rather than the append path of ticket 03; batches of the untimed last file take the append path.
