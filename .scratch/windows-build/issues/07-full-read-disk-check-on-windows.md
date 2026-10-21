# 07: Full Read checks free disk space on Windows

**What to build:** Before a Full Read downloads a remote log, Windows checks free space on the destination disk the same way macOS/Linux do, and refuses or warns when the estimate (plus margin) does not fit. Today Windows reports "unknown" free space, so the check is silently skipped.

**Blocked by:** 01 (Windows build goes green on CI)

**Status:** resolved (windows-build-integration)

- [ ] Free bytes for a folder are reported on Windows
- [ ] A Full Read that does not fit is stopped with the same message as on other platforms
- [ ] If free space cannot be read, behaviour stays as today (no false refusals)
