# 06: Record the decision

**What to build:** The domain docs match the new behaviour. CONTEXT.md changes the Columns, Extraction Rule and Column Label terms and the rules that mention them (17, 21, 23, 36, 38–40). A new ADR says why Extraction Rule was removed and that Log Format is now the only way to define columns, and it replaces the part of ADR 0013 that says users define columns with regex. The OpenSpec change `single-file-space-row` is checked for overlap and updated if it touches these terms.

**Blocked by:** 04, 05

**Status:** resolved (branch `focus-pane-columns`)

- [ ] CONTEXT.md no longer defines Extraction Rule or Column Label, and Columns is described as the Focus Pane's show/hide list
- [ ] Rules that named Extraction Rules are rewritten or removed, and the rest still read correctly
- [ ] A new ADR gives the reason and says which part of ADR 0013 it replaces
- [ ] The `single-file-space-row` change has no stale mention of the removed terms
