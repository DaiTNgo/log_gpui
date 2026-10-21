# 01: Windows build goes green on CI

**What to build:** The app builds and its test suite passes on Windows, checked on every push by a `windows-latest` CI job, so Windows breakage is caught instead of discovered. Today the crate does not compile for Windows at all (agent sign-in calls a Unix-socket-only russh API), the checked-in Cargo config points at macOS-only paths, and several tests use Unix-only APIs and tools.

Scope:
- Add a CI workflow that runs build and test on Windows (and keeps macOS/Linux covered).
- Stop committing the machine-specific Cargo config (macOS CA bundle path, `DEVELOPER_DIR`); move it to per-machine config and `.gitignore` it.
- Gate Unix-only tests (Unix permissions, `pkill`/`kill`, `ssh-agent`, `gzip`, hard-coded `/tmp`) so they skip on Windows rather than fail to compile.
- Agent sign-in on Windows: until ticket 05 lands, connecting a Server with Agent auth fails with the existing "agent unavailable" error instead of not compiling.
- Document how to cross-check from a Mac (`llvm-rc` from Homebrew LLVM for gpui's resource step, `cargo-xwin` for linking).

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

- [ ] A Windows CI job runs build and the test suite and is green
- [ ] No macOS-only Cargo config is tracked in git; a fresh Windows clone fetches crates without extra setup
- [ ] Unix-only tests are skipped on Windows, not removed elsewhere
- [ ] On Windows, Agent sign-in reports "agent unavailable" (no panic, no compile error)
- [ ] Cross-compile instructions from macOS are written down in the repo docs
