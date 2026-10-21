# 05: In the sidebar, a file's name wins over the "No timestamps" flag

**What to build:** A file row in the sidebar always shows as much of the file name as fits. The "No timestamps" flag becomes an icon with a tooltip explaining what it means (the file goes after all other rows) instead of a text badge that squeezes the name down to `java.l`.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] With the default sidebar width, `java.log` and `worker.log` flagged "No timestamps" show their full names
- [ ] The flag is an icon; hovering it shows "No timestamps — these lines go after all others"
- [ ] Long names truncate with an ellipsis; the full name is in a tooltip
