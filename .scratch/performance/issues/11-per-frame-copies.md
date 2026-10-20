# 11: Fewer per-frame copies in the Focus Pane and Raw View

**What to build:** Scrolling stays smooth with many Focus Find hits and in the Raw View of a 1M-line file. Each frame no longer:

- clones the Focus Find hit list or the whole column layout for the Focus Pane;
- rebuilds the Raw View's File Sections 4-6 times (build once per frame, share the line breaks);
- walks every line to locate the caret (use the editor's own lookup, or recompute only when caret/selection changed);
- falls back to scanning every row to find the selected line for the Inspector when the remembered position is stale (keep it in sync after row changes, or index it).

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] With 500k Focus Find hits, a frame does not copy the hit list.
- [ ] In the Raw View of 1M lines with the caret at the end, frame time does not scale with line count.
- [ ] Inspector and Focus Pane selection behave as before (existing tests pass).
