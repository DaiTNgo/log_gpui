Status: ready-for-agent

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

- [ ] When scrolling horizontally, pinned columns remain anchored next to the Gutter while unpinned columns slide underneath.
- [ ] Multiple pinned columns always preserve schema order (`TIMESTAMP` $\rightarrow$ `LEVEL` $\rightarrow$ `MODULE`) at the sticky boundary.
- [ ] Partition Divider renders a 1px border and elevation shadow when `scroll_x > 0`.
- [ ] Partition Divider shadow disappears smoothly when scrolled back to `scroll_x == 0`.
- [ ] Column header titles stay perfectly aligned with their corresponding row cells throughout all horizontal scroll positions.
- [ ] Quick-filters on Level badge and Module tag continue to trigger correctly on sticky columns.

## Blocked by

- .scratch/column-pinning/issues/02-dynamic-column-sizing-via-ingestion-scan.md
- .scratch/column-pinning/issues/03-column-header-pin-toggle-affordance-and-state.md
