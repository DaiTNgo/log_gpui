# 07: Highlight Rule value list scrolls, and Column Headers never truncate

**What to build:** In the Highlight Rule form, a column with many values can be scrolled and any value picked; today values past the first 160px are cut off. Every Column Header shows in full (e.g. "MODULE", not "MOD…"), even when the column has no values. Respects ADR-0003: this only raises the Dynamic Column Width floor to the header's width.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] The Highlight Rule value list scrolls, using the same pattern as the Filter value list.
- [x] A column's Dynamic Column Width is never narrower than its full Column Header text, including when the column has no values.
- [x] Column width tests (alongside the existing column tests) cover an empty column and a column whose values are shorter than its header.
- [ ] GUI smoke with a seeded session: the Module header shows in full; a column with many values scrolls in the Highlight Rule form.

## Comments

- 2026-10-19: Done. The Column Header label is drawn in the UI font (semibold, `text_xs`), which is wider than the monospace advance the old title floor used, so "MODULE" was cut to "MOD…". `ui::mono::measure` now also measures each header label in that font, and `compute_column_width` floors every column at `header_width(col)` (label + header padding). Before measuring (and in tests) the label width is estimated at 0.72 × font size per character.
- The Highlight Rule value list no longer caps its height at 160px; the panel scrolls, the same as the Filter value list.
- Tests: `column_header_never_truncates` (empty column, values shorter than the header, long values still widen), and the old title-floor assertions now check the header floor. `test_dynamic_width_across_densities` now uses a 5-char level ("ERROR"), since the header floor keeps one size across densities.
- GUI smoke: seeded session shows "MODULE" in full over an empty Module column. The Highlight Rule list scroll was not smoke-tested (it needs clicks); the last box stays open for that.
