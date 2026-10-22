# 05: Remove Extraction Rule

**What to build:** The Extraction Rule is removed from the app. The *Rules* group leaves the Columns section, and "Make column…" leaves the cell menu: making a column from a cell now goes through "Log Format…" (ticket 03). Rules already saved in a Project or session are ignored when it is read. Tests that only cover Extraction Rules are removed.

**Blocked by:** 03, 04

**Status:** resolved (branch `focus-pane-columns`)

- [ ] No *Rules* group in the Columns section and no "Make column…" in the cell menu
- [ ] A Project that has saved rules opens without error; the rules have no effect and are dropped on the next save
- [ ] The value order for a field no longer lists Extraction Rules
- [ ] No test or code path still reads or writes an Extraction Rule
