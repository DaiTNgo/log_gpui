# 0001. Unified Horizontal Scroll with Sticky Columns

## Context

In the log viewer canvas, data rows can be extremely long (when word wrap is disabled), requiring horizontal scrolling. Users need to pin specific metadata columns (such as `TIMESTAMP`, `LEVEL`, or `MODULE`) to keep them visible while scrolling through long payloads.

We evaluated two spatial models for horizontal scrolling:
1. **Split-Pane Model**: Splitting the canvas into two distinct containers—a fixed left pane for pinned columns and a scrollable right pane with a scoped scrollbar.
2. **Unified Canvas Model**: A single continuous canvas with a full-width horizontal scrollbar, where pinned columns behave as sticky elements that anchor to the left margin as content scrolls underneath.

## Decision

We chose the **Unified Canvas Model with Sticky Columns**.

All columns maintain their natural schema positions when un-scrolled (`scroll_x = 0`). When the user scrolls horizontally (`scroll_x > 0`), pinned columns remain anchored to the left (following the Gutter) while unpinned columns scroll smoothly underneath. The horizontal scrollbar always spans the full width of the canvas.

## Consequences

- Eliminates jarring layout shifts and scrollbar position jumping when users toggle column pins.
- Preserves the familiar spatial mental model of modern data tables and spreadsheets (like Excel freeze panes and Google Sheets).
- Requires row and header rendering in `canvas.rs` to compute horizontal sticky offsets for pinned columns relative to `scroll_x`.
