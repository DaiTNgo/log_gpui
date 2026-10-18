# 0005. Filter Is Built From Detected Columns; Remove Saved Views

_Partly superseded by ADR-0008: Search and Filter now narrow the Focus Pane, and the view always shows every line._

## Context

Filter was a list of rows (`level`/`module`/`message`/`request_id` with operators such as `is`, `contains`, `regex`) evaluated through a shared query engine. The `module` list in the View panel was hard-coded, so the UI did not reflect the file that was open. `SavedView` bundled filter, search and column layout into a project-wide preset, which made "View" mean three different things. A quick menu on level and module values offered "search this value".

## Decision

**Filter** is built from the file itself. When a file is loaded, its columns and their distinct values are detected. For each **Filterable Column** (at most 500 distinct values, never `timestamp` or `message`/payload) the user ticks **Filter Values** shown as checkboxes, or adds a **Custom Value** (contains, no regex). Values that differ only by letter case are one checkbox. A per-column **Hide Mode** turns "show only these" into "hide these". `timestamp` has its own **Time Range** (From/To). Columns above 500 distinct values are shown dimmed with their count and the reason.

Within one column the checked and custom values combine with OR; columns and the Time Range combine with AND. A row with no value in a column never matches it.

Each log file has its own Filter and Markers, restored automatically when the file is reopened. There are no project-level presets.

We therefore remove: `SavedView`, the View panel (density, pin buttons, module focus), the quick menu on level and module values, and Filter's regex and field-operator rows. Column pinning stays on the Column Header, width stays adjustable, and density moves to settings.

Filter gets its own small model instead of mapping onto the old query AST. With only equality and contains, one OR per column and one AND across columns, the general query engine is unnecessary.

## Considered Options

- **Keep the fixed `level`/`module`/`message`/`request_id` rows**: rejected, the lists do not reflect the open file and custom JSON fields are unreachable.
- **No cap on distinct values**: rejected, a checkbox list for `request_id` or `message` is unusable and slow.
- **Rename `SavedView` to Saved Filter**: rejected, users want a filter per file, not a project-wide preset.
- **Counts that update with other filters (faceted search)**: deferred, it needs a rescan on every click.

## Consequences

- Filter is only as good as column detection. A column that is not detected cannot be filtered on.
- Old saved filters (field/operator rows) cannot be migrated faithfully and are dropped when an old session is loaded.
- `src/query/` and the shared `QueryEngine` can be removed once nothing references them.
- Filter and Search share one Unicode string-matching helper, so `ĐÃ` matches `đã` in both.
