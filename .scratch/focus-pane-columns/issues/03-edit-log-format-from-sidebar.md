# 03: Edit a Log Format from the left sidebar

**What to build:** Right-clicking a Log Space row or a file row in the left sidebar offers "Log Format…". It opens the Log Format editor for the format that applies, with a live Preview of the columns. If no format applies, it opens the editor on a new format, filled in from a sample line of the file. A submenu in the same place chooses another Log Format. A format chosen for a file overrides the one chosen for its Log Space.

**Blocked by:** None (can start immediately)

**Status:** resolved (branch `focus-pane-columns`)

- [ ] "Log Format…" appears on Log Space rows and on file rows, not on glob rows
- [ ] With a format applied, the editor opens on that format; saving it changes the columns of the open view
- [ ] With no format applied, the editor opens on a new format with a sample line filled in
- [ ] The submenu lists the Log Formats and the choice applies to the Log Space or to the file
- [ ] A file's own choice wins over its Log Space's choice
