Status: resolved

# Prefactor: Deprecate Word Wrap & Establish Permanent Gutter Anchor

## Parent

.scratch/column-pinning/PRD.md

## What to build

Cleanly remove the legacy word wrap toggle across all layers (toolbar toggle button, application state, and canvas list options) in alignment with ADR 0002. Enforce unconstrained horizontal layout mode as the single default behavior so that log rows are always strictly single-line height, maximizing virtualized list performance.

Anchor the line number and marker Gutter (`#` and `•`) so that when the canvas scrolls horizontally, the Gutter remains stationary at `x = 0` on the left margin, establishing the baseline invariant for horizontal log navigation.

## Acceptance criteria

- [x] The `word_wrap` toggle button is removed from the editor toolbar.
- [x] The virtualized log list always renders in unconstrained horizontal mode (`ListHorizontalSizingBehavior::Unconstrained`).
- [x] Log rows maintain uniform fixed height across all densities without text clipping or line wrapping.
- [x] Horizontal scrollbar spans the full width of the canvas container.
- [x] When scrolling horizontally with the mouse wheel or scrollbar, the line number and marker Gutter remain visible and stationary at the left edge.
- [x] Existing log navigation (arrow keys, search jumps, row clicks) functions identically.

## Blocked by

None - can start immediately

## Comments

Resolved in Issue 01 implementation:
1. Deprecated word wrap across all layers (removed toolbar toggle button `btn_toggle_wrap`, removed `PaletteAction::ToggleWrap` and shortcut `⌥Z`, removed `word_wrap` field from `AppView` and `CanvasViewState`).
2. Enforced unconstrained single-line horizontal sizing (`ListHorizontalSizingBehavior::Unconstrained`, `whitespace_nowrap()`, uniform row heights across densities).
3. Anchored Gutter (`#` and `•`, 88px) permanently at `x = 0` on the left margin across horizontal scroll offsets using `ColumnLayoutState` and absolute positioning with opaque background so unpinned content slides smoothly underneath.
4. Synchronized header data columns shift with row data columns using `-scroll_x`.
5. Unit tested via TDD in `tests/column_test.rs`.
