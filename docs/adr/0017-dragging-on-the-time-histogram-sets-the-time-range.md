# 0017. Dragging on the Time Histogram Sets the Time Range

## Context

The Time Histogram shows when lines happened. Users want to narrow to a burst they see on it. The Time Range already narrows the Focus Pane by time (ADR-0008, ADR-0012), typed as text in From and To and read by `parse_time_bound`.

## Decision

A drag on the view's Time Histogram sets the Time Range, and nothing else: on release, the span is snapped to whole columns, written as text in From and To (`HH:MM:SS` when both ends are on the anchor day, `YYYY-MM-DD HH:MM:SS` otherwise, milliseconds added under a one-second column), and applied as typing it would be. Dragging an edge of the shaded range moves that bound, dragging inside moves both, and a click outside it clears it. Editing From or To moves the shading. Zooming the histogram (⌘ + scroll) and clicking a column never narrow anything (rule 41).

## Considered Options

- **A separate "histogram selection" filter**: rejected, two ways to filter by time that could disagree, and a second thing to clear.
- **Store the dragged range as numbers**: rejected, From and To would no longer show the Filter in effect, and the user could not read or edit it.
- **Zoom narrows the view**: rejected, zoom is a way of looking; it must not hide lines.

## Consequences

- The Time Range in From and To is always the one source of truth; the histogram reads it back to draw its shading.
- A drag re-evaluates the Focus Pane once, on release, not while dragging.
