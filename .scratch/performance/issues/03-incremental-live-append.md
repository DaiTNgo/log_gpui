# 03: A Live batch processes only the new rows

**What to build:** While Following timestamped files, a batch of appended lines costs time proportional to the batch, not to the whole view. For rows appended at the end of the view, only the new rows are evaluated against the Filter, painted by Highlight Rules (and added to rule counts), searched for Matches, given a delta time, and folded into the longest-row width. The Focus Pane extends with new passing rows. Everything earlier in the view is untouched. Full recomputation stays for structural changes and for Filter/Highlight Rule/Search Expression changes.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 02

**Status:** done

- [ ] After N incremental batches, Filter pass, Matches, highlight paints, rule counts, delta times and longest row equal what a full refresh would produce (test compares both).
- [ ] Selection, Focus Find and the top-of-view anchor behave as before.
- [ ] In a merged Log Space, lines whose time lands before the end of the view still take the correct (existing) path.
- [ ] The ticket 01 probe shows the Live batch time no longer scaling with total rows; numbers appended under `## Comments`.

## Comments

`append_to_open_view` calls the new `apply_rows_appended` when the new rows all sort after the last row; it extends the Filter pass (`FilterState::evaluate_from`), delta times, longest row, highlight paints and rule counts, Matches (`SearchState::extend_matches`), Focus Pane rows and Focus Find hits (`FocusFind::extend_hits`). Rows sorting before the end still take `apply_filters_and_refresh`. `AppView::refreshes` counts which path ran. `tests/live_incremental_test.rs` compares the result after each batch with a full refresh. Filter changes no longer rewrite delta times (`FilterState::pass`).

Probe, 1M lines (the probe now scans all rows once before the Filter toggle: the first scan after a batch that only touched the new rows pays tens of ms for bringing the old rows' memory back in):

```
                                   before      after
app Live append batch (1,000)     137.82ms     6.5ms
app Filter Value toggle            12.15ms    12.7ms
app Time Range change              12.74ms    13.4ms
```
