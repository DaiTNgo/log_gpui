# 08: The Gutter tints line numbers by Source File

**What to build:** In a Log Space with several files, interleaved line numbers (4240, 2139, 4241…) are readable at a glance: each Source File gets a subtle, stable tint in the Gutter, and the same tint marks the file in the sidebar and the Source Column.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Each file of a Log Space gets a distinct tint from a fixed palette, stable across restarts
- [ ] The tint shows in the Gutter, beside the file in the sidebar, and in the Source Column
- [ ] A Log Space with one file shows no tint
- [ ] Tints keep line numbers readable on the dark theme (contrast checked)
