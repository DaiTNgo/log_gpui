# 02: Windows release build opens no console window

**What to build:** Launching the release `.exe` shows only the app window — no black console window beside it. Debug builds keep the console so `eprintln!` traces (e.g. `LOG_GPUI_LIVE_TRACE`) stay visible.

**Blocked by:** 01 (Windows build goes green on CI)

**Status:** resolved (windows-build-integration)

- [ ] Release build on Windows uses the GUI subsystem; no console appears on launch
- [ ] Debug build on Windows still attaches a console
- [ ] macOS/Linux builds are unaffected
