# 12: Local Live files honour the Live Window

**What to build:** Following a local file for hours keeps memory bounded, like a Remote File: the Live Window trims the oldest lines (through ticket 04's in-place trim). New data is read in bounded chunks (a few MB), so a burst or a truncate-and-rewrite of a large file never produces one huge batch. Line splitting avoids the extra copy per line when nothing is pending.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** 04

**Status:** done

- [x] A local file grown far past the Live Window holds only the window's lines; Load more still reaches earlier lines.
- [x] A truncated-then-rewritten large local file is read in chunks.
- [x] CONTEXT.md's Live Window entry is updated if its meaning widens to local files.

## Comments

- `apply_file_events` (src/app/live.rs) now trims every live file, not only remote ones, through `trim_open_view`; `set_live_window` applies a new window to local files too.
- `LocalFileSource::drain` reads at most `CHUNK_BYTES` (4 MB) per `Append` and waits while 4 batches are unread, so a burst or a truncate-and-rewrite of a large file arrives in chunks and never queues the whole file.
- `LineSplitter::push_with_lens` decodes a line straight from the input when nothing is pending (no copy into `pending` first).
- Load More for local files: `AppView::load_more` sends `LoadMore` with the oldest held line's number and hash; `src/live/local_older.rs` finds that line by counting newlines, checks the hash (else `OlderUnavailable`), and reads the lines above with `BackwardLines`. The hold/menu UI (`hold_info`, Clear, Back to Live Window) is still remote-only; Load More for local is reachable through `AppView::load_more` only, no menu item yet.
- A local file opened larger than the window is trimmed at its first appended batch (it was loaded whole before that).
- Tests: `tests/live_local_window_test.rs`.

Menu follow-up: a local file followed live now has Load more, Load more (custom number)… and Back to Live Window in its file menu (Imported Log Spaces) and in the Raw View's actions row, without Clear (`HoldInfo::local`, `local_file_live_items`). `tests/local_hold_menu_test.rs` checks `hold_info` of a local file.
