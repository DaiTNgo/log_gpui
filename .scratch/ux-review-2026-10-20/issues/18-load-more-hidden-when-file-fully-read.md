# 18: "Load more" is not offered for a file that is already read whole

**What to build:** A File Section in the Raw View, and the status bar of a one-file Log Space, offer "Load more" only while there are older lines left to read. Today a local file read from its first line shows a dimmed "Load more (10,000)" under its header (`3,245 lines held (≈5 MB)  Load more (10,000)`), which reads as "something is missing but you can't get it".

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] A file held from its first line shows no Load more button (File Section header, file row menu, status bar)
- [x] A file whose older lines left the Live Window still shows Load more
- [x] A file stopped at the Full Read Limit keeps its disabled button with the existing reason in its tooltip
- [x] The "lines held" text stays, so the reader still sees how much is in memory

## Comments

- 2026-10-23: Done. `HoldInfo::offers_load_more()` (not a Remote Command, not held from its first line) decides whether Load more shows at all: in the Raw View File Section row, the status bar of a one-file Log Space, and the menus of local and Remote Files (with its 1,000/10,000/100,000 and custom items). At the Full Read Limit the button stays with its reason. Back to Live Window no longer hides with it: a file that Load more took to its first line still shows it in the status bar. Tests in `tests/local_hold_menu_test.rs`.
