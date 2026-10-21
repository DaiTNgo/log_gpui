# Building for Windows

CI (`.github/workflows/ci.yml`) builds and tests on `windows-latest` and
`macos-latest` on every push; `ubuntu-latest` runs too but is best effort.

## Cargo config is per machine

`.cargo/config.toml` is not in git (it holds macOS-only paths: the CA bundle
and `DEVELOPER_DIR`). On a Mac that needs them, copy
`.cargo/config.macos.toml.example` to `.cargo/config.toml`. A Windows or Linux
clone needs no Cargo config.

## What differs on Windows

- Agent sign-in: russh only talks to an ssh-agent over a Unix socket, so on
  Windows an Agent Server fails with "ssh-agent not available at ..." (the
  `AgentUnavailable` error). The Windows named-pipe agent is a later ticket.
- Tests that need Unix tools or APIs (`ssh-agent`, `kill`, file permissions)
  are `#[cfg(unix)]`. The SSH integration tests skip themselves without
  `SSH_TEST_DIR`, as everywhere.
- Tests that compare `/`-separated local paths are `#[cfg(unix)]` until local
  Windows paths are handled.

## Cross-checking from a Mac

Cargo cannot run Windows tests on a Mac, but the crate (and its tests) can be
type-checked for the MSVC target, which catches every "does not compile on
Windows" mistake.

One-time setup:

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked        # downloads the MSVC CRT + Windows SDK on first use
brew install llvm nasm                   # llvm-rc: gpui's resource step; nasm: aws-lc-sys (via russh)
export PATH="$(brew --prefix llvm)/bin:$PATH"
```

(Homebrew's `llvm` is keg-only, hence the `PATH` line. The first `cargo xwin`
run downloads the Windows SDK into `~/Library/Caches/cargo-xwin`.)

Then:

```sh
cargo xwin check --target x86_64-pc-windows-msvc --all-targets
```

`cargo-xwin` also does the linking (`cargo xwin build --target
x86_64-pc-windows-msvc`), but the result cannot be run on a Mac; the real test
run is the Windows CI job.

## Console window

`src/main.rs` sets `windows_subsystem = "windows"` for Windows release builds
(`cfg(all(windows, not(debug_assertions)))`), so the release `.exe` opens no
console. Debug builds keep the console, so `eprintln!` traces such as
`LOG_GPUI_LIVE_TRACE` stay visible. macOS and Linux are unaffected.
