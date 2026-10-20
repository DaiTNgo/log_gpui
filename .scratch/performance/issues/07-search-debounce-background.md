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
