# 14: Small performance fixes

**What to build:** A handful of independent cheap wins:

- Building the filter index looks up a field's column with a map instead of scanning the column list per field per row.
- A Live buffer keeps a running total of held bytes instead of summing every line when asked (it can be reached from render).
- Jumping to a Match from the ruler uses binary search over the sorted Matches.
- Quitting waits for remote sessions with an async timer, not a blocking sleep on the main thread.
- A Live batch that touches no file of the open view does not re-render the whole app, unless something shown (sidebar status, counts) changed.

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** done

- [x] Each item has unchanged behaviour (existing tests pass).
- [x] The sidebar still updates Live status for background files after the notify change.

## Comments

- Filter index: `FilterIndex` keeps a `HashSet<FieldKey>` of fields that have a column (kept in `build_merged`, `rebuild_fields`, `push_row`) instead of scanning `columns` per field per row.
- Live buffer: `byte_lens` is now a small `ByteLens` (Vec plus running total); `held_start` reads `total()` instead of summing. Unit test `byte_lens_tests`.
- Ruler jump: `jump_to_match_by_entry` uses `binary_search` on the sorted `matched_visible_indices`.
- Quit: the on-quit wait uses `cx.background_executor().timer(20ms).await` instead of `thread::sleep`.
- Live batch: `FileDelta.shown` is set for every event but `Inode`. A batch whose files are not in the open view only calls `cx.notify()` when some delta has `shown`, appended, structural or trimmed set, so the sidebar still updates status and counts of background files.
