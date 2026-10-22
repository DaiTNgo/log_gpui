# 03: Find Filter keeps the user's place and tolerates half-typed input

**What to build:** Toggling the Find Filter never loses the user's place. Switching it on while the current row is filtered out moves to the nearest kept row (the closest one above, else the closest one below). Switching it off keeps the current row and scrolls so it sits in the middle of the pane, with the restored rows around it. While the find text is an invalid Search Expression (e.g. mid-typing), filter mode keeps narrowing with the last valid matcher, as Focus Find already does for its hits, so the list never flashes to empty or to everything; the existing red border and "!" tag still show.

**Blocked by:** 02

**Status:** resolved (focus-find-filter-integration)

- [x] On, current row filtered out: selection moves to the nearest kept row above, else below
- [x] On, current row kept: selection stays
- [x] Off: the current row stays selected and is centered in the pane
- [x] Invalid find text in filter mode: rows stay as narrowed by the last valid matcher; error indicator shows
