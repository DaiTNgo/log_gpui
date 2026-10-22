# 04: Columns section has switches only

**What to build:** The Columns section keeps only the switches. The Log Format picker leaves it (its new home is ticket 03). Dragging to reorder columns goes away. Renaming a column goes away, so the Column Label is removed from the Column Header menu, from the Project's column settings, and from the term list. Labels already saved in a session are ignored when it is read.

**Blocked by:** 01, 03

**Status:** resolved (branch `focus-pane-columns`)

- [ ] The Columns section shows a switch per column and nothing else per row
- [ ] No Log Format picker in the Columns section
- [ ] Columns have a fixed order by source: built-in, from Log Format, then Detected
- [ ] No "Rename" anywhere for a column
- [ ] An older session with saved labels opens without error and shows the original column names
