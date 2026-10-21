# 04: Local Windows paths with backslashes and drive letters

**What to build:** Opening `C:\logs\app.log` (from the file picker, drag-and-drop or an Imported Log Space) shows `app.log` as the file name and `C:\logs` as the folder in the File Section header; glob matching against file names works; an ssh config `Include C:\…` is treated as absolute rather than relative to `~/.ssh`. Remote paths keep using `/` — only local paths change.

**Blocked by:** 01 (Windows build goes green on CI)

**Status:** resolved (windows-build-integration)

Note: touches the same ssh-config reading code as 03; don't run both in parallel on the same branch.

- [ ] Local file names/folders are derived with path-aware splitting, not by splitting on `/`
- [ ] Glob filters match local Windows files by name
- [ ] Absolute Windows paths in ssh config `Include` are read as absolute
- [ ] Remote Server paths still split on `/` on every platform
