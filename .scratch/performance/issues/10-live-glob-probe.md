# 10: Live Glob archive detection reads only file headers

**What to build:** The first scan of a large folder by a Live Glob is fast. Detecting whether a matched file is an Archive reads only the few header bytes it needs (today a fresh SFTP file asks for a full ~255 KB chunk), probes several files concurrently, and reuses the SFTP session across scans. Forgetting files that left the folder uses a set lookup instead of a nested scan.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Probing 500 files transfers on the order of KBs per file, not ~255 KB.
- [ ] Archive detection results are unchanged (existing tests pass).
- [ ] A folder of 20k entries rescans without quadratic cost.
