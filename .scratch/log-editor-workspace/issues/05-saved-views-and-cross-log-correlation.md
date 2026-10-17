Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Implement project-scoped **Saved Views** and **Cross-Log Correlation** to accelerate recurring investigative workflows across multiple microservice streams.

### Saved Views
Engineers frequently construct complex filter chains (e.g. `level = ERROR`, `status >= 500`, `duration_ms > 2000`) and column arrangements while diagnosing issues. Saved Views enable saving these configurations as named presets within the Project:
- Users can click "Save View As..." in the filter popover or toolbar to save the current filter conditions, query, and column state.
- Saved Views appear in the `Saved Views` drawer (`IconName::Sliders`).
- Clicking any saved view applies its filter pipeline and column setup to the currently active log tab in one click.
- Views can be renamed, duplicated, or removed.

### Cross-Log Correlation
Modern distributed systems link async operations via request/trace identifiers. When inspecting a log entry containing a `request_id` or `trace_id` (either in structured fields or detected via regex):
- The Inspector and row context menu expose a "Search Across All Project Files" action.
- The search engine evaluates all registered project logs in the background for occurrences of that trace identifier.
- Tabs in the Tab Bar display visual match count badges indicating where correlated events were found (e.g. `worker.log ●3`).
- A "Synchronize Timestamp" toggle in the status bar allows optionally syncing scroll positions across tabs based on nearest timestamp alignment ($O(\log N)$ binary search).

## Acceptance criteria

- [ ] Users can save the current tab's active filter conditions, search query, and column layout as a named Saved View.
- [ ] Saved Views are listed in the `Saved Views` drawer and can be applied to any open log tab.
- [ ] Users can edit, rename, and delete Saved Views within a project.
- [ ] Clicking a `request_id` or `trace_id` allows executing a cross-file correlation search across all registered project logs.
- [ ] Tabs in the Tab Bar display visual match badges when correlated trace events are found in their corresponding log streams.
- [ ] Clicking a correlated tab navigates directly to the matching trace log event.
- [ ] An optional timestamp synchronization toggle aligns tab scroll positions to equivalent chronological points.
- [ ] Headless unit tests verify view serialization, preset application, cross-file trace search, and timestamp alignment calculations.

## Blocked by

- .scratch/log-editor-workspace/issues/01-multi-tab-working-set-bar.md
- .scratch/log-editor-workspace/issues/03-activity-rail-and-contextual-drawer.md
