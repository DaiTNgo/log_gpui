# 0015. Log Format Is The Only Way To Define Columns; The Extraction Rule Is Removed

_Replaces the part of ADR-0013 that lets users define columns with an Extraction Rule ("Make column…"), and its Focus Pane line about order, widths and Column Labels._

## Context

ADR-0013 gave users three ways to make a column: an Extraction Rule (a regex with one capture group), a Log Format, and a switched-on Detected Field. In use they overlapped. A Log Format already reads a whole line into named fields, so a second place that cuts a stretch of text out of a line was a second place to fix when a column was wrong, with its own priority order (rule 38) to explain which one wins. The Columns section also did too much: it showed, hid, reordered and renamed columns, for the Canvas and the Focus Pane both.

## Decision

**Log Format is the only way to define columns.** The Extraction Rule, "Make column…" and the Column Label are removed. A wrong or missing column is fixed in one place: "Log Format…" in the sidebar (Log Space and file rows) and in the cell and Focus Pane menus, which opens the Log Format editor with a line as the sample (rule 21). Regex stays only in Log Formats (rule 36).

**The Columns section only shows and hides columns of the Focus Pane.** Each column is a switch; there is no reorder and no rename. The Focus Pane keeps the order by source (built-in, from Log Format, then Detected).

**The Unified Canvas shows every column.** The user cannot hide one there, so the Canvas never depends on what the Focus Pane shows.

The value order of rule 38 is now fixed column or JSON, then Log Format, then Detected Fields. Extraction Rules and Column Labels saved by an older version are ignored and dropped on the next save.

## Consequences

- **Known trade-off:** `key=value` Detected Fields are read only while their switch is on in the Focus Pane. They therefore show in the Canvas only while they are on there, and the Canvas cannot show one the Focus Pane hides. A field the user wants in the Canvas regardless must be in a Log Format.
- A Filter condition or Highlight Rule on a column that is no longer read stays, matches nothing, and is explained ("column no longer read"), as before.
- Users who relied on "Make column…" for one-off regex columns now write a Log Format instead.
