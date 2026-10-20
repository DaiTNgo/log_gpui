# 0013. Users Define Columns; Regex Stays Out Of Search And Filter

Extends ADR-0005: a Filterable Column is a column the app detects *or* one the user defines.

## Context

The Parsed View had four fixed columns (TIMESTAMP, LEVEL, MODULE, MESSAGE) plus JSON fields that could be filtered but never shown. Everything else in a line (`user=alice`, a request id, an IP, a duration, an HTTP status) stayed inside MESSAGE, so it could not be shown, filtered, coloured or pinned. ADR-0005 accepted that consequence: "a column that is not detected cannot be filtered on". Users who asked for regex in Search or Filter mostly wanted to pull a value out of the line, not to match lines with a pattern.

## Decision

Users define columns, and every column they define is a **Field Column**: a named field whose cell is a span of the stored line (rule 30), never new text. A Field Column can be shown, hidden, ordered, pinned, resized, filtered and used by Highlight Rules like the fixed columns. There are three ways to make one, each usable on its own:

1. **Extraction Rule** (Project): a name and a regex with exactly one capture group, made with "Make column…" from selected text. The app guesses how to read the value (the key before it, the Nth bracket group, the Nth word) and previews the guess on real lines, so the user does not have to write regex.
2. **Log Format** (app library): a template of Format Tokens (`%{TIMESTAMP:timestamp} %{IP:client} …`) that reads a whole line. Its `timestamp`, `level`, `module` and `message` fill the fixed columns, so ordering and Level Markers keep working. It is chosen per Log Space or Source File, or applies by file name glob.
3. **Detected Field**: JSON keys and `key=value` pairs in MESSAGE, listed with a line count and switched on by the user.

**Regex only reads columns** (rule 36). Regex appears in Extraction Rules and Log Formats, to decide which stretch of text a cell takes. Search and Filter keep matching by text and equality only (rules 9 and 14). ADR-0004 and ADR-0005 otherwise stand.

When several sources give the same field on a line, one value is taken: fixed column or JSON, then Log Format, then Extraction Rules in priority order, then Detected Fields (rule 38). A line that a format or rule does not match keeps all its text in MESSAGE (rule 39). The **Columns** section of the context sidebar is the one place that manages columns.

## Considered Options

- **Regex operator in Filter**: rejected. It is what ADR-0005 removed, and it gives the user no column to look at.
- **Keep extracted values as copied text**: rejected. It breaks rule 30 and costs memory.
- **One kind of column per source** (JSON column, rule column, …): rejected. UI and Filter would branch everywhere, and users do not care where a column came from.
- **Full Logstash grok syntax and pattern library**: rejected. It drags in a large pattern library; only the `%{TOKEN:field}` form is borrowed.
- **Log Formats per Project**: rejected. The same format (nginx, an in-house app) shows up across Projects.

## Consequences

- Fields are stored as spans. JSON fields moved from copied text (`json_fields`) to spans, saving about 744 MB per 1,000,000 JSON lines with 10 fields; each extra field costs 32 bytes a line.
- Changing a rule re-reads only the rule fields, in the background. Changing a Log Format re-reads the file from its stored lines in the background and rebuilds the timeline; Markers are kept because they belong to line numbers.
- A Filter condition or Highlight Rule on a column that is no longer read stays, matches nothing, and is explained ("column no longer read").
- In the Parsed View the Focus Pane shows the same columns as the view (order, widths, Column Labels, hidden columns), none pinned, instead of each raw line; the Raw View's pane still lists raw lines. ADR-0008's "each as its raw line" now holds for the Raw View only.
- The crate `regex` runs in linear time and compiled patterns have a size limit, so a user's pattern cannot hang the app.
