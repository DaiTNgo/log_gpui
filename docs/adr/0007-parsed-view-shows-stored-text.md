# 0007. Parsed View Shows Stored Text; Markers Color Text, Highlight Rules Color Rows

## Context

The Parsed View rewrote what it showed: the level column showed a normalized level (`E` and `err` became `ERROR`), the timestamp column showed a reformatted time without the date, and continuation lines were folded into their parent row behind an expand toggle. Users who typed or pasted `err` and saw `ERROR` stopped trusting that the Parsed View and the Raw View showed the same log. A **Marker** was a symbol in the Gutter (Bookmark, Important, Note, plus automatic Error/Warning), listed in a Marker panel, which was barely visible in the screenshots users take to share an analysis. Users also wanted every row with a given value to stand out in its own color.

## Decision

Every cell of the Parsed View that comes from the line shows exactly the characters stored in that line: level, timestamp and JSON values (only the surrounding quotes are dropped, escapes stay). The app still reads meaning from a value to sort, filter, place Level Markers and parse a Time Range, but never rewrites the text it shows. Every stored line is its own row, continuation lines included, so line numbers match the Raw View. A selection may run across cells and rows, and copying gives the stored text.

Because both views show the same characters, coloring splits into two concepts:
- A **Marker** is a color the user puts on selected text in one line, set from a small floating color bar next to the selection. It shows in both views and always sits above other colors. There is no Marker panel and no note; the overview ruler shows where Markers are.
- A **Highlight Rule** is a condition on one column's values (the same ticked values or "contains" text as Filter), paired with a color. It tints whole rows in the Parsed View only. Rules belong to the Project, can be switched off, and are ordered (the higher rule wins). A cell's context menu may create one ("Highlight rows where…").

The automatic error/warning symbols become **Level Markers**, which are not Markers. Colors come from a **Highlight Palette** of 10 hues × 3 shades, plus a free color picker.

## Considered Options

- **Keep normalized level and time in the Parsed View**: rejected, showing different characters than the file is what made users distrust the app, and it breaks Marker positions between views.
- **Highlight Rules as keyword rules on the whole line (Search Expression syntax)**: rejected, rules by column value match how users analyse logs ("all `module = DB` rows"), and a `message` "contains" rule still covers the keyword case.
- **Search also tints whole matching lines**: rejected, it would clash with Highlight Rule colors. Search keeps highlighting only the matched text.
- **Keep Marker types and the Marker panel**: rejected, users mark logs to read and share them as screenshots, which needs visible color on the text itself, not a list.

## Consequences

- The timestamp column gets wider, and Filter Values of the level column list each spelling (`E`, `ERROR`) separately, merging only case differences.
- Time Range accepts any time format the parser can read, since one Log Space can hold files with different formats.
- The parser must keep source character ranges for every field, so cells and Markers map to the stored line.
- Removing the expand toggle for stack traces changes row counts and the visible-row indexing that Search, Filter and selection rely on.
- Without a Marker panel, Markers cannot be listed or jumped through except via the overview ruler.
