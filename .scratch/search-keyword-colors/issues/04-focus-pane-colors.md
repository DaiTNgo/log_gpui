# 04: Focus Pane Search and Focus Find colored per Phrase

**What to build:** The Focus Pane paints Search Matches with the same Phrase colors as the Canvas, and Focus Find uses the same rules (each distinct Phrase is a color, same hash and hue separation) and the same color for the same Phrase: Focus Find takes Search's already-assigned hue for a Phrase present in both, and separates only the rest. With the Settings toggle off, both use the old single color.

**Blocked by:** 01, 02

**Status:** resolved

- [ ] Search Matches in the Focus Pane use the Canvas Phrase colors
- [ ] Focus Find Phrase colors follow the same rules
- [ ] A Phrase used in both Search and Focus Find shows the same color, even when separation would push it differently
- [ ] Toggle off restores the old single color in both

## Comments

- The app-wide **Search Highlight** toggle (`.scratch/focus-find-filter/issues/01-search-highlight-toggle.md`) gates Search Match painting in the Focus Pane, including these Phrase colors. Do not add a second toggle: with Search Highlight off, Search Matches are not painted at all, whatever this ticket's Settings toggle says.
