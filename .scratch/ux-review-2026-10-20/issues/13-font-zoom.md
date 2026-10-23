# 13: Zooming the text with ⌘+ / ⌘− / ⌘0

**What to build:** The user can make the log text larger or smaller in steps, independently of Density, and reset it with `⌘0`. Column widths follow the new size; the size is kept across restarts.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] `⌘+`/`⌘=` and `⌘−` change the size in steps within sane bounds; `⌘0` resets
- [x] Parsed View, Raw View and Focus Pane all follow; column widths recompute
- [x] The setting persists and appears in Settings › Appearance
- [x] Harness test that widths change with size

## Comments

- 2026-10-23: Done. `density::TextScale` (Density + zoom steps, −3…+8, one point each; rows grow in proportion) is what the Parsed View, the Focus Pane, column widths and the Raw View editors read. View: Larger Text (`⌘=`, `⌘+`, `⌘⇧=`), Smaller Text (`⌘−`) and Reset Text Size (`⌘0`) are Commands; the zoom is kept as an app setting (`text_zoom` in the session) and Settings › Appearance has a Text size row (A− / size / A+ / Reset). On the way: column widths of a live Log Space come from the defaults or the saved layout, not from measuring, so a size change scales a column with nothing measured in proportion (`ColumnLayoutState::fitted_font`), and a layout restored at another size is scaled when its tab opens. Tests in `tests/text_zoom_test.rs` and `src/density.rs`; checked in the release build at +3.
