# 19: Raw and Parsed View open a Log Space at the same place

**What to build:** Opening the same Imported Log Space lands the reader at the same place whichever view it opens in. Today the Parsed View opens at the newest line (Follow on, rule L3) while the Raw View opens at line 1 of the first File Section, with Follow still shown in the status bar.

**Blocked by:** None (can start immediately).

**Status:** needs-triage

Needs a decision before building: either the Raw View follows too (each File Section scrolled to its last line while Follow is on), or Follow is off by default in the Raw View and the status bar says so. Record the choice in CONTEXT.md (rule L3) or an ADR.

- [ ] A Log Space opened in the Raw View and in the Parsed View shows the same end of the log (per the decision above)
- [ ] The Follow control in the status bar matches what the Raw View actually does
- [ ] Switching Raw ↔ Parsed keeps the current line in sight
- [ ] GPUI harness test for the opening position in both views
