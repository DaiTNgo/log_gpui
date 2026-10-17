Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Introduce a multi-tab Working Set bar positioned immediately above the Unified Log Canvas, decoupling the application from the single-file view assumption.

Engineers can have multiple log files open concurrently in their working set (e.g. `api.log`, `worker.log`, `nginx.log`). Each tab maintains its own independent session state:
- Loaded log entries and visible indices
- Active filter engine conditions
- Active search query and matching state
- Column layout, widths, and pinned columns
- Scroll handle position and selected row cursor

The Tab Bar renders each open log with its file name, an indicator of error/warning presence, an active focus indicator, and a close button (`×`). Selecting a tab swaps the active log stream into the canvas with sub-16ms latency. Tabs can be switched using mouse clicks or keyboard shortcuts (`⌃Tab`, `⌃⇧Tab`, `⌘1`–`⌘9`), and closed via the `×` button, middle-click, or `⌘W`. Closing a tab removes it from the working set without removing the file from the project.

All working set operations are governed by a deterministic headless domain state machine (`WorkingSet`), which is tested thoroughly without GUI dependencies.

## Acceptance criteria

- [ ] `WorkingSet` manages an ordered list of open tabs and tracks the currently active tab index.
- [ ] Each tab preserves its own independent filter conditions, search state, column layout, scroll offset, and selected row pos when switching between tabs.
- [ ] Clicking a tab in the Tab Bar activates that tab and displays its log entries in the Log Canvas.
- [ ] Opening a file from the sidebar adds a new tab to the working set, or focuses its existing tab if already open (no duplicate tabs for the same file).
- [ ] Clicking the `×` affordance on a tab, middle-clicking the tab, or pressing `⌘W` closes the active tab and switches focus to the nearest adjacent tab.
- [ ] Keyboard navigation shortcuts (`⌃Tab`, `⌃⇧Tab`, `⌘1` through `⌘9`) switch between open tabs predictably.
- [ ] The Tab Bar handles overflow gracefully with horizontal scrolling or indicator cues when more tabs are open than can fit in the viewport width.
- [ ] Closing a tab preserves the underlying log file registration in the project file list.
- [ ] Deterministic headless tests verify tab addition, tab deduplication, tab closure, adjacent index resolution, and independent session state preservation.

## Blocked by

None - can start immediately
