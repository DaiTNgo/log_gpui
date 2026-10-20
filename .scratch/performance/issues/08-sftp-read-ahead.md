# 08: Sequential SFTP reads keep their read-ahead

**What to build:** Full Read, Tail, Backfill to the Live Window, Load more and the Starting Time search fetch only the data they use. In russh-sftp 3.0.1 every `seek` (even to the current position) discards the 16 pipelined read requests, each a full ~255 KB chunk the Server still sends. Sequential readers seek only when the offset actually differs (e.g. after a reconnect). Random-access readers (backward blocks, timestamp probes) request exactly the bytes they need, e.g. with a session limited to one concurrent read or a raw read of the exact length.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Full Read of a large Remote File reaches pipelined throughput: the full-read measuring example shows bytes transferred close to file size and a clear speed-up; numbers appended under `## Comments`.
- [ ] Backward reads and timestamp probes transfer roughly the requested size.
- [ ] Remote live tests (local sshd setup) still pass.
