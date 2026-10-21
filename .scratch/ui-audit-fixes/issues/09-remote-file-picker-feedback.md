# 09: Remote File picker says what it's doing

**What to build:** The Remote File picker tells the person what's happening: "Listing /var/log…" or "Matching *.log…" instead of "Loading…". The old folder's rows are cleared while the next one loads, so nothing stale can be clicked. An empty folder says "This folder is empty". A glob reports what it did ("Added 3 files (1 already chosen)") and the path/glob field keeps its text so it can be tweaked. Folders and files have their own icons.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness).

**Status:** ready-for-agent

- [ ] Loading text names the operation and the path or glob.
- [ ] Starting a new listing clears the previous rows.
- [ ] An empty folder shows "This folder is empty".
- [ ] A glob add reports added and skipped (already chosen) counts.
- [ ] The path/glob field keeps its text after a successful add.
- [ ] Rows show a folder or file icon.
- [ ] Tests drive the picker with a stubbed listing (no real SSH): loading text, rows cleared, empty-folder message, glob counts, field kept. Icons by GUI smoke.
