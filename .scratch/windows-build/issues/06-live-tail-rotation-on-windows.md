# 06: Live tail detects rotation on Windows

**What to build:** When a tailed local log is rotated on Windows (renamed away and a new file created under the same name), the Live view starts a new Generation and reads the new file from the start — even when the new file is created within Windows' ~15 s "file tunneling" window, which makes it inherit the old file's creation time. Today Windows uses creation time as the file identity, so such rotations are missed unless the new file happens to be shorter than the old read offset.

Identify the file by its volume serial + file index on Windows (the equivalent of an inode).

**Blocked by:** 01 (Windows build goes green on CI)

**Status:** ready-for-agent

- [ ] Rename-then-recreate within a few seconds is detected as rotation on Windows
- [ ] Truncation is still detected as before
- [ ] A test exercises rotation on Windows (runs in the Windows CI job)
