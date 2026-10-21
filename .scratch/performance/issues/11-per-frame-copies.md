# 11: Fewer per-frame copies in the Focus Pane and Raw View

**What to build:** Scrolling stays smooth with many Focus Find hits and in the Raw View of a 1M-line file. Each frame no longer:

- clones the Focus Find hit list or the whole column layout for the Focus Pane;
- rebuilds the Raw View's File Sections 4-6 times (build once per frame, share the line breaks);
- walks every line to locate the caret (use the editor's own lookup, or recompute only when caret/selection changed);
- falls back to scanning every row to find the selected line for the Inspector when the remembered position is stale (keep it in sync after row changes, or index it).

Parent: `.scratch/performance/PRD.md`

**Blocked by:** None (can start immediately).

**Status:** done

- [x] With 500k Focus Find hits, a frame does not copy the hit list.
- [x] In the Raw View of 1M lines with the caret at the end, frame time does not scale with line count.
- [x] Inspector and Focus Pane selection behave as before (existing tests pass).

## Comments

- Focus Pane: `PaneState.find` is a reference and `PaneState.columns` a reference to the view's own layout; the virtualized list reads the Focus Find and the column layout from the `AppView` when it builds rows, so a frame copies neither the hit list nor the layout. `pane_layout()` is the in-place read; `pane_columns()` stays as an unpinned copy, used only to hit-test a click.
- Raw View: `ParsedFile.raw_breaks` is an `Arc<BTreeMap>` (writers use `Arc::make_mut`), so building the File Sections (4-6 times per frame) copies no lines and no breaks; `raw_sections()` is now O(files).
- Caret: `raw_view::line_and_byte_in_rope` finds the caret's line from the editor's rope (log time) in `sync_raw_color_bar` and in the selection observer, replacing the walk over every line.
- Inspector: `entry_of_line_ref` also tries the row its last full scan found (validated each time), so a stale `selected_row_pos` costs one scan, not one per frame. `AppView.entry_scans` counts the scans.
- Tests: `tests/frame_copies_test.rs` (shared breaks and lines, rope lookup equals the walk, one scan for a stale position). The no-copy of the hit list / layout is by construction (references), there is no behavioral seam to assert it.
- Gotcha: the base commit 380b05d does not compile by itself (`release_unused_server` and `SourceCommand::Replace` handling are referenced but not defined; they were swept in half-finished by 2bce0b7). Tests were run with local stubs for those four sites that are not committed here.
- Flaky under load: `live_preparse_test::a_burst_is_applied_over_several_pumps_each_short` (timing, failed once at load average ~50) and `secret_retry_test::wrong_password_reopens_the_dialog_with_an_error` (scheduler determinism); both pass on rerun.
