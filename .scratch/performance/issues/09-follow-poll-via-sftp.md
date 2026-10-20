# 09: Following a Remote File polls its size over SFTP

**What to build:** Following many Remote Files no longer spawns a process on the Server every 500 ms per file. The size is polled with SFTP stat on the open handle or path (one request, no process). The inode check that detects rotation via `exec stat` runs only when the size shrinks or stalls, or every few seconds.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Following 20 files of a Live Glob runs no per-poll process on the Server in steady state.
- [ ] Rotation, truncation and Gone files are still detected (existing remote tests pass; add one for rotation detected via the slower inode check).
