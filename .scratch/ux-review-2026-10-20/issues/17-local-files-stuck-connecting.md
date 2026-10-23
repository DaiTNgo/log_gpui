# 17: Local files of an Imported Log Space never leave "Connecting"

**What to build:** An Imported Log Space of local files reports its files as live as soon as they are open. Today the status bar shows "Connecting (2 of 2)" for two local files and keeps showing it, in both the Parsed and the Raw View, so the reader thinks the app is stuck or is trying to reach a Server.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

Seen 2026-10-23 in the release build: a session seeded with `examples/seed_session` (two local files, no Server) shows "Connecting (2 of 2)" on launch and still does after several seconds, while every line is already shown. `LocalFileSource::run` sends `SourceStatus::Live` with `try_send` once the file is open (`src/live/local.rs:118`), but the status never reaches the label; the cause (dropped event, consumer not yet attached, or the label reading another state) is not yet known — diagnose before fixing.

- [ ] Opening an Imported Log Space of readable local files shows the live state (not "Connecting") as soon as the lines are shown
- [ ] A local file that does not exist yet still shows "Waiting for file"
- [ ] The status reaches the label even when it is sent before the view starts listening
- [ ] Test (GPUI harness or source test) that a freshly opened local file ends in the live state
