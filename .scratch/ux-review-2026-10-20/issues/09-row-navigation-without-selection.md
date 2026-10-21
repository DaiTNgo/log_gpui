# 09: Moving between rows by keyboard without selecting one first

**What to build:** Arrow keys work as soon as the view has focus: with no selected row, ↓/↑ select the first/last row in sight. PageUp/PageDown move the selection by one screen, Home/End go to the first/last row (End turns Follow back on in a live Log Space).

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] ↓ with no selection selects the first visible row; ↑ the last visible row
- [ ] PageUp/PageDown move by the number of rows in sight and keep the selection in view
- [ ] Home/End jump to first/last row; End re-enables Follow on a live Log Space
- [ ] None of these fire while typing in a text field
- [ ] Harness tests for each key
