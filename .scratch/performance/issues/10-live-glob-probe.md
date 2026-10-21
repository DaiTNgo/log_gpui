# 10: Live Glob archive detection reads only file headers

**What to build:** The first scan of a large folder by a Live Glob is fast. Detecting whether a matched file is an Archive reads only the few header bytes it needs (today a fresh SFTP file asks for a full ~255 KB chunk), probes several files concurrently, and reuses the SFTP session across scans. Forgetting files that left the folder uses a set lookup instead of a nested scan.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** done

- [x] Probing 500 files transfers on the order of KBs per file, not ~255 KB.
- [x] Archive detection results are unchanged (existing tests pass).
- [x] A folder of 20k entries rescans without quadratic cost.

## Comments

- `HeadProbe` (src/remote/glob_watcher.rs) uses a raw SFTP session (`RawSftpSession`) and reads 512 bytes per file; `SftpSession::File` always asks for a full packet. Up to 16 files are probed at once.
- The watcher keeps its `SftpSession` and `HeadProbe` between scans (`Conn`); dropped on a listing error or when the hub session changes/closes.
- `forget_missing` uses a `HashSet` of present names instead of a nested scan.
- `RemoteSession::open_sftp_stream` added in hub.rs (small, localized) to get the raw channel stream.
- Test: `tests/glob_probe_test.rs` counts bytes over the SFTP stream for 62 files (60 x ~300 KB): under 4 KB per file. Env-gated on `SSH_TEST_DIR`.
