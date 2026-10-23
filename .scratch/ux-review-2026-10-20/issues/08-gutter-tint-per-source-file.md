# 08: The Gutter tints line numbers by Source File

**What to build:** In a Log Space with several files, interleaved line numbers (4240, 2139, 4241…) are readable at a glance: each Source File gets a subtle, stable tint in the Gutter, and the same tint marks the file in the sidebar and the Source Column.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Each file of a Log Space gets a distinct tint from a fixed palette, stable across restarts
- [x] The tint shows in the Gutter, beside the file in the sidebar, and in the Source Column
- [x] A Log Space with one file shows no tint
- [x] Tints keep line numbers readable on the dark theme (contrast checked)

## Comments

- 2026-10-23: Re-checked with two interleaved files: the Gutter reads 3229, 1606, 3230, 1607… with nothing telling the files apart except the Source Column further right. Still open — P1.
- 2026-10-23: Done. Each file of a Log Space of several files takes a tint by its place in the file order (`LogSpace::source_tint_index`, stable across restarts, follows the File Order switch) from eight fixed hues that avoid the Level Marker red and yellow (`ui::canvas::source_tint`, lighter on dark, darker on light). It shows as a 3 px bar left of the line number, as the text colour of the Source Column, and on the file icon in the sidebar; a Log Space of one file and a File View show none, and the line number itself keeps its colour so contrast is unchanged. The Focus Pane rows are not tinted yet. Tests in `tests/log_space_test.rs`; checked in the release build.
