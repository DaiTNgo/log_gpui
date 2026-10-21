# 15: Export the lines listed in the Focus Pane

**What to build:** Once Filter and Search have narrowed the Focus Pane to the lines that matter, the user can take them out in one step: "Copy all lines" puts every listed line's stored text (with its Continuation Lines) on the clipboard, and "Save as…" writes them to a `.log` file.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Actions in the Focus Pane header and the Command Palette
- [ ] Output is the stored text in view order, Continuation Lines included, Breaks excluded
- [ ] In a Log Space with several files, an option prefixes each line with its Source File
- [ ] A notice confirms "Copied 200 lines" / "Saved 200 lines to …"
- [ ] Test that the exported text matches the listed lines
