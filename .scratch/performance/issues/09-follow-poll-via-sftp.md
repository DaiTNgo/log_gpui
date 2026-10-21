# 09: Following a Remote File polls its size over SFTP

**What to build:** Following many Remote Files no longer spawns a process on the Server every 500 ms per file. The size is polled with SFTP stat on the open handle or path (one request, no process). The inode check that detects rotation via `exec stat` runs only when the size shrinks or stalls, or every few seconds.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** done

- [x] Following 20 files of a Live Glob runs no per-poll process on the Server in steady state.
- [x] Rotation, truncation and Gone files are still detected (existing remote tests pass; add one for rotation detected via the slower inode check).

## Comments

- Each poll is now one SFTP `metadata` request on the path. The inode (`exec stat`) is read when the file is first seen, every 3 s (`INODE_CHECK_INTERVAL`), when the size is smaller than the read offset, and whenever exec stat is unavailable (as before). Between checks the remembered inode is reused. A missing file or SFTP error on the cheap path goes through the same Missing/Lost handling; a 5 s timeout and `is_closed()` make a dead connection count as Lost.
- `RemoteSession::exec_count()` (new, counts `exec` and `exec_stream`) lets a test see processes started on the Server. Tests in `tests/remote_source_test.rs`: a quiet file starts at most 3 processes in 6 s (it was 12); a rotation to a bigger file (size never shrinks) is still found by the slower check.
- Edits in `src/live/remote.rs` are limited to the doc header, two constants, one `Memory` field and `stat()`.
- Note: base 380b05d does not compile on its own (commit 2bce0b7 carries `SourceCommand::Replace` and a `release_unused_server` call from unfinished work). I used throwaway stubs to run tests and did not commit them. Lost-connection tests that kill sshd children with `pkill -P` fail here with and without this change.
