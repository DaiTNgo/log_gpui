# 13: Each line's text is stored once

**What to build:** Memory for a large Log Space drops because a line's text exists once and is shared, instead of separately in the file's lines, the raw line, the payload and a cloned entry in the view. Payload, time and tag become ranges into the shared text; field spans are built without per-line string maps; parsing allocates far less per line. This touches the entry type used everywhere: sequence it expand-contract (add the shared form beside the old, migrate readers in batches, then remove the old fields), each step staying green.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 06

**Status:** ready-for-agent

- [ ] The memory probe shows a clear drop in RSS for the same 1M-line file; numbers appended under `## Comments`.
- [ ] Rendering, Search, Filter, Extraction Rules and Raw View output are unchanged (full test suite passes).
- [ ] Split into expand / migrate / contract tickets before starting if it does not fit one context window.
