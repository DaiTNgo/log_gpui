# 0014. Live and Search Work Is Incremental and Off the UI Thread

## Context

At 1M lines every Live batch (up to ten a second) re-ran Filter, highlights, Search and the Focus Pane over every row, a Live Window trim built the whole view again, lines were read on the UI thread, and every Search keystroke scanned every row before the next could be typed. The PRD in `.scratch/performance/` has the measurements.

## Decision

- **Work follows what changed.** A Filter change (a Value, Hide Mode, a Custom Value, the Time Range) refreshes only the Focus Pane: Matches, highlights and their counts read every row (ADR-0012) and stay. Lines that arrive after the last row, including those of a file with no time, only add their own rows to everything that reads rows. The oldest lines of a file leaving the Live Window take their rows out of the view in place. Anything that moves rows in the middle still builds again.
- **Reading and scanning leave the UI thread.** A batch of 1,500 lines or more is read on a worker thread and applied when read; a pump applies for about 8 ms and takes at most 8,000 lines per file, the rest waits for the next one. Sources and Rotated File decoding are unchanged.
- **Typing waits for a pause.** The Matches of a Search Expression are looked for 100 ms after the last keystroke, on a worker thread, for the last text only; until then the Matches are those of the text before it, and the typed expression already highlights. A Time Range re-filters after the same pause, and only when both bounds can be read and differ from the ones in use; Enter applies at once.
- **Sequential SFTP reads keep their read-ahead**, and reads that look at a few bytes (Starting Time, Load More, the head of a file) ask for what they need only.
- **The Live Window also applies to a local file** that is followed live, once it grows past it.

## Considered Options

- **Parsing in the sources**: the sources would send finished entries. Rejected, the numbers, Parent Line and Log Format are the buffer's, so reading ahead is a guess the buffer checks and falls back from.
- **Searching in slices on the UI thread**: rejected, it needs no copy of the rows' texts but still blocks frames, and slices go stale when Live lines arrive.
- **No pause for Search under some size**: rejected, one rule is easier to predict.

## Consequences

- Typing in Search shows no new count for the length of the pause plus the scan. Tests that type wait out the pause.
- A Live Window trim that leaves a Continuation Line without its Parent Line still builds the view again.
- A stale Search scan is dropped when it ends; it is not stopped halfway.
