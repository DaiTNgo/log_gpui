Status: needs-triage

# PRD: Go to Line and Go to Time

## Problem Statement

An investigation usually starts from a coordinate someone else hands over: "the error at 14:32:05", "line 18 422 of `api.log`", a stack trace that names a line. Today the only ways to get there are scrolling, Search for a piece of the text, or a Time Range. Scrolling hundreds of thousands of rows is slow and imprecise. Search needs text the user does not have yet. A Time Range *narrows the Focus Pane* (ADR-0008), so it hides the lines around the moment instead of showing them, and the user has to clear it again afterwards.

Nothing in the app moves the view to a known line number or a known time while leaving every line in place.

## Solution

Two Keymap commands open a small **Go to** bar above the view:

- **Go to Line** (`navigate.go_to_line`, default `cmd-l`) takes a line number and moves the view to that line of one file.
- **Go to Time** (`navigate.go_to_time`, default `cmd-shift-l`) takes a time in any form the Time Range accepts and moves the view to the first line at or after that time.

On Enter the target becomes the current line of the open Focus Tab, exactly as a Search jump does: the view selects it, centres it and flashes it; the Focus Pane selects it when it lists it; Follow turns off (rule L2). Escape closes the bar without moving. Neither command changes Search, Filter, Time Range or which lines are shown, and nothing typed in the bar is remembered across restarts.

Both commands work in the Parsed and the Raw View, in a File View and in a Log Space of any kind (Imported, Scratch, Remote).

## User Stories

1. As a developer given "line 18422 of api.log", I want to press `cmd-l`, type `18422` and land on that line, so that I read it with its surroundings at once.
2. As a developer, I want the bar to show the range of valid line numbers of the target file as a hint, so that I know what I can type.
3. As a developer who types a number past the last line, I want to land on the last line with a note saying the file has fewer lines, so that a typo or a truncated file does not leave me nowhere.
4. As a developer who types something that is not a positive whole number, I want an inline error and no jump, so that I can fix it without losing what I typed.
5. As a developer in a Log Space of several files, I want Go to Line to target the file of the current line, so that line numbers mean what they mean in that file (rule 34: one row per stored line, numbers match the file).
6. As a developer in a Log Space of several files, I want to pick another file of the Log Space from the bar, so that I can go to a line of a file I am not in yet.
7. As a developer following a rotated live file, I want Go to Line to target the Generation of the current line, falling back to the newest Generation, so that a number matches the file on disk (rule L5).
8. As a developer on a Remote File, I want a line that has left the Live Window to be reported as gone rather than silently replaced by a nearby line, so that I know to Load more or Open in full (rule L12).
9. As a developer told "it broke at 14:32", I want to press `cmd-shift-l`, type `14:32` and land on the first line at or after 14:32:00, so that I see what happened from that moment on.
10. As a developer, I want Go to Time to read times in the same forms as the Time Range (full date and time, time of day on the first day of the view, plain seconds), so that I learn one format.
11. As a developer, I want the bar to show the earliest and latest time of the view as a hint, so that I know which day a bare time of day refers to.
12. As a developer who types a time before the first line or after the last, I want to land on the first or last timed line with a note, so that I am never left without a result.
13. As a developer in a merged Log Space, I want Go to Time to land on the first row of the merged timeline at or after the time, whichever file it comes from (rule 22).
14. As a developer, I want Continuation Lines to count with their Parent Line's time (rule 19), so that I never land in the middle of a stack trace.
15. As a developer in the Raw View, I want Go to Time to search the File Section of the current line, so that it respects that the Raw View shows each file on its own.
16. As a developer with a file whose lines have no time, I want Go to Time to say the file has no times instead of jumping somewhere arbitrary.
17. As a developer, I want to compare times as written in the file, with no timezone or clock correction (rules L8, L25), so that the time I type is the time I see in the column.
18. As a developer, I want the target line to flash like other jumps, so that my eye finds it.
19. As a developer, I want both commands in the Command Palette and rebindable in the Keymap, so that they behave like every other command.
20. As a developer, I want jumping to keep Search, Filter, Time Range and Focus Tabs untouched, so that going somewhere never changes what I was looking for.

## Implementation Decisions

- **One bar, two modes.** A single Go to bar with a mode switch (Line / Time) opened by either command; switching commands while it is open switches its mode and keeps it open. Line and Time stay separate commands because a plain number is a valid time in the Time Range grammar (seconds), so one field cannot tell them apart.
- **Pure resolution module.** A new `src/go_to.rs` holds the logic with no GPUI dependency:
  - `parse_line(text) -> Result<usize, GoToError>`
  - `resolve_line(file_line_count, n) -> (line, Option<Note>)`
  - `resolve_time(rows_in_view_order, t) -> Result<(pos, Option<Note>), GoToError>` taking each row's effective timestamp (`FilterIndex::effective_timestamp`, which already applies rule 19).
  This is the main test seam.
- **Time parsing reuses `filter::parse_time_bound`** with `FilterIndex::anchor_day()`, so Go to Time and Time Range accept identical input.
- **"First at or after" is in view order**, not "nearest". The merged Log Space is sorted, so this is a binary search there. A File View whose times are not monotonic is scanned in order, and the first qualifying line wins, which matches what a reader scrolling down would meet first.
- **Landing goes through `AppView::set_current_line(LineRef, Origin::Jump, cx)`**, which already selects, scrolls, flashes and syncs the Focus Pane in both views. Go to adds no scroll code of its own.
- **Line identity is `LineRef { file_id, line, generation }`.** Target file: the current line's file, else the first file in the Log Space's file order (rule 29). Target Generation: the current line's when it is in the target file, else the newest.
- **Keymap.** Two new `Command` variants, `GoToLine` and `GoToTime`, ids `navigate.go_to_line` / `navigate.go_to_time`, titles `Navigate: Go to Line…` / `Navigate: Go to Time…`, defaults `cmd-l` / `cmd-shift-l` (both currently unbound).
- **Not persisted.** The bar's text and mode live only while the window is open.
- **CONTEXT.md** gains **Go to Line**, **Go to Time** and a rule that a jump never changes what is shown.

## Testing Decisions

- Unit tests on `src/go_to.rs`: parsing (whitespace, `0`, negative, huge, non-numeric), clamping past the end, time before/after the range, equal timestamps, continuation rows, untimed rows, non-monotonic File View order.
- App tests in `tests/go_to_test.rs`, in the style of `tests/find_filter_test.rs`: open a Log Space from fixtures, dispatch the command, type, press Enter, and assert the open Focus Tab's current `LineRef`, `selected_row_pos`, and that Search and Filter state are unchanged. Cover the Raw View and a two-file Log Space.
- A live test on the existing live harness for Follow turning off and for a line that left the Live Window.

## Out of Scope

- Relative jumps (`+30s`, `-5m` from the current line). Tracked as a follow-up issue so it can be triaged separately.
- Jumping by Match index, by Marker, or by Focus Tab position.
- Any timezone or clock-skew handling (rule L8).
- Persisting a history of Go to targets.
- Narrowing anything. That remains Filter's and Time Range's job.

## Further Notes

- Default keys: `cmd-g` is already Next Match, so the VS Code convention (`ctrl-g`) is avoided. `cmd-l` follows Xcode's "Jump to Line".
- Open question for triage: should Go to Time in a Log Space also accept a file (like Go to Line does) to search one file's own clock? The default here uses the merged timeline only.
