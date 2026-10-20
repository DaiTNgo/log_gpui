# 05: Files without timestamps use the fast Live append

**What to build:** A Log Space containing a file with no timestamps (for example a plain-text Remote Command) no longer rebuilds the whole view on every Live batch. New lines of an untimed file are appended in file order through the same incremental path as ticket 03.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] Following a Remote Command with plain-text output in a Log Space with timestamped files performs no full rebuild per batch.
- [ ] The resulting row order matches what a full rebuild would produce (test).
