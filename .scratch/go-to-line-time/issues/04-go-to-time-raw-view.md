# 04: Go to Time in the Raw View

**What to build:** Go to Time in the Raw View searches one File Section, because the Raw View shows each file on its own: the section of the current line, else the first section. The landing line is the first line of that file at or after the time, with the same Continuation and edge rules as issue 03. A collapsed File Section is expanded to show it. A scratch file edited since it was parsed has no reliable times (see `inspector_lines`), so for it Time mode says "times are read again when you switch to the Parsed View" and does not jump.

**Blocked by:** 03

**Status:** needs-triage

- [ ] Raw View: searches the current line's File Section (else the first), lands and flashes there
- [ ] A collapsed File Section is expanded to reveal the target
- [ ] Edited scratch file: explanatory message, no jump
- [ ] App test in the Raw View with two File Sections
