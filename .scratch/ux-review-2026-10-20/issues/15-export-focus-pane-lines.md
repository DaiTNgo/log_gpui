# 15: Export the lines listed in the Focus Pane

**What to build:** Once Filter and Search have narrowed the Focus Pane to the lines that matter, the user can take them out in one step: "Copy all lines" puts every listed line's stored text (with its Continuation Lines) on the clipboard, and "Save as…" writes them to a `.log` file.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] Actions in the Focus Pane header and the Command Palette
- [x] Output is the stored text in view order, Continuation Lines included, Breaks excluded
- [x] In a Log Space with several files, an option prefixes each line with its Source File
- [x] A notice confirms "Copied 200 lines" / "Saved 200 lines to …"
- [x] Test that the exported text matches the listed lines

## Comments

- 2026-10-23: Done. `AppView::focus_pane_export` gives the stored text of the lines the open Focus Tab lists, in view order, each listed line followed by its Continuation Lines (a stack trace under a matching ERROR line comes along although it does not match); Breaks are not rows, so they are never in it. Copy all lines and Save lines as… are buttons in the Focus Pane header and Commands (Focus Pane: Copy All Lines / Save Lines As…), with "Copied N lines" / "Saved N lines to …". The Source File prefix (`b.log: `) follows the Source switch of the Columns section in a Log Space of several files, rather than a separate option. The header buttons were not checked in the running app. Tests in `tests/focus_export_test.rs`.
