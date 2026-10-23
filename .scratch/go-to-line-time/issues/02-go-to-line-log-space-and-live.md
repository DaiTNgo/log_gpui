# 02: Go to Line in multi-file and live Log Spaces

**What to build:** Make Go to Line correct when a line number alone is ambiguous. In a Log Space of two or more files, the bar names the target file and lets the user pick another file of the Log Space. The default target is the current line's file, else the first file in the Log Space's file order (rule 29). The `1–N` hint follows the chosen file. For a rotated live file, the line is looked up in the current line's Generation when it belongs to the target file, otherwise in the newest Generation (rule L5). A line that has left the Live Window (L12), or lies beyond what has been read so far, shows "line N is no longer / not yet in the Live Window" and does not jump. A jump turns Follow off (L2), and a paused Log Space stays paused.

**Blocked by:** 01

**Status:** needs-triage

- [ ] Bar shows the target file in a Log Space of 2+ files; the user can switch it to any file of the Log Space
- [ ] Default target: the current line's file, else the first file in file order
- [ ] Hint and clamping follow the chosen file
- [ ] Rotated file: the current line's Generation is used when it is in the target file, else the newest
- [ ] Line outside the Live Window: message, no jump, no silent substitute
- [ ] Jumping turns Follow off; Pause is unchanged
- [ ] App test with a two-file fixture Log Space; live test on the existing live harness
