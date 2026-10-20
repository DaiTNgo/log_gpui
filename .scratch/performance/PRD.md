# Performance

A read-only performance review (2026-10-20) of the render, Live ingestion, Search/Filter and remote paths. Rendering is properly virtualized; the cost is in work done around it:

- Every Live batch re-runs Filter, highlight paints, Search and Focus Pane refresh over every row, up to 10 times a second.
- A Live Window trim of a Remote File, and any batch while a file without timestamps is in the Log Space, rebuild the whole view.
- Live lines are parsed on the UI thread, up to 200k per tick.
- Search runs on every keystroke, synchronously, with no cancellation; a Filter change rescans Search although ADR 0012 made Matches independent of Filter.
- Every SFTP `seek` throws away russh-sftp's 16 read-ahead requests, so sequential and probing reads fetch 16-64x the data they use.
- Each followed Remote File runs `exec stat` on the Server every 500 ms.
- Each line's text is stored about three times, and local Live files are never trimmed.

Tickets are in `issues/`, numbered in dependency order. Ticket 01 provides the numbers every Live/Search/Filter ticket must improve.
