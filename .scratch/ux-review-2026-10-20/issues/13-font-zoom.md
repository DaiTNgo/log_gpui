# 13: Zooming the text with ⌘+ / ⌘− / ⌘0

**What to build:** The user can make the log text larger or smaller in steps, independently of Density, and reset it with `⌘0`. Column widths follow the new size; the size is kept across restarts.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] `⌘+`/`⌘=` and `⌘−` change the size in steps within sane bounds; `⌘0` resets
- [ ] Parsed View, Raw View and Focus Pane all follow; column widths recompute
- [ ] The setting persists and appears in Settings › Appearance
- [ ] Harness test that widths change with size
