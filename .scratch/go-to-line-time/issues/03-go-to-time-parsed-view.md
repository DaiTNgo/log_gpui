# 03: Go to Time in the Parsed View

**What to build:** Add the Keymap command `navigate.go_to_time` (`Navigate: Go to Time…`, default `cmd-shift-l`). It opens the same Go to bar in Time mode, and running either command while the bar is open switches its mode. The input is read by `filter::parse_time_bound` with `FilterIndex::anchor_day()`, so it accepts exactly what the Time Range accepts, compared as written in the file (L8, L25). The hint shows the earliest and latest time of the view (`FilterIndex::time_hint`). Enter lands on the first row, in view order, whose effective timestamp is at or after the typed time. Continuation Lines count with their Parent Line's time (rule 19), so the landing row is never inside a stack trace. In a merged Log Space this is a binary search over the timeline (rule 22). In a File View with out-of-order times it is the first qualifying row scanning down. A time before the first timed line, or after the last, lands on the first or last timed line with a note. A view with no timed lines disables Time mode and says why.

Add `resolve_time` to `src/go_to.rs`, taking effective timestamps in view order.

**Blocked by:** 01

**Status:** needs-triage

- [ ] `navigate.go_to_time` exists, in the Command Palette and the Keymap; `cmd-shift-l` by default
- [ ] One bar for both modes; the commands switch its mode while it is open
- [ ] Accepts the Time Range's forms (full date-time, time of day on the anchor day, seconds); invalid text gives an inline error
- [ ] Lands on the first row at or after the time; never on a Continuation Line
- [ ] Merged Log Space: correct across files, equal timestamps follow rule 22's order
- [ ] Before / after the range: lands on the first / last timed line with a note
- [ ] No timed lines: Time mode is disabled with a reason
- [ ] Follow turns off; Search, Filter and Time Range are unchanged
- [ ] Unit tests for `resolve_time` (equal times, continuations, untimed rows, non-monotonic order); app test
- [ ] **Go to Time** is defined in CONTEXT.md
