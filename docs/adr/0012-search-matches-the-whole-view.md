# 0012. Search Matches the Whole View; Filter Narrows Only the Focus Pane

Supersedes the sentence of ADR-0008 that says the Match count, next/previous, overview ruler and highlights use the same set as the Focus Pane.

## Context

ADR-0008 made the view show every line and moved Filter to the Focus Pane, but kept Matches limited to lines that pass Filter. With Filter on `INFO` and Search `pool`, the view showed the `DEBUG` line containing `pool` unmarked and the counter read `0/0`. The view is the full log, so its Search should work on all of it.

## Decision

Matches are every line of the view that satisfies the Search Expression, whatever Filter says. The Match count, next/previous, overview ruler and highlights in the view use that set. The Focus Pane still lists only lines that pass both Filter and the Search Expression. Jumping to a Match that the pane does not list leaves the pane without a current line (it scrolls to the nearest listed line before it). Focus Find is unchanged: it works only on lines the pane lists.

## Considered Options

- **Keep Matches limited by Filter** (ADR-0008): rejected, the counter and next/previous disagreed with what the view visibly contains.
- **Show the unlisted Match in the pane temporarily**: rejected, it would make Filter lie.
- **Next/previous skip Matches outside the pane**: rejected, that is the bug.

## Consequences

- The Match count can be larger than the number of lines in the Focus Pane.
