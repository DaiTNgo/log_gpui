# 01: Auto-detect reads timestamps with a comma fraction (logback, log4j, Python logging)

**What to build:** A line such as `2026-10-20 10:00:08,123 ERROR [main] c.a.OrderService - failed to place order` is read by Auto-detect: it gets a TIMESTAMP and a LEVEL, the stack trace lines below it become its Continuation Lines, `⌘E` jumps to it, and its file is interleaved by timestamp with the other files of the Log Space instead of being flagged "No timestamps" and placed last. Today the built-in layout only accepts a `.` before the fraction, although reading the time itself already accepts a comma.

**Blocked by:** None (can start immediately).

**Status:** resolved

- [x] A logback/log4j default line (`yyyy-MM-dd HH:mm:ss,SSS LEVEL [thread] logger - msg`) shows its timestamp and level in the Parsed View
- [x] A Python logging default line (`yyyy-MM-dd HH:mm:ss,SSS - name - LEVEL - msg`) shows its timestamp and level
- [x] `at …` lines and the exception line under such a line are Continuation Lines of it (inherit its level and timestamp)
- [x] Jump to next error finds these ERROR lines; Level Markers appear in the Gutter
- [x] The file is no longer flagged "No timestamps" and interleaves with other files of the Log Space
- [x] The text shown stays exactly the stored text (rule *Parsed Text Is Stored Text*)
- [x] Parser tests cover both layouts, with and without a fraction

## Comments

- 2026-10-23: Re-checked in the release build with two files of `2026-10-23 09:00:00,814 INFO  [db.pool] …` lines: both are flagged "No timestamps", TIMESTAMP/LEVEL/MODULE are empty and the whole line sits in MESSAGE. The same files with `.` before the fraction parse fully (levels, Level Markers, Continuation Lines, interleaving). Still the worst first impression for Java/Python logs — P0.
- 2026-10-23: Done. The built-in layout (`RE_STANDARD` / `RE_STANDARD_NO_LEVEL` in `src/parser/text.rs`) takes `.` or `,` before the fraction, so logback/log4j lines read their time, level and `[thread]` as MODULE. Python logging's default (`<time> - <logger> - <LEVEL> - <msg>`) has its own layout, `RE_PYTHON`, used only when the word in the level slot is a level. The shown text is unchanged (spans into the stored line). Tests in `tests/parser_test.rs`; checked in the release build: two logback files show TIMESTAMP/LEVEL/MODULE, Level Markers and Continuation Lines, interleave, and lose the "No timestamps" flag.
