# 0011. Full Reading Goes Through A Remote Copy, Not A Fourth Space Kind

Reading a Remote File in full (the whole file, its Rotated Files, or everything from a time) does not change what a Remote Log Space is. Open in full reads the file from the Server once into the app's data folder, as a **Remote Copy**, and adds that copy to an **Imported Log Space**, next to files on disk and Archive Entries. Remote Log Space stays a window (L10) and Imported Log Space stays static and read-only.

The app is built on the Live Window: a Remote File holds a bounded number of lines and drops the oldest. Reading a whole multi-gigabyte file does not fit that, and each line costs about 1.3 KB of memory, so the lines are bounded by a **Full Read Limit** (2,000,000 lines by default) that the user is asked about before reading, never cut silently (L26).

## Considered Options

- **A fourth Log Space kind ("Remote Snapshot")**: rejected. It would copy most of what an Imported Log Space already does (Filter, Search, Markers, Focus Pane, saving, opening again) and add a branch to rule 24.
- **A "full" mode for single files of a Remote Log Space**: rejected. One Log Space would mix files that are cut by the Live Window with files that are not, and Follow, Pause and the Filter counts would be hard to explain.
- **Keep the lines in memory only**: rejected. Opening the Project again would read the Server again, and the file may have rotated since.

## Consequences

- A Remote Copy is stored as one text file per Generation (decompressed) and a `meta.json`; opening a Project needs no connection. Removing the copy, or its Log Space, deletes the folder; folders nobody refers to are swept at start.
- A Rotated File is an earlier Generation of its Remote File, not a Source File (L24): its Markers are kept by Generation distance, like those of a file that rotated while it was open.
- Compressed Rotated Files (gzip, zstd, xz, bzip2) are told by their first bytes and decoded by pure-Rust crates, so no C library is built.
- "Read again" reads the same Generations and range again and replaces the copy; Markers stay on lines whose Generation, number and text did not change.
