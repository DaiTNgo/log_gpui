Status: resolved

# Sticky Horizontal Scrolling & Schema Order Partitioning

## Parent

.scratch/column-pinning/PRD.md

## What to build

Implement the core spatial layout engine described in ADR 0001. Connect horizontal scroll offset (`scroll_x`) to the rendering of Column Headers and virtualized log rows.

When `scroll_x == 0`, all columns render in their natural schema sequence (`Gutter` $\rightarrow$ `TIMESTAMP` $\rightarrow$ `LEVEL` $\rightarrow$ `MODULE` $\rightarrow$ `MESSAGE`).

When `scroll_x > 0`:
- Pinned columns remain anchored (sticky) immediately to the right of the Gutter.
- If multiple columns are pinned (e.g. both `MODULE` and `TIMESTAMP`), they are strictly laid out in their natural schema order (`TIMESTAMP` before `MODULE`), regardless of the sequence in which the user clicked their Pin Toggles.
- Unpinned columns and the message payload scroll horizontally underneath the anchored partition.
- A vertical **Partition Divider** appears at the right boundary of the sticky columns, featuring a subtle border and an elevation shadow (`shadow-sm`) while content scrolls underneath.
- Row cell click handlers (such as clicking on Level to add a level filter, or Module to filter by tag) remain fully operational when columns are sticky.

## Acceptance criteria

- [x] When scrolling horizontally, pinned columns remain anchored next to the Gutter while unpinned columns slide underneath.
- [x] Multiple pinned columns always preserve schema order (`TIMESTAMP` $\rightarrow$ `LEVEL` $\rightarrow$ `MODULE`) at the sticky boundary.
- [x] Partition Divider renders a 1px border and elevation shadow when `scroll_x > 0`.
- [x] Partition Divider shadow disappears smoothly when scrolled back to `scroll_x == 0`.
- [x] Column header titles stay perfectly aligned with their corresponding row cells throughout all horizontal scroll positions.
- [x] Quick-filters on Level badge and Module tag continue to trigger correctly on sticky columns.

## Blocked by

- .scratch/column-pinning/issues/02-dynamic-column-sizing-via-ingestion-scan.md
- .scratch/column-pinning/issues/03-column-header-pin-toggle-affordance-and-state.md

## Comments

Resolved in Issue 04 implementation:
1. Implemented spatial layout engine methods on `ColumnLayoutState` in `src/column.rs` (`pinned_width()`, `sticky_partition_width()`, `partition_divider_offset()`, `is_partition_divider_active()`, `sticky_column_left()`, `natural_column_left()`, `natural_message_left()`, `column_screen_position()`, `message_screen_position()`).
2. Pinned columns strictly preserve schema ordering (`TIMESTAMP` -> `LEVEL` -> `MODULE`) at the anchor boundary regardless of toggle sequence.
3. Column headers in `src/ui/canvas.rs` render anchored pinned columns at `x = 0` with unpinned columns sliding underneath with `-scroll_x`.
4. Virtualized rows in `src/ui/canvas.rs` render an anchored partition with absolute positioning (`left = scroll_x`) and opaque background (`row_bg`) so unpinned columns slide cleanly underneath.
5. Vertical Partition Divider renders 1px border (`ZED.border_variant`) and elevation shadow (`shadow_sm()`) when `scroll_x > 0`, smoothly reverting to `shadow_none()` when `scroll_x == 0`.
6. Quick-filters on Level badge and Module tag trigger correctly on both sticky and scrolling column cells.
7. Verified with comprehensive unit tests in `tests/column_test.rs`. Full test suite passes (28 tests).
