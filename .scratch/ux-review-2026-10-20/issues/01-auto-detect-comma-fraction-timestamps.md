# 01: Auto-detect reads timestamps with a comma fraction (logback, log4j, Python logging)

**What to build:** A line such as `2026-10-20 10:00:08,123 ERROR [main] c.a.OrderService - failed to place order` is read by Auto-detect: it gets a TIMESTAMP and a LEVEL, the stack trace lines below it become its Continuation Lines, `⌘E` jumps to it, and its file is interleaved by timestamp with the other files of the Log Space instead of being flagged "No timestamps" and placed last. Today the built-in layout only accepts a `.` before the fraction, although reading the time itself already accepts a comma.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] A logback/log4j default line (`yyyy-MM-dd HH:mm:ss,SSS LEVEL [thread] logger - msg`) shows its timestamp and level in the Parsed View
- [ ] A Python logging default line (`yyyy-MM-dd HH:mm:ss,SSS - name - LEVEL - msg`) shows its timestamp and level
- [ ] `at …` lines and the exception line under such a line are Continuation Lines of it (inherit its level and timestamp)
- [ ] Jump to next error finds these ERROR lines; Level Markers appear in the Gutter
- [ ] The file is no longer flagged "No timestamps" and interleaves with other files of the Log Space
- [ ] The text shown stays exactly the stored text (rule *Parsed Text Is Stored Text*)
- [ ] Parser tests cover both layouts, with and without a fraction
