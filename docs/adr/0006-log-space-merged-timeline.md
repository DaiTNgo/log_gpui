# 0006. Log Space: Files Grouped Into One Merged Timeline

## Context

A Project registered log files directly and every tab showed one file. Investigating an incident usually means reading several related files (`api.log`, `worker.log`, `db.log`) side by side in time order, which meant switching tabs and lining up timestamps by hand. Users also want to paste log text that does not exist as a file yet and read it parsed.

## Decision

A **Log Space** sits between the Project and its files. It is created in one step from the New Log Space dialog: a name plus files dropped or picked. No files gives a **Scratch Log Space** (empty `untitled.log`, `untitled-2.log`, … in the app's data folder, editable). One or more files gives an **Imported Log Space** (read-only). The two kinds never mix: an Imported Log Space only gains files by import, a Scratch Log Space only gains new empty scratch files.

Opening a Log Space shows one **Parsed View** whose rows from all its files are interleaved by timestamp. Its columns are the union of the files' detected columns, matched by name, plus a pinnable, filterable **Source Column** that can be hidden from **Quick Settings**. Equal timestamps follow the Log Space's file order (added or by name), continuation lines stay with their parent, and a file with no timestamps goes last and is flagged. Each file can also be opened on its own as a **File View**.

The Filter belongs to the view: one shared Filter per Log Space, a separate one per File View. A Marker belongs to one line of one file and shows in every view of that line. This replaces "Filter and Marker are per file" from ADR 0005.

Each view switches between Parsed View and **Raw View**, the file text as stored, one collapsible **File Section** per file. Raw View ignores Filter; Search only highlights and never hides lines or sections. Scratch files are edited only in Raw View and re-parsed when switching back.

## Considered Options

- **Keep one tab per file and group files only in the sidebar**: rejected, it leaves the time alignment across files to the user, which is the main reason to group them.
- **Let one Log Space hold both imported and scratch files**: rejected, read-only and editable files in one Raw View and one Filter blur which lines the user may change.
- **Shared Filter between a Log Space and its File Views**: rejected, a change in one view silently changing another is confusing.
- **Raw View shows only File Sections with a Match while searching**: rejected, Search highlights and must not act as a Filter.
- **Save As moves the scratch file to the chosen place**: rejected, keeping scratch files in the app's data folder stops outside edits or deletion. Save As writes a copy.

## Consequences

- Persistence, tabs and Filter state change shape: tabs target a Log Space or a File View, and old sessions are not migrated.
- Merge order depends on timestamp parsing. A file whose timestamps are not detected falls to the end instead of interleaving.
- Tailing an imported file inserts new rows at their timestamp position, possibly mid-canvas, so the viewport must hold its place.
- Editing a scratch line drops that line's Marker. Markers are keyed by line number, and edits shift lines.
