# 08: Empty canvas explains Log Spaces, and load errors show in place

**What to build:** With no Log Space open, or a Log Space with no files, the canvas shows an icon, a heading and one sentence saying a Log Space groups log files read as one timeline, plus the existing action (e.g. Add files). When a Source File fails to load, the canvas shows that error in place instead of looking like an empty log; the Notice bar still shows it too.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness).

**Status:** resolved

- [x] "No Log Space open" and "This Log Space has no files" each have an icon, heading, supporting sentence and their action.
- [x] A Source File that couldn't be found or read shows its error on the canvas.
- [x] Wording uses CONTEXT.md terms (Log Space, Source File).
- [x] Harness tests: which empty or error state is shown for no Log Space, an empty Log Space, and a missing Source File.
- [x] GUI smoke for the icon and layout.

## Comments

- 2026-10-19: Done. `EmptyCanvas` (No Log Space / No files / Load failed) is decided by `AppView::empty_canvas`, which now also picks the canvas versus the empty state. Each state has an icon (Layers, FolderOpen, TriangleAlert in the danger color), a heading, a supporting sentence and its action (New Log Space, or Add files for an Imported Log Space).
- A file that can't be read is cached as `ParsedFile::failed(error)`. When every Source File of the view failed (one file for a File View), the canvas lists the errors; the Notice bar still shows them.
- Not covered: a Log Space where only some files failed still shows its rows, and the failure only in the Notice bar.
- Tests: `tests/empty_canvas_test.rs` (1 pure, 4 harness, incl. an unreadable file). GUI smoke: screenshot of the No Log Space state with icon, heading, sentence and button.
