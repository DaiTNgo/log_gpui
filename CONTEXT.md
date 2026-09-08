# Log Viewer Domain

Log viewer canvas, virtualized log streaming, and spatial grid layout.

## Language

**Column Header**:
The top-anchored label area of a data column providing title identification, resize handling, and column-level controls.
_Avoid_: Column title, table header.

**Sticky Column**:
A data column that retains its natural position at `scroll_x = 0`, but remains anchored to the left margin (immediately adjacent to the Gutter or preceding sticky columns) when the table is scrolled horizontally.
_Avoid_: Frozen column, locked column, fixed column.

**Pin Toggle**:
The interactive affordance on a Column Header used to anchor or release a Sticky Column.
_Avoid_: Pin button, freeze switch.

**Pin Toggle State**:
- `Unpinned (Idle)`: Hidden to preserve content-as-hero minimalism.
- `Unpinned (Hover)`: Revealed with muted emphasis (`text_muted`) on Column Header hover.
- `Pinned (Active)`: Persistently visible with accent emphasis (`text_accent`).
- `Pinned (Hover)`: Indicated with release affordance (`PushPinSimpleSlash`) for unpinning.

**Gutter**:
The persistent non-scrolling leftmost margin displaying line numbers and row markers, permanently anchored by domain rule.
_Avoid_: Margin, line number bar.

**Unified Canvas**:
The continuous scrollable rendering surface encompassing both sticky and free-scrolling columns, governed by a single full-width horizontal scrollbar.
_Avoid_: Split pane, dual grid.

**Partition Divider**:
The visual boundary (subtle vertical border and scroll shadow) marking the rightmost edge of anchored columns when content scrolls underneath.
_Avoid_: Split line, separator.

**Dynamic Column Width**:
A computed column width derived from the maximum character length of ingested field values and scaled by monospace character advance width, protected from collapse by a minimal `min_width` safety floor with no arbitrary maximum.
_Avoid_: Fixed width, hardcoded width.

**Column Resize Handle**:
The interactive boundary between adjacent Column Headers that supports dragging to resize or double-clicking to trigger Auto-fit.
_Avoid_: Divider, column split.

**Auto-fit**:
The automatic calculation of a column's width to match its longest ingested content based on monospace character advance width, floored by `min_width`.
_Avoid_: Best fit, auto size.

## Rules & Constraints

1. **Default Pin State**: Only the Gutter is anchored by default. All data columns (`TIMESTAMP`, `LEVEL`, `MODULE`, `MESSAGE`) start in their unpinned state.
2. **Pinnability**: Only metadata columns (`TIMESTAMP`, `LEVEL`, `MODULE`) are pinnable. `Gutter` is permanently anchored; `MESSAGE` is permanently unpinnable.
3. **Order Preservation**: When multiple columns are pinned, they always maintain their original schema ordering (`TIMESTAMP` -> `LEVEL` -> `MODULE`) at the anchor boundary, regardless of the sequence in which they were toggled.
4. **Unified Scrolling**: The horizontal scrollbar always spans the full viewport width. Toggling a pin never shifts or resizes the scrollbar element itself.
5. **No Word Wrap**: The canvas operates strictly in unconstrained horizontal mode (`ListHorizontalSizingBehavior::Unconstrained`). Word wrap is permanently deprecated.
6. **Open Dynamic Sizing with Hitbox Safety Floor**: Columns auto-size to fit their longest content without any arbitrary `max_width` ceiling. A minimal `min_width` safety floor (~36–48px) strictly protects the interactive hitbox so columns cannot be collapsed into an unclickable zero-width state.
