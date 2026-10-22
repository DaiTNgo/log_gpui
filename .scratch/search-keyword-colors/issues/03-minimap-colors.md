# 03: Minimap marks colored per Phrase

**What to build:** Minimap ticks for Matching lines show one vertical stripe per distinct Phrase color found on the line, in expression order. A single Phrase gives a single color; if the tick is too narrow for stripes, it takes the color of the first Phrase. The stripe set comes from the Phrases that occur on the line, not from which ranges survive overlap resolution: a Phrase fully covered by a longer one still gets its stripe. The current Match stays clearly marked. With the Settings toggle off, ticks use the old single color.

**Blocked by:** 01, 02

**Status:** resolved

- [ ] Single-Phrase lines show that Phrase's color
- [ ] Multi-Phrase lines show stripes, or the first Phrase's color when too narrow
- [ ] A Phrase fully covered by a longer one still gets its stripe
- [ ] Current Match remains distinguishable
- [ ] Toggle off restores the old tick color
