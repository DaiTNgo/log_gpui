# 16: Recent Search Expressions

**What to build:** The Search field remembers recent Search Expressions. With the field empty (or with the history opened), ↑/↓ step through them; a small dropdown lists them for picking. The history is kept per Project across restarts.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] An expression is added when it was searched (Enter or a pause after typing), not for every keystroke; duplicates move to the top; capped (e.g. 20)
- [ ] ↑/↓ in an empty field cycle the history; Escape restores what was typed
- [ ] A dropdown lists recent expressions with a way to remove one
- [ ] History persists with the session, per Project
- [ ] Harness test for adding, cycling and persisting
