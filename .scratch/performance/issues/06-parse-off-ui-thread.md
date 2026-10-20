# 06: Live lines are parsed off the UI thread

**What to build:** Sources hand the app already-parsed entries, so a burst of Live lines (a fast writer, catching up after a reconnect, a large Load more) never stalls a frame. Parsing state stays per file and sequential, so results are identical. The UI pump's per-tick budget is time-based (around 8 ms) instead of a fixed 200k lines.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] Parsed entries (level, time, Continuation Lines, Detected Fields) are identical to today's for local files, Remote Files and Remote Commands (existing tests pass).
- [ ] Log Format and Extraction Rule changes still re-parse correctly.
- [ ] A burst of 1M lines keeps the UI responsive (frames continue while the backlog drains).
