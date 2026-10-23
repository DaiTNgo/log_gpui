# 11: Jump to the next and previous Marker

**What to build:** Like jumping between errors, the user can jump to the next/previous line carrying a Marker, by shortcut and from the Command Palette, wrapping around the view.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Shortcut and palette entries for next and previous Marker (shortcut chosen not to clash with existing ones)
- [x] Jumps select the line and scroll it into view; wraps at the ends
- [x] With no Markers, a brief notice says so
- [x] Harness test

## Comments

- 2026-10-23: Done. Navigate: Jump to Next Marker (`⌥⌘↓`) and Jump to Previous Marker (`⌥⌘↑`) select the next/previous row carrying a Marker, wrapping around the view; with no row selected they start at the first/last row (counting it). With no Marker the notice says "No Markers in this view". Tests in `tests/marker_jump_test.rs`.
