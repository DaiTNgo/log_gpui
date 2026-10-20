# 12: Local Live files honour the Live Window

**What to build:** Following a local file for hours keeps memory bounded, like a Remote File: the Live Window trims the oldest lines (through ticket 04's in-place trim). New data is read in bounded chunks (a few MB), so a burst or a truncate-and-rewrite of a large file never produces one huge batch. Line splitting avoids the extra copy per line when nothing is pending.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 04

**Status:** ready-for-agent

- [ ] A local file grown far past the Live Window holds only the window's lines; Load more still reaches earlier lines.
- [ ] A truncated-then-rewritten large local file is read in chunks.
- [ ] CONTEXT.md's Live Window entry is updated if its meaning widens to local files.
