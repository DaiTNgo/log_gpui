Status: ready-for-agent

# Interactive Column Resizing & Palette Reset Command

## Parent

.scratch/column-pinning/PRD.md

## What to build

Provide interactive manual column resizing and keyboard reset facilities.

Each Column Header renders a subtle **Column Resize Handle** along its right edge with a `col-resize` cursor. Dragging the handle adjusts the target column's width in real time while enforcing the `min_width` safety floor (preventing columns from collapsing into an unclickable zero-width state). Double-clicking the resize handle resets that column to its auto-fit width based on the longest ingested entry.

Integrate an `UnpinAllColumns` action into the Command Palette (`⌘K`), titled `Columns: Unpin All`, which releases all active pins and returns all columns to the unpinned default state.

## Acceptance criteria

- [ ] Column headers render a draggable resize handle at their right border.
- [ ] Dragging the handle expands or narrows the column width smoothly.
- [ ] Dragging cannot shrink a column below its safety floor `min_width` (~36–48px).
- [ ] Double-clicking the resize handle recalculates and snaps the column back to its optimal auto-fit width.
- [ ] Command Palette (`⌘K`) displays the `Columns: Unpin All` action.
- [ ] Triggering `Columns: Unpin All` via Command Palette releases all pinned columns and re-renders the canvas in default scrollable state.

## Blocked by

- .scratch/column-pinning/issues/04-sticky-horizontal-scrolling-and-partition-divider.md
