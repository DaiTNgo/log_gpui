# 02: A Filter change does not rescan Search Matches or highlights

**What to build:** Ticking a Filter Value, toggling Hide Mode, typing a Custom Value or changing the Time Range re-evaluates only the Filter and refreshes the Focus Pane. Since ADR 0012 Search matches the whole view, so Matches, the Match counter, highlight paints and Highlight Rule counts cannot change on a Filter change and must not be recomputed. The single "refresh everything" step is split into a "rows changed" path (keeps today's behaviour) and a "Filter changed" path.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 01

**Status:** done

- [ ] A Filter change leaves Matches and highlight paints identical (same `Arc`, not recomputed) and the Focus Pane shows the right rows.
- [ ] Row-changing events (load, Live append, rebuild) still refresh everything.
- [ ] A test proves Matches survive a Filter change unchanged and that Focus Pane rows follow the Filter.
- [ ] The ticket 01 probe shows the Filter toggle and Time Range timings dropping; numbers appended under `## Comments`.

## Comments

Split `apply_filters_and_refresh` (rows changed: unchanged behaviour) from the new `apply_filter_change` (Filter changed: re-evaluate the Filter, refresh the Focus Pane). Tested in `tests/filter_change_test.rs` (same `Arc` for Matches and paints, pane rows follow the Filter). Probe, 1M lines:

```
                                   before      after
app Filter Value toggle           64.38ms    12.15ms
app Time Range change (From)      65.74ms    12.74ms
```
