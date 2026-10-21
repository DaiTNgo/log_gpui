# 08: Sequential SFTP reads keep their read-ahead

**What to build:** Full Read, Tail, Backfill to the Live Window, Load more and the Starting Time search fetch only the data they use. In russh-sftp 3.0.1 every `seek` (even to the current position) discards the 16 pipelined read requests, each a full ~255 KB chunk the Server still sends. Sequential readers seek only when the offset actually differs (e.g. after a reconnect). Random-access readers (backward blocks, timestamp probes) request exactly the bytes they need, e.g. with a session limited to one concurrent read or a raw read of the exact length.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** done

- [x] Full Read of a large Remote File reaches pipelined throughput: the full-read measuring example shows bytes transferred close to file size and a clear speed-up; numbers appended under `## Comments`.
- [x] Backward reads and timestamp probes transfer roughly the requested size.
- [x] Remote live tests (local sshd setup) still pass.

## Comments

Full Read (`read_plain`) now tracks where its file is and seeks only when the next offset differs (after a reconnect), so the 16 pipelined reads survive. Random-access readers use a second kind of session, `RemoteSession::open_sftp_random` (one request in flight, packets of at most 64 KB): the Starting Time / Time Range search of a Full Read, Open in Full's size estimate and listing, and the Live source's tail, backfill, Load more and `count_since` (its compressed Rotated Files decode on a pipelined `open_sftp` session of their own). `RemoteHub::sftp_bytes()` counts bytes received on SFTP channels; `tests/sftp_read_test.rs` asserts transfer for Full Read, a Starting Time search, timestamp probes and a Live tail + backfill. Tail polling of the Live source runs on the random-access session too, so a large backlog after a rotation is read 64 KB at a time (slower on high latency; a small tail is unaffected).

`cargo run --release --example measure_full_read` (2,000,000 lines, 217 MB, loopback sshd):

```
                                   before      after
Full Read time                     35.5 s      11.1 s
SFTP bytes received                3440 MB     217 MB
Full Read speed                    6 MB/s      20 MB/s
```

Test transfers (180,000 lines, 18 MB file): Starting Time search near the end 19.6 MB before (more than the file) and under 2.5 MB after; Live tail + 8,000 lines backfill 5.4 MB before, under 2 MB after.

Note: the base `perf-integration` did not compile (an earlier commit left `SourceCommand::Replace` unhandled in `live/{local,remote,command}.rs` and a call to a missing `release_unused_server` in `app/live.rs`); this branch adds no-op arms and drops that call.
