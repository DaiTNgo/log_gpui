# 19: Raw and Parsed View open a Log Space at the same place

**What to build:** Opening the same Imported Log Space lands the reader at the same place whichever view it opens in. Today the Parsed View opens at the newest line (Follow on, rule L3) while the Raw View opens at line 1 of the first File Section, with Follow still shown in the status bar.

**Blocked by:** None (can start immediately).

**Status:** resolved

Needs a decision before building: either the Raw View follows too (each File Section scrolled to its last line while Follow is on), or Follow is off by default in the Raw View and the status bar says so. Record the choice in CONTEXT.md (rule L3) or an ADR.

- [x] A Log Space opened in the Raw View and in the Parsed View shows the same end of the log (per the decision above)
- [x] The Follow control in the status bar matches what the Raw View actually does
- [ ] Switching Raw ↔ Parsed keeps the current line in sight
- [x] GPUI harness test for the opening position in both views

## Comments

- 2026-10-23: Decided by the user: the Raw View follows too. Done. While Follow is on, `RawViewState::scroll_to_end` makes every File Section's editor scroll to its last line on the next render (on opening a live Log Space, on each live batch, and when Follow is turned on); Open in full still opens with Follow off at the first line. Scrolling up in the Raw View turns Follow off (watched in the capture phase, since the editors take the wheel); scrolling back down does not turn it on again — the Follow switch or End does. CONTEXT.md L2 and L3 updated. Not done: switching Raw ↔ Parsed keeping the current line in sight (the view is rebuilt and, with Follow on, lands on the newest lines); left unticked. The stack of File Sections itself is not scrolled, so a lower section can sit below the window. Tests in `tests/raw_follow_test.rs`; checked in the release build.
