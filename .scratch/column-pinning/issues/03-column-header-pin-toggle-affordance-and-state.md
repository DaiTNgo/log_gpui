Status: ready-for-agent

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

- [ ] All data columns start in the unpinned state by default upon application load.
- [ ] Hovering over the `TIMESTAMP`, `LEVEL`, or `MODULE` column headers displays the pin toggle icon.
- [ ] Clicking an unpinned column's toggle marks it as pinned and activates the persistent accent icon.
- [ ] Clicking a pinned column's toggle releases the pin and returns the icon to the unpinned hover/idle state.
- [ ] The `MESSAGE` column header does not show a pin toggle.
- [ ] Pin toggle clicks do not propagate into column sorting or row selection.

## Blocked by

- .scratch/column-pinning/issues/01-prefactor-deprecate-word-wrap-and-anchor-gutter.md
