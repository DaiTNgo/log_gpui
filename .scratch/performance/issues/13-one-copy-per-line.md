# 13: Each line's text is stored once

**What to build:** Memory for a large Log Space drops because a line's text exists once and is shared, instead of separately in the file's lines, the raw line, the payload and a cloned entry in the view. Payload, time and tag become ranges into the shared text; field spans are built without per-line string maps; parsing allocates far less per line. This touches the entry type used everywhere: sequence it expand-contract (add the shared form beside the old, migrate readers in batches, then remove the old fields), each step staying green.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 06

**Status:** done (see Comments for what was not done)

- [x] The memory probe shows a clear drop in RSS for the same 1M-line file; numbers appended under `## Comments`.
- [x] Rendering, Search, Filter, Extraction Rules and Raw View output are unchanged (full test suite passes).
- [ ] Split into expand / migrate / contract tickets before starting if it does not fit one context window.

## Comments

Done in the expand / migrate / contract order, each commit green (`cargo test`):

1. Expand + migrate: `LogEntry::time_str()`, `tag()`, `level_text()`, `payload()` and setters; every reader (UI cells, Filter index, columns, correlation, tests) moved off the fields.
2. Contract: the four `SharedString` fields are gone. A parsed entry reads them from `raw_line` through its `spans` (`timestamp`, `module`, `level`, `message`), so a line's text exists once; `entry.payload()` points inside `raw_line` (`tests/entry_text_sharing_test.rs`). Entries made by hand (`LogEntry::new`, a setter, an event not read from its line) keep their own texts in `own: Option<Arc<OwnTexts>>`; `from_event` sets it only when the line's stretches differ from the event's texts (a whitespace-only line), so output is unchanged. `sizeof LogEntry` 328 -> 240 (this is also what a view copy of an entry costs).
3. `parse_entries` reads a whole file line by line instead of holding the `LogEvent`s of every line first, and `ParsedFile::lines` (the Raw View's lines, now `Arc<Vec<SharedString>>`) share the entries' `raw_line` instead of a second copy; Live `push_lines` / `insert_above` do the same.

`examples/mem_probe.rs`, 1M-line file (80.9 MB, 5 fields per line), release, RSS growth (before = `perf-integration`):

| | before | after |
|---|---|---|
| parse a file (`ParsedFile::from_bytes`) | 1851 MB (1851 B/line) | 506 MB (506 B/line) |
| `--live`: entry + Raw line + view copy | 1190 MB | 749 MB (probe now shares the Raw line as the app does; with the old probe's separate line copy 918 MB) |
| `sizeof LogEntry` | 328 | 240 |

`examples/perf_probe.rs`, same 1M lines (peak RSS of the whole process from `/usr/bin/time -l`, which holds the text, `ParsedFile`, a clone and the Filter index):

| | before | after |
|---|---|---|
| peak RSS | 3.65 GB | 1.76 GB |
| parse | 2.08 s | 1.90 s |
| app Live append batch (1,000) | 6.12 ms | 5.65 ms |
| Search Matches after the pause | 44.6 ms | 40.2 ms |
| Filter Value toggle | 12.0 ms | 7.5 ms |
| Live Window trim (~100k) | 112 ms | 84 ms |

Not done (left for a follow-up; the three boxes above hold without them):
- `Spans` still allocates an `Arc<[(&str, Range<usize>)]>` per line (about 140 B); a compact form (interned name ids, `u32` ranges) would save roughly half of that.
- The parser still builds a transient `LogEvent` (String copies and two `HashMap`s) per line and converts it; `LineParser::parse`, `reread` and the Live path go through it. Building the entry's spans directly from the regex groups would remove those allocations (time, not RSS, as they are freed per line).
- `request_id` / `trace_id` (`Option<String>`), `markers`, `break_*`, `file_id`, `source` still take about 100 B of every entry's 240 B inline; boxing the rarely set ones would shrink the entry and every view copy further.
