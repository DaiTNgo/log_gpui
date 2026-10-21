# 07: Search and Time Range are debounced, run in the background and cancel stale work

**What to build:** Typing a Search Expression at 1M lines does not lag. The Match scan starts after a short debounce (~100 ms), runs on a background executor, and a newer query drops older results. The Time Range inputs re-filter only when the parsed bounds actually change, after the same debounce. Matches are stored once in a shared list, not copied.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** done

- [ ] Typing quickly produces one scan for the final query; the Match counter shows the final result.
- [ ] A Live batch arriving during a background scan neither loses nor duplicates Matches.
- [ ] Half-typed Time Range text that does not parse causes no re-filter.
- [ ] The ticket 01 Search keystroke timing on the UI thread drops; numbers appended under `## Comments`.

## Comments

Typing in Search (`search_text_changed`) parses the expression at once (it highlights), then looks for the Matches after a 100 ms pause (`SEARCH_DEBOUNCE`), on a worker thread over a copy of the rows' texts; a newer text, a refresh of every row, or a tab change drops the scan (`search_generation`), rows that left or moved while it ran (`rows_epoch`) start it again, and lines appended meanwhile are searched when it lands, so none is lost or counted twice (`SearchState::pending` keeps a Live batch from extending Matches of the text before). The Matches are stored once: `matched_shared` is gone, `matched_visible_indices` is the shared `Arc<Vec<usize>>`. Time Range typing (`on_time_inputs_typed`) re-filters after the same pause, and only when both bounds read and differ from the ones in use; Enter applies at once. `tests/search_debounce_test.rs`; `focus_pane_notice_test` waits out the pause after typing.

Probe, 1M lines (Search `user=4`, 113k Matches; the 100 ms pause itself is not in the second line):

```
                                           before      after
app Search keystroke, UI thread           17.48ms     2.2ms
app Search Matches after the pause            -      48.7ms (worker scan, copy of the texts and applying them)
```

### Follow-up 07b: stale scans stop, and the texts are kept between scans

A scan that a newer query, a full refresh or a tab change made stale used to run to the end and be dropped. `AppView::supersede_search_scan` (replaces every `search_generation += 1`) now also sets a shared `AtomicBool` and swaps in a fresh one for the next scan; the worker runs `par::filter_cancellable`, which looks at the flag every 4096 rows in each thread (small logs too) and gives up with `None`. A cancelled result is never applied (`finish_search_scan` ignores a stale generation; a `None` for the current one starts again). Tests: `tests/par_cancel_test.rs`, the two cancel tests at the end of `tests/search_debounce_test.rs`.

The O(rows) clone of every `raw_line` on the UI thread in `start_search_scan` was 80-100 ms at 1M rows in a loaded probe run, the biggest UI-thread cost of a scan. It is gone: `SearchLines` (`src/search_lines.rs`) keeps the texts in 65,536-row blocks between scans; a full block is shared and immutable so a scan under way keeps reading its snapshot while the next one grows, and only rows appended since the last scan (plus the last partly filled block) are copied. Anything that replaces or removes rows must bump `rows_epoch` (the snapshot is then rebuilt); the two row-installing sites in `tabs.rs` now do, live trim and `apply_filters_and_refresh` already did, Live append only extends. A `debug_assert` in `sync` compares the snapshot with the rows in debug builds, so a site that forgets to bump fails the tests. Full suite: 975 passed.

Probe, 1M lines (release, Search `user=4`, 113k Matches; run-to-run noise is large while other builds run):

```
                                                   before (07)   after
app Search keystroke, UI thread ('user=4')         ~2-15ms       ~2ms
app Search Matches after the pause                 48-83ms       ~37ms   (first scan: builds the snapshot)
app Search keystroke, UI thread ('user=45')        -             ~2ms
app Search Matches after the pause (next query)    -             ~13ms   (after a 1,000-line Live batch)
```
