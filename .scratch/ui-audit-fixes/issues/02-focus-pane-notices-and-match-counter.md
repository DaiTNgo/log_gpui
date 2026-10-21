# 02: Focus Pane says when nothing matches, and the Match counter tells "no Search" from "no Matches"

**What to build:** When a Search Expression, a Filter or Focus Find leaves the Focus Pane empty, the pane says so and says what to do, instead of going blank. With nothing narrowing it, the pane keeps today's hint. The Match counter shows "–" when the Search Expression is empty and "0/0" only when it found nothing. Follows ADR-0008: Filter narrows the Focus Pane only in the Parsed View.

Parent: `.scratch/ui-audit-fixes/PRD.md`

**Blocked by:** 01 (GPUI test harness).

**Status:** resolved

- [x] Nothing narrowing → today's hint text.
- [x] Search Expression active, zero rows → "No lines match" with the expression shown.
- [x] Filter active in the Parsed View, zero rows → a message that the Filter excludes every line, with a Clear Filter action that works.
- [x] Focus Find active, zero finds → the same no-matches wording for Focus Find.
- [x] Match counter shows "–" for an empty Search Expression and "0/0" for an expression with no Matches.
- [x] The unused Match Navigator component is wired up or deleted, so "No matches" text has one source.
- [x] If Filter can empty the Parsed View canvas itself, the canvas shows the same Filter-excludes-all state; if ADR-0008 means it never can, say so here and skip it.
- [x] Harness tests cover each state above, plus Clear Filter bringing the rows back.

## Comments

- 2026-10-19: Done. The notice is `FocusPaneNotice` (Hint / NoMatches / FilterExcludesAll), computed by `AppView::focus_pane_notice`; a No-matches notice with an active Filter also offers Clear Filter. Focus Find's counter reads "–", "No matches" or "i/n". The unused Match Navigator component is deleted.
- Parsed View zero rows: skipped. Per ADR-0008 the view always shows every line and Filter only narrows the Focus Pane, so Filter can never empty the canvas.
- Tests: `tests/focus_pane_notice_test.rs` (2 pure, 6 harness). Verified in a clean worktree of `main` + these changes, because the shared tree did not build at the time (another session's in-progress edit to the remote browse code).
