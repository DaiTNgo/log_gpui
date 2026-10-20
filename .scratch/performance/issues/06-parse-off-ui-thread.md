# 06: Live lines are parsed off the UI thread

**What to build:** Sources hand the app already-parsed entries, so a burst of Live lines (a fast writer, catching up after a reconnect, a large Load more) never stalls a frame. Parsing state stays per file and sequential, so results are identical. The UI pump's per-tick budget is time-based (around 8 ms) instead of a fixed 200k lines.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 03

**Status:** done

- [ ] Parsed entries (level, time, Continuation Lines, Detected Fields) are identical to today's for local files, Remote Files and Remote Commands (existing tests pass).
- [ ] Log Format and Extraction Rule changes still re-parse correctly.
- [ ] A burst of 1M lines keeps the UI responsive (frames continue while the backlog drains).

## Comments

The app reads big batches ahead on a worker thread, rather than the sources doing it (they stay as they were): a pump takes up to 8,000 lines per file (a longer `Append` is cut, the rest waits in a per-file backlog), and a batch of 1,500 lines or more goes to `PrepareJob::run`, which reads the lines of `Append`, `Tail`, `Backfill` and `Older` events with a copy of the file's parser. When it is done the batch is applied on the UI thread with the entries already made (`LiveBuffer::set_prepared`); the buffer uses them only when numbers, Parent Line and reading (Log Format and Extraction Rules, by identity) are what they were, else it reads the lines again, so the result is the same as before. The file's other events wait while its batch is read. The pump stops taking files after 8 ms (`PUMP_BUDGET`) and comes again 16 ms later while lines are left. `tests/live_preparse_test.rs`: same entries as `ParsedFile::from_text`, ordering of queued batches, a guard for a changed state, and a 200,000-line burst applied over 26 pumps.
