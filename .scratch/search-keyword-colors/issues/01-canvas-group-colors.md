# 01: Search Matches colored per Phrase in the Canvas

**What to build:** Typing a Search Expression such as `error && "error code"` paints the Matches in the Canvas cells in two colors: `error` has one, `"error code"` another. The unit of color is each distinct Phrase; `||`, `&&` and parentheses never affect color (`a && b || c` has three colors, and `error || (error && x)` has two because `error` repeats). Color is generated, not taken from the Highlight Palette: hue comes from a hash of the Phrase (case-folded), so the same Phrase always gets the same color in every tab and every session. Hues of distinct Phrases that land too close (under about 30 degrees) are pushed apart in order of first appearance; past about 12 distinct Phrases, the farthest hue is taken and closeness is accepted. Lightness and saturation are fixed per light/dark theme so text stays readable. The Match background is the Phrase color at moderate opacity with the text color unchanged. The current Match uses the same color, stronger, plus the outline. Where Phrases overlap in a line, the longer Phrase's color wins (tie: earlier in the expression). Every Phrase occurrence on a Matching line is painted; lines failing the whole expression paint nothing. Update CONTEXT.md (Match).

**Blocked by:** None (can start immediately)

**Status:** resolved

- [ ] `a && b || c` yields three Phrase colors; identical Phrases in one expression share one color
- [ ] Same Phrase gives the same color across tabs and restarts, case-blind; nothing is persisted
- [ ] Close hues of distinct Phrases are separated; behavior past about 12 Phrases is defined and tested
- [ ] Colors readable in light and dark themes
- [ ] Overlap between Phrases resolves to the longer Phrase
- [ ] Current Match is visibly stronger than other Matches
- [ ] Unit tests cover per-Phrase colors, hash stability, hue separation, overlap
- [ ] CONTEXT.md updated
