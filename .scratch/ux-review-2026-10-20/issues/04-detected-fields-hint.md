# 04: The view says when Detected Fields are available but hidden

**What to build:** When a Log Space's lines have Detected Fields (JSON keys, `key=value` pairs) and none is turned on, the Parsed View shows a small, dismissible hint such as "3 fields detected: status, latency_ms, path" that opens the Columns section on its *Detected* group. Detected Fields stay off by default; this only makes them discoverable.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] The hint appears only when Detected Fields exist and none is shown
- [x] It names up to a few fields by line count and says how many more there are
- [ ] Clicking it opens the Columns section scrolled to the *Detected* group
- [x] It disappears once any Detected Field is turned on, or when dismissed (dismissal kept per Log Space across restarts)
- [x] Test in the GPUI harness: JSON Log Space shows the hint; turning one field on hides it

## Comments

- 2026-10-23: Done, except scrolling to the *Detected* group. `AppView::detected_fields_hint` shows a bar over the Parsed View ("7 fields detected: order, reason, pool and 4 more", fields with the most lines first) while a Log Space has Detected Fields and none is on. Clicking it reveals the Columns section; its × sets `LogSpace::detected_hint_dismissed`, kept in the session. The context sidebar has no scroll-to-section yet, so the Columns section opens but is not scrolled to *Detected*. Tests in `tests/detected_fields_hint_test.rs`; checked in the release build.
