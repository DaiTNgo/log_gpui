# 07: Search and Time Range are debounced, run in the background and cancel stale work

**What to build:** Typing a Search Expression at 1M lines does not lag. The Match scan starts after a short debounce (~100 ms), runs on a background executor, and a newer query drops older results. The Time Range inputs re-filter only when the parsed bounds actually change, after the same debounce. Matches are stored once in a shared list, not copied.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** ready-for-agent

- [ ] Typing quickly produces one scan for the final query; the Match counter shows the final result.
- [ ] A Live batch arriving during a background scan neither loses nor duplicates Matches.
- [ ] Half-typed Time Range text that does not parse causes no re-filter.
- [ ] The ticket 01 Search keystroke timing on the UI thread drops; numbers appended under `## Comments`.
