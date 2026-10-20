# 01: Baseline timings for Live, Search and Filter at 1M lines

**What to build:** A maintainer can run one probe that loads a synthetic Log Space of about 1M timestamped lines with a few Highlight Rules and an active Search, then prints wall-clock timings for: one Live append batch (e.g. 1,000 new lines), one Search keystroke, one Filter Value toggle, one Time Range change, and one Live Window trim. Extend the existing perf probe example rather than adding a new tool. The numbers are recorded in this ticket so tickets 02-07 can show before/after.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** done

- [ ] The probe drives the same code paths the app uses (the real refresh after an append, the real Search update), not reimplementations.
- [ ] It runs in a release build with one command and prints one line per measured operation.
- [ ] Baseline numbers from this machine are appended to this ticket under `## Comments`.

## Comments

Probe: `cargo run --release --example perf_probe` (the `app …` lines drive the real `AppView` through `pump_live`, `trigger_search_query`, `filter_toggle_value` and `apply_filters_and_refresh`; a fake Remote File source sends the Append events, and the Live Window is lowered by 10% to force a trim). 1M timestamped lines, 2 Highlight Rules, Search `user=42` active. Baseline, Apple silicon, release build, 2026-10-20:

```
app Live append batch (1,000 lines)        137.82ms
app Search keystroke ('user=4')             17.48ms
app Filter Value toggle (level=error)       64.38ms
app Time Range change (From)                65.74ms
app Live Window trim (~100k lines)         452.58ms
```
