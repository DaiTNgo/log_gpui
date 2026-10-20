# 02: A Filter change does not rescan Search Matches or highlights

**What to build:** Ticking a Filter Value, toggling Hide Mode, typing a Custom Value or changing the Time Range re-evaluates only the Filter and refreshes the Focus Pane. Since ADR 0012 Search matches the whole view, so Matches, the Match counter, highlight paints and Highlight Rule counts cannot change on a Filter change and must not be recomputed. The single "refresh everything" step is split into a "rows changed" path (keeps today's behaviour) and a "Filter changed" path.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 01

**Status:** ready-for-agent

- [ ] A Filter change leaves Matches and highlight paints identical (same `Arc`, not recomputed) and the Focus Pane shows the right rows.
- [ ] Row-changing events (load, Live append, rebuild) still refresh everything.
- [ ] A test proves Matches survive a Filter change unchanged and that Focus Pane rows follow the Filter.
- [ ] The ticket 01 probe shows the Filter toggle and Time Range timings dropping; numbers appended under `## Comments`.
