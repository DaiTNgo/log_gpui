# 05: Relative offsets in Go to Time (follow-up)

**What to build:** Let Go to Time also take an offset from the current line's time: `+30s`, `-5m`, `+1h30m`. The result lands with the same "first at or after" rule as issue 03. With no current line, an offset is counted from the first timed line. This reuses `remote::starting_time::parse_span` (the Starting Time span syntax, `1h`) with a sign added, so the user learns one way to write spans.

Out of scope for the first cut (see PRD). Triage whether it is worth it once 01–04 are in use.

**Blocked by:** 03

**Status:** needs-triage

- [ ] `+`/`-` spans are parsed with the Starting Time span grammar
- [ ] Counted from the current line's effective time, else the first timed line
- [ ] Unit tests for parsing and resolution
