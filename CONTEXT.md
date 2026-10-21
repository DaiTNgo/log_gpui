# Log Viewer Domain

Log viewer canvas, virtualized log streaming, and spatial grid layout.

## Language

### Spaces

**Project**:
The top-level investigation unit the user switches between. It holds Log Spaces and never shares state with other Projects.
_Avoid_: Workspace.

**Log Space**:
A named group of log files inside a Project, opened as one canvas whose rows from all its files are interleaved by timestamp. It is exactly one of an Imported Log Space, a Scratch Log Space or a Remote Log Space, and files can be added or removed after it is created.
_Avoid_: Log File Space, file space.

**Source File**:
The file inside a Log Space that a row came from.
_Avoid_: Origin, input.

**File View**:
A single file of a Log Space opened on its own, showing only that file's rows.
_Avoid_: File tab, single view.

**Pinned Tab**:
A tab of the tab strip kept at its start. It moves in front of the other tabs, shows a pin in place of the close button, and stays when Close Others, Close to the Left/Right or Close All run and when `⌘W` is pressed on it; clicking its pin closes it. It is kept across restarts. The right-click menu of a tab also copies the path of a File View's file, shows it in Finder, and shows its Log Space in the sidebar.
_Avoid_: Pin Toggle (that pins a column), frozen tab, sticky tab.

**Source Column**:
The column in a Log Space's Parsed View that names each row's Source File. It is pinnable and filterable, and can be hidden from Quick Settings.
_Avoid_: File column, origin column.

**Quick Settings**:
The toolbar popover holding display switches for the open view, such as showing the Source Column and density.
_Avoid_: View panel, view options.

**Imported Log Space**:
A Log Space whose files were dropped or picked when it was created, or Archive Entries, or Remote Copies. Its files are read-only.
_Avoid_: Read-only space.

**Archive**:
A `.zip`, `.tar`, `.tar.gz`/`.tgz` or single `.gz` file dropped or picked like a log file. It is never added itself: it stands for the Archive Entries inside it. Recognized by its first bytes, not its extension.
_Avoid_: Compressed file, bundle.

**Archive Entry**:
A text file inside an Archive, added to an Imported Log Space and shown as `<archive name> › <path in archive>` (a lone `.gz` shows its name without `.gz`). It is read straight into memory, never extracted to disk, and never read live.
_Avoid_: Extracted file, unpacked file.

**Scratch Log Space**:
A Log Space created with only a name. Its files (`untitled.log`, `untitled-2.log`, …) start empty in the app's own data folder, and the user types or pastes log text into them.
_Avoid_: Empty space, manual space.

**Remote Log Space**:
A Log Space whose files live on servers reached over SSH. Its files are read-only, can come from several servers, and keep receiving new lines while the Log Space is open.
_Avoid_: SSH space, server space, remote tab.

**Remote File**:
A file of a Remote Log Space, named by its server and its path on that server. A Remote File is either added by its path or brought in by a Live Glob. Edit… changes its path, Server or Starting Time in place: it keeps its place in the Log Space and its File View. Only the Starting Time changing reads the same file again and keeps its Markers; another path or Server makes it another file, so its lines and Markers go.
_Avoid_: SSH file, server log.

**Rotated File**:
A file on a Server that holds an earlier Generation of a Remote File: it sits in the same folder and is recognised by its name (`<name>.<N>`, `<name>-<YYYYMMDD>`, `<name>-<YYYY-MM-DD>`, with or without a `.gz`, `.bz2`, `.xz` or `.zst` suffix). A Rotated File is not a Remote File of its own: it shows as the Generations above the current file.
_Avoid_: Backup file, old log, archive log.

**Remote Copy**:
The content of a Remote File and the Rotated Files chosen, read whole once into the app's data folder and added to an Imported Log Space as `<server>:<path>`. A Remote Copy is static and read-only: it does not follow the file on the Server.
_Avoid_: Download, snapshot, cached file.

**Live Glob**:
A glob on one Server, kept by a Remote Log Space, whose matching files are the space's Remote Files, including files that appear after it was added. The glob may only be in the file name. Edit… changes its pattern, folder or Server in place: files that still match go on untouched, files that stop matching are removed (not Gone), new matches are added, and the dialog previews `+N files · −M files · K kept` first.
_Avoid_: Wildcard, watch, pattern, auto-add.

**Stopped Live Glob**:
A Live Glob the user has stopped watching: the files it already brought keep reading live, but it adds no new matching files until the user resumes it. Stopping is not Pause: lines keep arriving.
_Avoid_: Paused glob, frozen glob, disabled glob.

**Gone**:
The state of a Remote File that came from a Live Glob and no longer exists on its Server. Its lines stay until the Log Space is closed; it is not brought back when the Project is reopened.
_Avoid_: Deleted, missing, stale.

**Recent Paths**:
The paths and Live Globs last added from one Server, offered again in the file picker. Kept per Server for the whole app.
_Avoid_: History, bookmarks, favorites.

**Live Window**:
The newest lines a followed file holds, up to a set number of lines: each Remote File, and each local file that is followed live once it grows past it. Lines older than the window are no longer shown; Load More brings them back from the file.
_Avoid_: Buffer, cache, history limit.

**Server**:
The saved details for reaching one machine over SSH: address, port, user, how to sign in (key, password, or ssh-agent, optionally followed by questions the server asks such as a one-time code), and optionally a Jump Server. A Server may belong to one Server Group and carry tags. Servers are shared by every Project.
_Avoid_: Host, connection, remote, SSH profile.

**Server Group**:
A named heading that Servers are listed under in the Servers list and the Server chooser, such as `prod` or `staging`. A Server belongs to at most one.
_Avoid_: Folder, environment, cluster.

**Quick Connect**:
Adding a Remote File by typing `user@host[:port]:/path` in one field. It reuses a matching Server or saves a new one.
_Avoid_: Ad-hoc connection, temporary server.

**Sign-in Prompt**:
A question the server asks while signing in (keyboard-interactive), such as a password or a one-time code, shown to the user as asked. Answers are never stored.
_Avoid_: 2FA dialog, OTP box.

**Jump Server**:
A Server that the app connects through to reach another Server that is not directly reachable.
_Avoid_: Bastion, proxy, jump host.

### Live

**Follow**:
A view switch that keeps the newest line in sight: while it is on, the view scrolls to each new line as it arrives. Turning it off leaves the view where it is while new lines keep arriving.
_Avoid_: Tail, auto-scroll, stick to bottom.

**Pause**:
A switch that stops a Log Space from taking in new lines from its files. While paused, the view, Filter and Focus Pane do not change.
_Avoid_: Freeze, stop, tail off.

**Break**:
A labelled divider drawn between two rows where a file's lines stop being continuous, such as when the file was rotated or truncated, or the connection to its Server was lost and regained. It is not a row: it has no line number, is never a Match, never passes or fails Filter, and is never copied.
_Avoid_: Gap row, separator line, marker.

**Generation**:
The stretch of a live file's lines between two rotation or truncation Breaks. Line numbers start again at 1 in each Generation, matching the file on disk or on its Server, and a line is known by its Generation and line number. The older Generations of a Remote File can also be read from its Rotated Files on the Server; the Generation just before the current file is the newest Rotated File.
_Avoid_: Segment, epoch, rotation.

**Starting Time**:
Where reading a Remote File begins, instead of its last lines: a time on the clock of the file, or a span such as `1h` counted back from the newest line of the file.
_Avoid_: Since, offset, time filter (a Time Range is a Filter; the Starting Time decides which lines are read at all).

**Full Read Limit**:
The most lines one Open in full reads, one value for the whole app.
_Avoid_: Max lines, cap.

### Views

**Raw View**:
Each file's text shown line by line exactly as stored, with line numbers and no columns, one collapsible File Section per file. It always shows every line, ignoring Filter; only Search and Markers apply. Editable only in a Scratch Log Space.
_Avoid_: Text mode, plain view.

**File Section**:
One file's block in the Raw View, headed by the file name and folder, that can be collapsed or expanded.
_Avoid_: Excerpt, file panel.

**Parsed View**:
The Unified Canvas: the file split into detected columns, with Search, Markers and Highlight Rules. It always shows every line; Filter narrows only the Focus Pane below it.
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
Finding and jumping between log lines that match a Search Expression. Search never changes which lines or File Sections the view shows; it marks Matches across the whole view, moves the selection between them, and narrows the Focus Pane together with Filter.
_Avoid_: Query.

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
A log line of the view whose raw line satisfies the Search Expression, with every Phrase occurrence in it highlighted. Filter does not affect Matches; a Match may be a line the Focus Pane does not list.
_Avoid_: Hit, result.

**Filter**:
Narrowing the lines of the Focus Pane by the values of the file's columns, in the Parsed View only. Columns and their values are detected when the file is loaded, and the user picks values to show or hide. The only place where the user asks about a specific column.
_Avoid_: Query, view.

**Focus Pane**:
The full-width pane under the view and both sidebars, holding one or more Focus Tabs. The open Focus Tab lists only the lines passing both Filter and its Search Expression. In the Parsed View each line shows in the same columns as the view (same order, widths and Column Labels; a column hidden in the view is hidden here too), with its line number; in the Raw View each line shows as its raw line with its line number and, when the Log Space has several files, its Source File. An empty Search Expression or an empty Filter sets no limit; when both are empty the pane is empty. In the Raw View only the Search Expression applies. Its current line and the view's selection always follow each other. It shows Markers and Level Markers, and keeps up with tailed lines and scratch edits.
_Avoid_: Match Pane, Filter Log, result pane, search results.

**Focus Window**:
A separate window the Focus Pane can be moved into, to be made larger or put on another screen. While it is open the Focus Pane exists only there, together with the Search field of the open Focus Tab. It always follows the view open in the main window, and closing it or choosing "Move back" returns the Focus Pane under the main window. It is not reopened after a restart.
_Avoid_: Popup, second window, detached pane.

**Focus Tab**:
One numbered tab (1, 2, 3, …) of the Focus Pane with its own Search Expression and its own Focus Find. The Search field in the toolbar always edits the open Focus Tab's Search Expression, and only the open tab's Matches are marked in the view. Numbers are never reused while the tab is open, and a new tab takes the next number after the highest. The last remaining Focus Tab cannot be closed.
_Avoid_: Search tab, result tab.

**Focus Find**:
Finding and jumping between the lines a Focus Tab already lists, written like a Search Expression. It never narrows the Focus Tab, and its finds are marked only in the Focus Pane.
_Avoid_: Pane search, sub-search.

**Inspector**:
The right sidebar section that shows the stored text of the selected line, with its Continuation Lines, in both the Raw and the Parsed View.
_Avoid_: Details, fields panel.

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

**Field Column**:
A column of the Parsed View besides TIMESTAMP, LEVEL, MODULE and MESSAGE that shows a named field read from the line: a JSON key, the field of an Extraction Rule or of a Log Format, or a Detected Field turned on. Its cell is a stretch of the line's text.
_Avoid_: Custom column, extra column, dynamic column.

**Column Label**:
The name the user gives a column to show, belonging to the Project. The field's own name does not change.
_Avoid_: Alias, display name, column title.

**Coverage**:
The share of Parent Lines that have a value in a column, over the lines the view holds.
_Avoid_: Fill rate, match rate.

**Extraction Rule**:
A name and a regex with exactly one capture group, belonging to the Project. It reads a Field Column from every line of every Log Space in the Project.
_Avoid_: Parser, pattern, custom field.

**Log Format**:
A named template describing a whole line with Format Tokens, shared by the whole app. It is chosen for a Log Space or a Source File, or applies on its own by a file name glob.
_Avoid_: Grok, parser, profile, schema.

**Format Token**:
A named pattern in a Log Format, such as `TIMESTAMP` or `IP`, written `%{TOKEN:field}`.
_Avoid_: Placeholder, macro.

**Auto-detect**:
How a line is read when no Log Format applies to it: JSON and the built-in layouts.
_Avoid_: Default parser.

**Detected Field**:
A field the app finds in lines on its own (a JSON key, or a `key=value` pair in MESSAGE), listed in the *Detected* group of the Columns section with its line count and a switch, off by default.
_Avoid_: Suggested column, auto column.

**Columns** (section of the context sidebar):
The one place to show, hide and order the columns of the view, manage Extraction Rules and choose a Log Format.
_Avoid_: Column manager, view panel.

## Rules & Constraints

**Live**

L1. **Pause Is Per Log Space**: Pause stops every file of the Log Space at once; there is no per-file pause. Resuming takes in every line written while paused, so nothing is skipped.
L2. **Follow Tracks The Scroll**: Scrolling away from the newest line turns Follow off; scrolling back to it turns Follow on.
L3. **Live Starts Fresh**: A Remote or Imported Log Space always opens following and not paused; Follow and Pause are not remembered. A Scratch Log Space has neither. An Imported Log Space made by Open in full opens with Follow off and the view at the first line read.
L4. **Remote Opens At The End**: A Remote File first shows its last lines at once, or the lines from its Starting Time when the user chose one, then fills its Live Window with older lines in the background while it stays readable and live, until the Live Window is full, the Starting Time is reached or the file ends. When there is room left, the newest Rotated File is read on if it is not compressed (L24); other Rotated Files are read only when the user chooses. Every load stays inside the Live Window. Line numbers are always the line's real number in its Generation on its Server.
L5. **Rotation Keeps History**: When a file is rotated, truncated or deleted, the lines already read stay, a Break is drawn, and reading continues in the new file as a new Generation, waiting for it to appear if needed. A Marker belongs to one line of one Generation.
L6. **Reconnect Without Loss**: A lost connection is retried by itself and reading resumes where it stopped, with a Break marking the outage. After a long outage the app stops retrying and waits for the user, and a Server that needs Sign-in Prompts is not retried by itself at all (L17).
L7. **Live Status In The Status Bar**: The connection state of a Remote Log Space, and the Follow and Pause switches, are shown in the status bar only.
L8. **Remote Clocks Are Not Adjusted**: Rule 19 holds across Servers: times from different Servers are compared as written, with no timezone or clock-skew correction.
L9. **Readable Without Elevation**: A Remote File is read with the Server's own user. A file that user cannot read shows a permission error; the app never asks for elevated rights.
L10. **Live Is A Window, Not An Archive**: A Remote Log Space may stay connected for as long as the user likes, but holds only its Live Window; the oldest lines leave as new ones arrive. Nothing received is kept after the Log Space is closed. To read a Remote File in full, the user uses Open in full to make a Remote Copy, or downloads the file to keep it in a folder of their own.
L11. **Paths May Not Exist Yet**: A Remote File or Live Glob may be added before any file exists; it waits and starts reading when a file appears. A Server that cannot be reached or a folder that does not exist is refused when adding.
L12. **Leaving The Window**: A line that leaves the Live Window loses its Marker, stops counting toward Filter Values, and a Break at the top of the view says older lines have left. A Filter Value with no lines left disappears unless it is ticked. A Focus Tab whose current line left moves to the oldest line still held.
L13. **Window Per Remote File**: The Live Window size is one app-wide setting applied to each Remote File separately, so a busy file never pushes a quiet file's lines out. A local file followed live in an Imported Log Space has the same Live Window; it is not applied to a local file when it is opened, only as the file grows.
L14. **Download To Keep**: A Remote File, or every file of a Remote Log Space at once, can be downloaded to a chosen folder and then opened as a new Imported Log Space. By default only the current file is downloaded; the user may tick Rotated Files to take along, and they are kept as they are, compressed ones included.
L15. **Servers In Use Stay**: A Server cannot be deleted while a Remote Log Space or another Server's Jump Server uses it; the app lists what uses it instead.
L16. **Config Is Imported, Not Followed**: Servers can be imported from `~/.ssh/config` and refreshed on request; the app never reads it while connecting, and an imported Server is an ordinary Server the user can edit.
L17. **Sign-in Prompts Need The User**: Answers to Sign-in Prompts are never stored. A Server that needs them is never reconnected by itself — after a lost connection or on reopening a Project it waits for the user to press Connect.
L18. **Quick Connect Saves A Server**: A Remote File always belongs to a saved Server. Quick Connect reuses the Server matching the typed user, host and port (or the typed Server name) and otherwise saves a new one when the file is added.
L19. **Agent Keys Stay In The Agent**: With ssh-agent sign-in the app never sees or stores the private key; it only asks the agent to sign.
L20. **Globs Stay Live**: A Live Glob keeps adding the files that come to match it while the Log Space is open and when it is reopened. A matching file is never added twice; a Rotated File is never added as its own Remote File, since it is an earlier Generation of the file it belongs to (see Rotated File); a file already read under another name is never read again; and a file removed by the user is not added again by its Live Glob unless the user adds that exact path again. How many files one Live Glob reads at once and how often its folder is checked are app-wide settings. Each file it brings has its own Live Window (L13).
L21. **Gone Is Not An Error**: A file of a Live Glob that disappears from its Server becomes Gone: its lines stay, a Break is drawn, and it does not count toward the worst state in the status bar. If a file appears again at the same path it continues as a new Generation (L5).
L22. **One Pick, Many Servers**: A path or Live Glob picked once can be added on several Servers at once; each Server gets its own Remote File or Live Glob, named `<server>:<path>`.
L23. **Stopping A Glob Keeps Its Files**: A Live Glob can be stopped and resumed. While stopped, its files keep reading live and can still become Gone, but no new matching file is added. Resuming checks the folder at once and reads files that appeared meanwhile from their start, within the Live Window. A stopped Live Glob stays stopped when the Project is reopened, and reopens only the files it already had that still exist.

L24. **Rotated Files Are Earlier Generations**: The Rotated Files of a Remote File show as Generations above the current file, never as Source Files of their own. Their order is by time of last write, the newest nearest the current file. When a Remote File has been read to its first line and the Live Window has room, the newest Rotated File is read on automatically if it is not compressed; an older one, or a compressed one, is read only when the user chooses. Open in full reads no Rotated File the user did not tick.
L25. **Starting Time Uses The File's Clock**: A Starting Time is compared with the times as written in the file. A span such as `1h` is counted back from the newest line of the file, not from the clock of this machine or of the Server (L8).
L26. **Full Read Never Cuts Silently**: Open in full reads no more than the Full Read Limit. When the estimate is above it the app asks before reading; when the real count reaches it during the read the app stops and draws a Break "stopped at Full Read Limit".

**Canvas, Search and Filter**

1. **Default Pin State**: Only the Gutter is anchored by default. All data columns (`TIMESTAMP`, `LEVEL`, `MODULE`, `MESSAGE`) start in their unpinned state.
2. **Pinnability**: The Gutter is always anchored; MESSAGE can never be pinned. TIMESTAMP, LEVEL, MODULE, the Source Column and every Field Column can be pinned.
3. **Order Preservation**: Pinned columns always keep the view's column order (TIMESTAMP → LEVEL → MODULE → Source → Field Columns in the order of the Columns section), whatever order they were pinned in.
4. **Unified Scrolling**: The horizontal scrollbar always spans the full viewport width. Toggling a pin never shifts or resizes the scrollbar element itself.
5. **No Word Wrap**: The canvas operates strictly in unconstrained horizontal mode (`ListHorizontalSizingBehavior::Unconstrained`). Word wrap is permanently deprecated.
6. **Open Dynamic Sizing with Hitbox Safety Floor**: Columns auto-size to fit their longest content without any arbitrary `max_width` ceiling. A minimal `min_width` safety floor (~36–48px) strictly protects the interactive hitbox so columns cannot be collapsed into an unclickable zero-width state.
7. **Search Is Line-Wide**: A Search Expression matches against the whole raw line, never against a single column or field. Hiding a column does not change what matches. Each shown cell paints the Matches it holds; a Match line whose Matches are all in hidden columns (or in text no column shows) has a mark in the Gutter whose tooltip names those columns.
8. **Search Is Case-Insensitive**: There is no match-case option. Case is folded per Unicode (e.g. `Đ` matches `đ`).
8a. **Search Is Diacritic-Sensitive**: Diacritics are never ignored. `lỗi` matches only `lỗi`, and `loi` does not match `lỗi`. Composed and decomposed forms of the same character match each other. Filter string comparisons follow the same Unicode rules.
9. **Search Has No Regex Or Negation**: Only `&&`, `||` and parentheses combine Search Terms. `&&` binds tighter than `||`.
10. **Phrase Whitespace**: Any run of whitespace in the raw line satisfies a space between words of a Phrase.
11. **Bare Adjacency Is A Phrase**: Words next to each other without an operator form one Phrase. A Phrase directly followed by a parenthesised group, or the reverse, is a syntax error.
11a. **Switching Focus Tab Moves The View**: Each Focus Tab remembers its current line, which is its current Focus Find when there is one. Opening a Focus Tab jumps the view to that line.
12. **Invalid Expression Keeps Last Matches**: While the typed Search Expression is incomplete or invalid, the Matches from the last valid expression stay shown and an error indicator appears beside the field.
13. **Filter Combination**: Within one column, Filter Values and Custom Values combine with OR; across columns, and with the Time Range, they combine with AND.
14. **Filter Has No Regex**: Filter matches by equality (Filter Value) or contains (Custom Value) only.
15. **Filter Counts Ignore Other Filters**: A column's Filter Values and their row counts never change because other filters are applied. They count the lines the view holds: computed when the file is loaded, new values appended unticked and counts grown as lines arrive, and counts shrunk as lines leave a Live Window. A column that grows past 500 distinct values keeps the conditions already set but accepts no new ones.
16. **Search Counts Only Filtered Lines**: In the Parsed View, only lines that pass Filter count as Matches. The Match count, next/previous, the overview ruler, the highlights in the view and the Focus Pane all use this one set. In the Raw View, Filter is ignored.
17. **Quick Settings Only**: There is no View panel. Column pinning stays on the Column Header and column width stays adjustable by drag and Auto-fit. Quick Settings holds only display switches (Source Column, density). Showing, hiding and ordering columns, Extraction Rules and Log Formats live in the Columns section; the Source Column switch in both places is one setting.
18. **Missing Value Never Matches**: A row with no value in a column never matches any Filter Value or Custom Value of that column. It is hidden when the column shows only the chosen values, and stays visible in Hide Mode. There is no "(empty)" value.
19. **Continuation Lines Inherit**: A Continuation Line takes the timestamp, level, module and every other value of its Parent Line, so Filter, Time Range and Highlight Rules treat it like its Parent Line; it passes Filter when its Parent Line does. Its own text stays its message. Filter counts and Level Markers count only the Parent Line. Lines before the first timestamp have no Parent Line and are always shown. Times are compared in the file's own clock, with no timezone conversion.
20. **Filter Per View, Marker Per Line**: A Log Space's Filter is shared by all its files, and each File View has its own separate Filter; both are remembered automatically and restored when reopened. A Marker belongs to one line of one file and shows wherever that line is shown; removing the file from the Log Space removes its Markers. Focus Tabs, with their Search Expressions, Focus Finds and current lines, are remembered per view in the same way. There are no project-level saved presets.
21. **No Quick Filter Menu**: There is no quick menu for filtering by a cell value; filtering is done from the Filter section only. A cell's context menu may offer "Highlight rows where…", which creates an ordinary Highlight Rule managed with all the others, and "Make column…", which creates an Extraction Rule.
22. **Interleave By Timestamp**: A Log Space's rows are ordered by timestamp across files. Equal timestamps keep the order of files in the Log Space, then line order. Continuation lines stay with their parent row. Lines before a file's first timestamp take that first timestamp. A file with no timestamps goes after all other rows and is flagged.
23. **Columns Are United By Name**: A Log Space's columns are the union of columns detected in its files, matched by name. Field Columns from JSON, Extraction Rules, Log Formats and Detected Fields are united by field name too; Column Labels take no part in it.
24. **Space Kinds Never Mix**: An Imported Log Space only gains files and Archive Entries by import, and Remote Copies by Open in full; a Scratch Log Space only gains new empty scratch files; a Remote Log Space only gains Remote Files.
25. **Imported And Remote Files Are Read-Only**: The app never changes an imported file, an Archive Entry (or its Archive) or a Remote File. Removing it from a Log Space leaves it untouched on disk or on its server. An Archive Entry is static: it does not follow the Archive if it changes. A Remote Copy is static too: it does not follow the Remote File when it changes, until the user chooses Read again. Removing a Remote Copy from a Log Space deletes its content from the app's data folder.
26. **Scratch Edits Happen In Raw View**: The Parsed View of a scratch file is read-only and is re-parsed when switching from Raw View. Editing a line drops that line's Marker; the Filter keeps its conditions while its values are re-detected.
27. **Save As Copies**: Save As on a scratch file writes a copy to the chosen place; the scratch file stays in the app's data folder.
28. **Log Space Names Are Unique Per Project**: A Log Space needs a non-blank name not used by another Log Space in the same Project.
29. **One File Order Per Log Space**: A Log Space's files are ordered either by when they were added or by name. The same order is used in the sidebar, for File Sections, and for breaking timestamp ties.
30. **Parsed Text Is Stored Text**: Every cell in the Parsed View that comes from the line (all but the Gutter, the Source Column and the `_` of a Continuation Line) shows exactly the characters of that line as stored. The app may read meaning from a value (a level, a time) to sort, filter or place Level Markers, but never rewrites the text it shows.
31. **No Marker Panel**: There is no list or panel of Markers. Markers are found by reading or through the overview ruler, and are changed or removed from the marked text itself.
32. **Marker Over Highlight**: A Marker's color always shows above any Highlight Rule color. Between Highlight Rules, the rule higher in the list wins.
33. **Highlight Rules Belong To The Project**: Highlight Rules are shared by every Log Space in a Project, remembered automatically, and each can be switched off without being deleted. They apply only in the Parsed View, and in the Focus Pane while the view above is the Parsed View.
34. **One Row Per Stored Line**: Every stored line is its own row in the Parsed View, continuation lines included, so line numbers match the Raw View. A continuation line fills only the `message` column.
35. **Selection Is Stored Text**: A selection in the Parsed View may run across cells and rows, and copying it gives the stored text of the selected range. Copying from the Focus Pane gives only the lines it lists.
36. **Regex Only Reads Columns**: Regex is used only in Extraction Rules and Log Formats, to decide which stretch of text a cell takes. Search and Filter still have no regex (rules 9 and 14).
37. **Labels Never Rename Fields**: A Column Label changes only the text on the Column Header and in Filter, Highlights and the Columns section. Field names, Filter conditions and Highlight Rules do not change with it.
38. **One Value Per Field Per Line**: When several sources give a value for the same field name on a line, the value is taken in this order: fixed column or JSON, then Log Format, then Extraction Rules (in the order of the *Rules* group), then Detected Fields.
39. **Unmatched Lines Lose Nothing**: A line that a Log Format or Extraction Rule does not match is still one row, keeps its text in MESSAGE, and only leaves empty the cells that could not be read.
40. **Invalid Patterns Keep Last Preview**: While the regex of an Extraction Rule or the template of a Log Format is invalid, the Preview keeps the result of the last valid one. The error shows only once the user stops typing or leaves the field.
