# 02: nginx/Apache access log lines get a timestamp in the Parsed View

**What to build:** A combined/common access log line such as `10.0.0.1 - - [20/Oct/2026:10:00:01 +0000] "GET /health HTTP/1.1" 200 12 "-" "curl/8"` shows its time in the TIMESTAMP column, can be bounded by a Time Range, and interleaves with other files of the Log Space. Reading that time form is already supported; no built-in layout captures it from the line.

**Blocked by:** None (can start immediately).

**Status:** ready-for-agent

- [ ] Common and combined access log lines show the bracketed time in TIMESTAMP (as stored text)
- [ ] A Time Range filter includes/excludes them correctly
- [ ] An access log file is not flagged "No timestamps" and interleaves by time with other files
- [ ] The rest of the line stays in MESSAGE; nothing is lost (rule *Unmatched Lines Lose Nothing*)
- [ ] Parser tests cover both formats
