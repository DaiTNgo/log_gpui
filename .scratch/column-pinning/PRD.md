Status: ready-for-agent

# PRD: Interactive Column Pinning, Sticky Horizontal Scroll, and Dynamic Sizing

## Problem Statement

When analyzing log streams, engineers frequently encounter wide log entries containing lengthy payloads, URLs, JSON objects, and stack traces. Because log viewer rows have fixed heights to support high-performance virtualization across hundreds of thousands of lines, horizontal scrolling is necessary to read unconstrained text.

However, horizontally scrolling the canvas currently slides all metadata columns (Timestamp, Severity Level, and Module/Service) off-screen. Users lose the surrounding context—such as when the error occurred or which service emitted it—while inspecting long messages.

Additionally:
- Column widths are hardcoded, causing either truncated identifiers or wasted horizontal space.
- Existing word wrap features conflict with fixed-height row virtualization and compress multiple tabular columns into unreadable fragments.

## Solution

A unified horizontal canvas featuring **Sticky Columns**, interactive **Pin Toggles**, and **Dynamic Monospace Column Sizing**:

1. Users hover over any pinnable Column Header (`TIMESTAMP`, `LEVEL`, `MODULE`) to reveal a Pin Toggle icon. Clicking the icon pins the column.
2. In the **Unified Canvas**, all columns maintain their natural schema order at `scroll_x = 0`. When scrolling horizontally (`scroll_x > 0`), pinned columns remain anchored (sticky) to the left margin immediately following the permanently anchored Gutter, while unpinned columns scroll smoothly underneath.
3. A subtle **Partition Divider** with scroll elevation indicates when content is scrolled underneath the anchored columns.
4. Word wrap is deprecated in favor of unconstrained horizontal scrolling with Sticky Columns, backed by the dedicated Inspector Panel (`⌘I`) for multi-line JSON formatting and deep inspection.
5. Column widths are automatically determined upon log ingestion via an $O(1)$ monospace character advance scan, bounded from below by a minimal safety floor (`min_width`) to protect interactive hitboxes from collapsing during manual drag-resizing.
6. A quick command in the Command Palette (`⌘K`) allows users to unpin all columns at once.

## User Stories

1. As a developer debugging long log payloads, I want the line number and marker Gutter to remain permanently anchored on the left during horizontal scrolling, so that I never lose line context.
2. As a developer inspecting wide log entries, I want to hover over the `TIMESTAMP` column header to reveal a pin toggle icon, so that I can discover the ability to pin it.
3. As a developer, I want to click the pin toggle icon on the `TIMESTAMP` column to pin it, so that the timestamp remains visible when I scroll horizontally across long messages.
4. As a developer, I want a pinned column's icon to remain persistently visible and highlighted in the theme's accent color, so that I can immediately identify which columns are anchored.
5. As a developer, I want to click the pin toggle on an already-pinned column to unpin it, so that the column smoothly returns to its natural scrollable position.
6. As a developer hovering over an already-pinned column's icon, I want visual feedback indicating an unpin action (such as an unpin icon), so that I understand clicking will release the anchor.
7. As a developer, I want to pin the `MODULE` column, so that I can track which microservice or tag produced an error as I scroll deep into JSON payloads.
8. As a developer, I want to pin the `LEVEL` column, so that I can maintain visibility of severity levels when scanning horizontally.
9. As a developer, I want the `MESSAGE` column to be permanently unpinnable, so that the primary log payload always serves as the main scrollable content.
10. As a developer who pins `MODULE` and then pins `TIMESTAMP`, I want the anchored columns to automatically arrange themselves in their natural schema order (`TIMESTAMP` before `MODULE`), so that my reading order remains predictable.
11. As a developer scrolling horizontally through logs, I want unpinned columns to slide smoothly underneath anchored columns, so that the transition feels natural and visually continuous.
12. As a developer with scrolled content, I want a subtle vertical border and elevation shadow to appear at the right boundary of the anchored section, so that I can visually discern that content is scrolling underneath.
13. As a developer at scroll position 0, I want the elevation shadow on the partition divider to disappear, so that the interface looks clean and flat when no content is obscured.
14. As a developer using a trackpad or mouse wheel, I want a single horizontal scrollbar that spans the full width of the canvas, so that the scrollbar does not jump or resize when I toggle column pins.
15. As a developer opening a log file, I want all data columns (`TIMESTAMP`, `LEVEL`, `MODULE`) to start unpinned by default, so that I have maximum viewport space for log messages.
16. As a developer opening a log file with long timestamps (e.g. ISO-8601 with microseconds), I want the column width to automatically expand on load to fit the longest timestamp without manual adjustments.
17. As a developer working with deep package names (e.g. Java/Rust module paths), I want the `MODULE` column to automatically size to accommodate the longest module string without arbitrary truncation.
18. As a developer viewing logs, I want column auto-sizing to use an $O(1)$ monospace character advance calculation upon file load, so that opening 500,000 lines remains instantaneous and does not freeze the UI.
19. As a developer, I want column headers to expose a drag-to-resize handle on their right boundary, so that I can manually widen or narrow any column to my preference.
20. As a developer resizing columns, I want a safety floor (`min_width`) of 36–48px enforced, so that I cannot accidentally shrink a column to zero width or lose its interactive hitbox.
21. As a developer who has manually resized a column, I want to double-click its resize handle to trigger auto-fit, so that it snaps back to the width of its longest content.
22. As a developer with multiple pinned columns, I want to open the Command Palette (`⌘K`) and execute `Columns: Unpin All`, so that I can reset all columns to the default scrollable state using the keyboard.
23. As a developer with pinned columns who clicks on a row, I want the Inspector panel (`⌘I`) to display the complete, unclipped log payload with multi-line wrapping and key-value formatting, so that I can inspect complex data structures without layout distortion.
24. As a developer, I want row selection and quick-filter clicks (clicking on Level to filter by severity, or clicking on Module to filter by tag) to continue working seamlessly on sticky columns, so that pinning does not break existing interactive workflows.

## Implementation Decisions

- Architectural alignment with ADR 0001 (Unified Canvas), ADR 0002 (Deprecate Word Wrap), and ADR 0003 (Dynamic Column Sizing).
- State ownership inside `AppView` with decoupled domain layout calculations.
- Highest test seam on `ColumnLayoutState`.
