# Spec: Interactive Column Pinning, Sticky Horizontal Scroll, and Dynamic Sizing

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

### Modules & Architecture

1. **Column State Engine (`ColumnLayoutState`)**:
   - A dedicated state struct manages column definitions, pin states, widths, and layout offsets.
   - Maintains a set of active pinned column identifiers (`PinnableColumn::Timestamp`, `PinnableColumn::Level`, `PinnableColumn::Module`).
   - Computes layout positions for any given horizontal scroll offset (`scroll_x`).
   - Tracks dynamic widths calculated during log ingestion and manual user resize overrides.

   *Prototype Type Shape*:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
   pub enum PinnableColumn {
       Timestamp,
       Level,
       Module,
   }

   pub struct ColumnLayoutState {
       pub pinned_columns: BTreeSet<PinnableColumn>,
       pub widths: HashMap<PinnableColumn, Pixels>,
       pub min_widths: HashMap<PinnableColumn, Pixels>,
       pub gutter_width: Pixels,
       pub is_resizing: Option<(PinnableColumn, Pixels)>,
   }
   ```

2. **Application State Integration**:
   - The central application view owns `ColumnLayoutState`.
   - Actions for `toggle_pin_column`, `unpin_all_columns`, and `set_column_width` update state and trigger reactive canvas re-renders.
   - When new log streams are ingested, the parser loop calculates `max_chars` per field and initializes `ColumnLayoutState` widths.

3. **Unified Canvas & Header Rendering**:
   - Headers and rows are rendered using layout offsets derived from `ColumnLayoutState`.
   - At `scroll_x = 0`, items are rendered in natural order: Gutter $\rightarrow$ `TIMESTAMP` $\rightarrow$ `LEVEL` $\rightarrow$ `MODULE` $\rightarrow$ `MESSAGE`.
   - When `scroll_x > 0`:
     - Pinned columns are anchored immediately adjacent to the Gutter (offset `88px`), ordered by their natural schema sequence (`TIMESTAMP` $\rightarrow$ `LEVEL` $\rightarrow$ `MODULE`).
     - Unpinned columns receive an offset shifted by `-scroll_x`.
     - A partition divider element with border and shadow is placed at the right boundary of the sticky section.
   - Each column header includes an interactive pin button (with hover reveal and active states) and a resize handle.

4. **Deprecation of Word Wrap**:
   - The `word_wrap` toggle is removed from the toolbar, application state, and canvas options.
   - The virtualized list operates in unconstrained horizontal mode (`ListHorizontalSizingBehavior::Unconstrained`).
   - Multi-line reading needs are met by the Inspector Panel and Context View.

5. **Command Palette Integration**:
   - Adds an `UnpinAllColumns` action to the palette action enum, allowing keyboard-driven resets.

## Testing Decisions

### What Makes a Good Test

Tests must verify observable behaviors, state transitions, and layout invariants—never internal styling or DOM hierarchy. Tests should validate:
- Ingestion auto-fit calculations for varied content lengths.
- Hitbox safety floor clamping during simulated resize actions.
- Pin and unpin state transitions.
- Strict schema order preservation when toggling multiple pins out of order.
- Exact sticky and scrolled offset calculations across different `scroll_x` coordinates.

### Tested Modules & Test Seams

- **Primary Test Seam**: `ColumnLayoutState`. This represents the highest headless test seam. It encapsulates all sizing calculations, pin state transitions, schema ordering rules, and sticky coordinate resolution without requiring a live GPU window context.
- **Ingestion Scan Seam**: Ingestion width computation functions, verifying character-to-pixel conversions using monospace advance width across diverse log entries.

### Prior Art in Codebase

- `tests/filter_test.rs`: Tests `FilterEngine` and `SearchState` as decoupled state machines handling complex filtering logic and marker indexing independent of UI elements.

## Out of Scope

- Drag-and-drop reordering of columns.
- Pinning columns to the right-hand viewport boundary (all pins anchor to the left).
- Persisting column layout preferences to persistent disk storage across application restarts (state lives in memory for the active window session).
- Per-cell word wrapping inside virtualized grid rows.

## Further Notes

- Monospace character advance width is computed as $W_{\text{char}} = 0.6 \times \text{font\_size}$ based on standard macOS and Zed font metrics (`.AppleSystemUIFontMonospaced`).
- The architecture cleanly supports future extensible log columns (e.g. Thread, TraceId, SpanId, or custom JSON keys) by implementing `PinnableColumn` variants.
