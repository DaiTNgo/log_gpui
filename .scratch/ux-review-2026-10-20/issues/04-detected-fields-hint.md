# 04: The view says when Detected Fields are available but hidden

**What to build:** When a Log Space's lines have Detected Fields (JSON keys, `key=value` pairs) and none is turned on, the Parsed View shows a small, dismissible hint such as "3 fields detected: status, latency_ms, path" that opens the Columns section on its *Detected* group. Detected Fields stay off by default; this only makes them discoverable.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] The hint appears only when Detected Fields exist and none is shown
- [ ] It names up to a few fields by line count and says how many more there are
- [ ] Clicking it opens the Columns section scrolled to the *Detected* group
- [ ] It disappears once any Detected Field is turned on, or when dismissed (dismissal kept per Log Space across restarts)
- [ ] Test in the GPUI harness: JSON Log Space shows the hint; turning one field on hides it
