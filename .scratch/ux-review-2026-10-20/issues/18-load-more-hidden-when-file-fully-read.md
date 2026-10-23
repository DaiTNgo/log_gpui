# 18: "Load more" is not offered for a file that is already read whole

**What to build:** A File Section in the Raw View, and the status bar of a one-file Log Space, offer "Load more" only while there are older lines left to read. Today a local file read from its first line shows a dimmed "Load more (10,000)" under its header (`3,245 lines held (≈5 MB)  Load more (10,000)`), which reads as "something is missing but you can't get it".

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] A file held from its first line shows no Load more button (File Section header, file row menu, status bar)
- [ ] A file whose older lines left the Live Window still shows Load more
- [ ] A file stopped at the Full Read Limit keeps its disabled button with the existing reason in its tooltip
- [ ] The "lines held" text stays, so the reader still sees how much is in memory
