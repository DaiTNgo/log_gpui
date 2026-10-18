# Log Viewer Domain

Log viewer canvas, virtualized log streaming, and spatial grid layout.

## Language

### Spaces

**Project**:
The top-level investigation unit the user switches between. It holds Log Spaces and never shares state with other Projects.
_Avoid_: Workspace.

**Log Space**:
A named group of log files inside a Project, opened as one canvas whose rows from all its files are interleaved by timestamp. It is either an Imported Log Space or a Scratch Log Space, never both, and files can be added or removed after it is created.
_Avoid_: Log File Space, file space.

**Source File**:
The file inside a Log Space that a row came from.
_Avoid_: Origin, input.

**File View**:
A single file of a Log Space opened on its own, showing only that file's rows.
_Avoid_: File tab, single view.

**Source Column**:
The column in a Log Space's Parsed View that names each row's Source File. It is pinnable and filterable, and can be hidden from Quick Settings.
_Avoid_: File column, origin column.

**Quick Settings**:
The toolbar popover holding display switches for the open view, such as showing the Source Column and density.
_Avoid_: View panel, view options.

**Imported Log Space**:
A Log Space whose files were dropped or picked when it was created. Its files are read-only.
_Avoid_: Read-only space.

**Scratch Log Space**:
A Log Space created with only a name. Its files (`untitled.log`, `untitled-2.log`, …) start empty in the app's own data folder, and the user types or pastes log text into them.
_Avoid_: Empty space, manual space.

### Views

**Raw View**:
Each file's text shown line by line exactly as stored, with line numbers and no columns, one collapsible File Section per file. It always shows every line, ignoring Filter; only Search and Markers apply. Editable only in a Scratch Log Space.
_Avoid_: Text mode, plain view.

**File Section**:
One file's block in the Raw View, headed by the file name and folder, that can be collapsed or expanded.
_Avoid_: Excerpt, file panel.

**Parsed View**:
The Unified Canvas: the file split into detected columns, with Filter, Search and Markers.
_Avoid_: Table view, grid view.

### Marking

**Marker**:
A color the user puts by hand on text they selected in one line of one file, so it stands out when reading or in a screenshot. A selection over several lines puts one Marker on each line.
_Avoid_: Bookmark, tag, annotation.

**Level Marker**:
The symbol the app puts automatically in the Gutter of error and warning lines. It is not a Marker and the user cannot set or remove it.
_Avoid_: Passive marker, auto marker.

**Highlight Rule**:
A condition on the values of one column paired with a color. In the Parsed View, rows that meet it are shown in that color for as long as the rule is on.
_Avoid_: Marker, color rule, keyword highlight.

**Highlight Palette**:
The fixed set of colors offered for Markers and Highlight Rules, each hue in several shades, next to a free color picker.
_Avoid_: Color list, swatches.

### Lines

**Continuation Line**:
A stored line with no timestamp of its own that comes after a line with one, such as a stack trace line. It is its own row; its cells other than the message show a muted `_`, while Filter and Highlight Rules use the values of its Parent Line.
_Avoid_: Sub line, child line, stack line.

**Parent Line**:
The nearest line above a Continuation Line that has its own timestamp.
_Avoid_: Head line, main line.

### Canvas, Search and Filter

**Column Header**:
The top-anchored label area of a data column providing title identification, resize handling, and column-level controls.
_Avoid_: Column title, table header.

**Sticky Column**:
A data column that retains its natural position at `scroll_x = 0`, but remains anchored to the left margin (immediately adjacent to the Gutter or preceding sticky columns) when the table is scrolled horizontally.
_Avoid_: Frozen column, locked column, fixed column.

**Pin Toggle**:
The interactive affordance on a Column Header used to anchor or release a Sticky Column.
_Avoid_: Pin button, freeze switch.

**Pin Toggle State**:
- `Unpinned (Idle)`: Hidden to preserve content-as-hero minimalism.
- `Unpinned (Hover)`: Revealed with muted emphasis (`text_muted`) on Column Header hover.
- `Pinned (Active)`: Persistently visible with accent emphasis (`text_accent`).
- `Pinned (Hover)`: Indicated with release affordance (`PushPinSimpleSlash`) for unpinning.

**Gutter**:
The persistent non-scrolling leftmost margin displaying line numbers and row markers, permanently anchored by domain rule.
_Avoid_: Margin, line number bar.

**Unified Canvas**:
The continuous scrollable rendering surface encompassing both sticky and free-scrolling columns, governed by a single full-width horizontal scrollbar.
_Avoid_: Split pane, dual grid.

**Partition Divider**:
The visual boundary (subtle vertical border and scroll shadow) marking the rightmost edge of anchored columns when content scrolls underneath.
_Avoid_: Split line, separator.

**Dynamic Column Width**:
A computed column width derived from the maximum character length of ingested field values and scaled by monospace character advance width, protected from collapse by a minimal `min_width` safety floor with no arbitrary maximum.
_Avoid_: Fixed width, hardcoded width.

**Column Resize Handle**:
The interactive boundary between adjacent Column Headers that supports dragging to resize or double-clicking to trigger Auto-fit.
_Avoid_: Divider, column split.

**Auto-fit**:
The automatic calculation of a column's width to match its longest ingested content based on monospace character advance width, floored by `min_width`.
_Avoid_: Best fit, auto size.

**Search**:
Finding and jumping between log lines that match a Search Expression. Search never changes which lines or File Sections are shown; it only marks matches and moves the selection between them.
_Avoid_: Find, query.

**Search Expression**:
What the user types into Search: Search Terms combined with `&&`, `||` and parentheses.
_Avoid_: Search query, query language.

**Search Term**:
A Phrase that must appear in the raw line for the term to match.
_Avoid_: Keyword, token.

**Phrase**:
One or more words typed next to each other with no operator between them, matched in order as a single unit. Optionally written in double quotes to match characters that would otherwise read as operators or parentheses.
_Avoid_: String, literal.

**Match**:
A log line whose raw line satisfies the Search Expression, with every Phrase occurrence in it highlighted.
_Avoid_: Hit, result.

**Filter**:
Narrowing the shown lines by the values of the file's columns. Columns and their values are detected when the file is loaded, and the user picks values to show or hide. The only place where the user asks about a specific column.
_Avoid_: Query, view.

**Filterable Column**:
A detected column the user may filter on: it has at most 500 distinct values, and it is not `timestamp` or `message`/payload. A column with more than 500 distinct values is shown as unavailable with its distinct count and the reason.
_Avoid_: Facet, filter field.

**Distinct Count**:
The number of different values a column holds in the loaded file, shown next to the column when the user chooses a column to filter on.
_Avoid_: Cardinality, unique count.

**Filter Value**:
A value taken from the loaded file and shown as a checkbox under its Filterable Column. Ticking it matches rows whose value in that column equals it. Values that differ only by letter case are one Filter Value, shown in their most frequent form with counts added together; values that differ by diacritics stay separate.
_Avoid_: Option, facet value.

**Custom Value**:
A text the user types under a Filterable Column with "Add value". It matches rows whose value in that column contains the text, Unicode case-insensitively, without regex.
_Avoid_: Manual value, free text.

**Hide Mode**:
A per-column switch that turns a column's Filter Values and Custom Values from "show only these" into "hide these".
_Avoid_: Exclude, negate.

**Time Range**:
A From/To bound on the `timestamp` column, typed in any time form the app can read from log lines. An empty bound means no limit on that side. It is the only way to filter on `timestamp`.
_Avoid_: Date filter, time slider.

## Rules & Constraints

1. **Default Pin State**: Only the Gutter is anchored by default. All data columns (`TIMESTAMP`, `LEVEL`, `MODULE`, `MESSAGE`) start in their unpinned state.
2. **Pinnability**: Only metadata columns (`TIMESTAMP`, `LEVEL`, `MODULE`) are pinnable. `Gutter` is permanently anchored; `MESSAGE` is permanently unpinnable.
3. **Order Preservation**: When multiple columns are pinned, they always maintain their original schema ordering (`TIMESTAMP` -> `LEVEL` -> `MODULE`) at the anchor boundary, regardless of the sequence in which they were toggled.
4. **Unified Scrolling**: The horizontal scrollbar always spans the full viewport width. Toggling a pin never shifts or resizes the scrollbar element itself.
5. **No Word Wrap**: The canvas operates strictly in unconstrained horizontal mode (`ListHorizontalSizingBehavior::Unconstrained`). Word wrap is permanently deprecated.
6. **Open Dynamic Sizing with Hitbox Safety Floor**: Columns auto-size to fit their longest content without any arbitrary `max_width` ceiling. A minimal `min_width` safety floor (~36–48px) strictly protects the interactive hitbox so columns cannot be collapsed into an unclickable zero-width state.
7. **Search Is Line-Wide**: A Search Expression matches against the whole raw line, never against a single column or field.
8. **Search Is Case-Insensitive**: There is no match-case option. Case is folded per Unicode (e.g. `Đ` matches `đ`).
8a. **Search Is Diacritic-Sensitive**: Diacritics are never ignored. `lỗi` matches only `lỗi`, and `loi` does not match `lỗi`. Composed and decomposed forms of the same character match each other. Filter string comparisons follow the same Unicode rules.
9. **Search Has No Regex Or Negation**: Only `&&`, `||` and parentheses combine Search Terms. `&&` binds tighter than `||`.
10. **Phrase Whitespace**: Any run of whitespace in the raw line satisfies a space between words of a Phrase.
11. **Bare Adjacency Is A Phrase**: Words next to each other without an operator form one Phrase. A Phrase directly followed by a parenthesised group, or the reverse, is a syntax error.
12. **Invalid Expression Keeps Last Matches**: While the typed Search Expression is incomplete or invalid, the Matches from the last valid expression stay shown and an error indicator appears beside the field.
13. **Filter Combination**: Within one column, Filter Values and Custom Values combine with OR; across columns, and with the Time Range, they combine with AND.
14. **Filter Has No Regex**: Filter matches by equality (Filter Value) or contains (Custom Value) only.
15. **Filter Counts Ignore Other Filters**: A column's Filter Values and their row counts never change because other filters are applied. They are computed when the file is loaded; while the file is being tailed, new values are appended unticked and counts grow with new rows. A column that grows past 500 distinct values keeps the conditions already set but accepts no new ones.
16. **Search Ignores Hidden Lines**: Search matches only lines that Filter currently shows; matches on lines hidden by Filter are not counted.
17. **Quick Settings Only**: There is no View panel. Column pinning stays on the Column Header and column width stays adjustable by drag and Auto-fit. Quick Settings holds only display switches (Source Column, density).
18. **Missing Value Never Matches**: A row with no value in a column never matches any Filter Value or Custom Value of that column. It is hidden when the column shows only the chosen values, and stays visible in Hide Mode. There is no "(empty)" value.
19. **Continuation Lines Inherit**: A Continuation Line takes the timestamp, level, module and every other value of its Parent Line, so Filter, Time Range and Highlight Rules treat it like its Parent Line; it is shown when its Parent Line is shown. Its own text stays its message. Filter counts and Level Markers count only the Parent Line. Lines before the first timestamp have no Parent Line and are always shown. Times are compared in the file's own clock, with no timezone conversion.
20. **Filter Per View, Marker Per Line**: A Log Space's Filter is shared by all its files, and each File View has its own separate Filter; both are remembered automatically and restored when reopened. A Marker belongs to one line of one file and shows wherever that line is shown; removing the file from the Log Space removes its Markers. The Search Expression is not remembered. There are no project-level saved presets.
21. **No Quick Filter Menu**: There is no quick menu for filtering by a cell value; filtering is done from the Filter section only. A cell's context menu may offer "Highlight rows where…", which creates an ordinary Highlight Rule managed with all the others.
22. **Interleave By Timestamp**: A Log Space's rows are ordered by timestamp across files. Equal timestamps keep the order of files in the Log Space, then line order. Continuation lines stay with their parent row. Lines before a file's first timestamp take that first timestamp. A file with no timestamps goes after all other rows and is flagged.
23. **Columns Are United By Name**: A Log Space's columns are the union of columns detected in its files, matched by name.
24. **Imported And Scratch Never Mix**: An Imported Log Space only gains files by import; a Scratch Log Space only gains new empty scratch files.
25. **Imported Files Are Read-Only**: The app never changes an imported file. Removing it from a Log Space leaves it untouched on disk.
26. **Scratch Edits Happen In Raw View**: The Parsed View of a scratch file is read-only and is re-parsed when switching from Raw View. Editing a line drops that line's Marker; the Filter keeps its conditions while its values are re-detected.
27. **Save As Copies**: Save As on a scratch file writes a copy to the chosen place; the scratch file stays in the app's data folder.
28. **Log Space Names Are Unique Per Project**: A Log Space needs a non-blank name not used by another Log Space in the same Project.
29. **One File Order Per Log Space**: A Log Space's files are ordered either by when they were added or by name. The same order is used in the sidebar, for File Sections, and for breaking timestamp ties.
30. **Parsed Text Is Stored Text**: Every cell in the Parsed View that comes from the line (all but the Gutter, the Source Column and the `_` of a Continuation Line) shows exactly the characters of that line as stored. The app may read meaning from a value (a level, a time) to sort, filter or place Level Markers, but never rewrites the text it shows.
31. **No Marker Panel**: There is no list or panel of Markers. Markers are found by reading or through the overview ruler, and are changed or removed from the marked text itself.
32. **Marker Over Highlight**: A Marker's color always shows above any Highlight Rule color. Between Highlight Rules, the rule higher in the list wins.
33. **Highlight Rules Belong To The Project**: Highlight Rules are shared by every Log Space in a Project, remembered automatically, and each can be switched off without being deleted. They apply only in the Parsed View.
34. **One Row Per Stored Line**: Every stored line is its own row in the Parsed View, continuation lines included, so line numbers match the Raw View. A continuation line fills only the `message` column.
35. **Selection Is Stored Text**: A selection in the Parsed View may run across cells and rows, and copying it gives the stored text of the selected range, leaving out rows hidden by Filter.
