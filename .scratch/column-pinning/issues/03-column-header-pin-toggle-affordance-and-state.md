Status: resolved

# Column Header Pin Toggle Affordance & Pin State Engine

## Parent

.scratch/column-pinning/PRD.md

## What to build

Introduce the state engine for tracking pinned columns and render interactive Pin Toggle controls on each eligible Column Header (`TIMESTAMP`, `LEVEL`, `MODULE`).

The pin state engine tracks active pinned columns in a deterministic collection. The Column Header renders a subtle pin icon with four distinct interaction states:
1. `Unpinned (Idle)`: Icon hidden to keep the minimalist theme clean.
2. `Unpinned (Hover)`: Reveals a muted pin icon (`IconName::PushPinSimple`, `text_muted`) on header hover.
3. `Pinned (Active)`: Persistently visible pin icon highlighted with `text_accent` (`#60a5fa`).
4. `Pinned (Hover)`: Displays an unpin release affordance (`IconName::PushPinSimpleSlash`).

Clicking the toggle toggles the pinned state of the column, storing it in the application's column layout state. The `MESSAGE` column remains permanently unpinnable without a toggle icon.

## Acceptance criteria

- [x] All data columns start in the unpinned state by default upon application load.
- [x] Hovering over the `TIMESTAMP`, `LEVEL`, or `MODULE` column headers displays the pin toggle icon.
- [x] Clicking an unpinned column's toggle marks it as pinned and activates the persistent accent icon.
- [x] Clicking a pinned column's toggle releases the pin and returns the icon to the unpinned hover/idle state.
- [x] The `MESSAGE` column header does not show a pin toggle.
- [x] Pin toggle clicks do not propagate into column sorting or row selection.

## Blocked by

- .scratch/column-pinning/issues/01-prefactor-deprecate-word-wrap-and-anchor-gutter.md

## Comments

Resolved in Issue 03 implementation:
1. Implemented pin state engine on `ColumnLayoutState` in `src/column.rs` (`pin()`, `unpin()`, `toggle_pin()`, `pinned_columns()`, `unpinned_columns()`).
2. Guaranteed schema ordering preservation (`TIMESTAMP` -> `LEVEL` -> `MODULE`) using `BTreeSet` and pre-sorted `ordered_pinned` slice.
3. Enforced four distinct `PinToggleState` interaction states (`UnpinnedIdle`, `UnpinnedHover`, `PinnedActive`, `PinnedHover`) and embedded Phosphor SVG icon data for `PushPinSimple` and `PushPinSimpleSlash`.
4. Wired reactive pin actions and hover state tracking in `AppView` (`toggle_pin_column`, `set_column_header_hovered`, `set_column_toggle_hovered`).
5. Rendered interactive pin toggles on `TIMESTAMP`, `LEVEL`, and `MODULE` headers with `stop_propagation()` on click in `src/ui/canvas.rs`. The `MESSAGE` column remains permanently unpinnable without a toggle.
6. Verified via comprehensive unit tests in `tests/column_test.rs`. Full test suite passes.
