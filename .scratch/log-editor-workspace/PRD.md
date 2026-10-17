Status: ready-for-agent

# PRD: Log Editor Workspace Hierarchy, Multi-Tab Working Set, and Investigation Navigation

## Problem Statement

When diagnosing modern distributed systems, production incidents rarely occur within a single isolated log file. An engineer troubleshooting a payment failure, for example, must correlate events across `api.log`, `worker.log`, `nginx.log`, and `database.log`.

Currently, `log_gpui` operates under a 1:1 "single log file viewer" mental model. The user opens one file (`app.log`), views its contents in the canvas, and inspects rows. Switching to another file completely obliterates the current working context—including active search queries, filter pipelines, row selections, line markers, and scroll positions.

Furthermore:
- **Project is conflated with a single file**: In real-world incident forensics, a "Project" is an investigation space containing multiple log files, environment configurations, and investigative findings, not a single static file on disk.
- **Lack of Working Set (Tabs)**: Engineers cannot hold multiple logs in flight simultaneously. Comparing timestamps or cross-referencing request IDs between an API gateway and an asynchronous worker requires repeatedly re-opening files and re-applying filters.
- **No Concept of Investigation / Incident Session**: When an engineer identifies critical root-cause lines across disparate logs, there is no way to pin them into a coherent case file (e.g., `INC-142: Payment Timeout`). Quitting the application drops all markers and filters, forcing the user to reconstruct their forensic trail from scratch.
- **Sidebar Clutter vs. Cognitive Overload**: Flat sidebars that list all files and projects concurrently cause visual noise and cognitive bleed during high-stress on-call triage.

## Solution

Transform `log_gpui` from a simple log viewer into a dedicated **Log Editor & Incident Workspace** based on a strict 3-tier architectural hierarchy:

$$\mathbf{Workspace\ /\ Project} \longrightarrow \mathbf{Working\ Set\ (Open\ Tabs)} \longrightarrow \mathbf{Log\ Editor\ Canvas}$$

1. **Project as an Investigation Space**: A Project represents a cohesive system or service cluster (e.g. `Payment Service`, `E-Commerce Core`). It owns a registry of log files, saved filter presets, investigation cases, cross-file markers, and session layouts.
2. **Project Switcher**: A minimalist, high-speed breadcrumb/dropdown switcher located at the top-left chrome. It allows switching between projects in under 16ms with zero visual bleed between unrelated services.
3. **Multi-Tab Working Set Bar**: Positioned directly above the Unified Canvas. Tabs represent the engineer's active working memory. A tab can host an individual log file (`api.log`), an investigation casefile (`INC-142`), or a merged chronological chronicle. Closing a tab removes it from working memory without deleting it from the Project.
4. **Contextual Navigation Drawer & Activity Rail**: A 40px Activity Rail (`Files`, `Investigations`, `Saved Views`, `Markers`, `Settings`) paired with an expandable 240px Contextual Drawer. It provides progressive disclosure, keeping the screen minimal and focused while offering deep navigation when needed.
5. **Investigation Cases & Cross-File Evidence Trail**: Engineers can create named investigation cases (e.g. `INC-142: Upstream Timeout`) and attach log line anchors across multiple open files. Markers synchronize across logs via timestamp correlation.
6. **Persistent Log Sessions**: Project workspace state—including open tabs, active tab index, filter conditions, search queries, pinned columns, and cursor positions—is automatically preserved across app restarts.

## User Stories

### Project Organization & Switching
1. As an SRE debugging an incident, I want to create a Project named "Payment Service" containing multiple log files (`api.log`, `worker.log`, `nginx.log`), so that all related system logs are grouped under one investigation domain.
2. As a developer, I want to see the currently active project name prominently displayed in the top-left window header, so that I always know my working context at a glance.
3. As a developer, I want to click the Project Switcher header button (or press `⌘P` / `⌘⌥P`) to open a fast fuzzy-search palette of recent projects, so that I can switch projects instantly without cognitive friction.
4. As an engineer managing multiple environments, I want each project to have an identifiable color accent badge (e.g. Blue for Payment, Green for E-commerce, Amber for Production), so that I don't accidentally execute triage actions in the wrong environment.
5. As a developer switching from Project A to Project B, I want the active tabs, canvas logs, and filters to switch cleanly, so that unrelated log data never bleeds into my current investigation.
6. As a developer, I want to add a new project by selecting a local directory containing log files, so that all detected `.log` files are automatically registered into the new project.
7. As a developer, I want to remove a project from my workspace without deleting the underlying log files from disk, so that my file system remains safe.
8. As a developer, I want to rename an existing project inline, so that I can adapt the project label as our service taxonomy evolves.

### Working Set & Tab Bar Navigation
9. As an engineer comparing logs, I want a persistent Tab Bar directly above the Log Canvas displaying all currently open logs in my active working set, so that I can toggle between logs with a single click.
10. As a developer, I want to click a file in the sidebar's Files drawer to open it as a new tab in the Tab Bar, so that it becomes part of my active working set.
11. As a developer, I want clicking an already-open file in the sidebar to activate its existing tab rather than creating a duplicate, so that my tab bar stays tidy.
12. As a developer, I want to close any open tab by clicking its `×` button or pressing `⌘W`, so that I can remove logs from my working set when I am done with them.
13. As a developer, I want closing a tab to preserve the underlying file registration in the Project, so that I can reopen it later from the Files drawer.
14. As a developer, I want to switch between open tabs using keyboard shortcuts (`⌃Tab`, `⌃⇧Tab`, or `⌘1`–`⌘9`), so that I can navigate rapidly between services without leaving the keyboard.
15. As a developer, I want each tab to display the file name and an indicator badge showing how many error-level logs or markers exist in that file, so that I can spot trouble areas across services at a glance.
16. As a developer with many open tabs, I want the Tab Bar to support smooth horizontal scrolling or tab overflow truncation, so that wide collections of open logs do not distort window chrome.
17. As a developer, I want each tab to maintain its own independent scroll position, selected row, search query, and active filter pipeline, so that switching tabs never resets my place in the log.
18. As a developer, I want to middle-click a tab to close it, matching standard native desktop ergonomics.
19. As a developer, I want to drag and drop tabs along the Tab Bar to reorder my working set, so that I can position related logs side by side.

### Activity Rail & Contextual Drawer (Progressive Disclosure)
20. As a developer who values screen real estate, I want a 40px Activity Rail on the far left that lets me toggle drawers for Files, Investigations, Saved Views, Markers, and Settings, so that navigation controls do not crowd the log stream.
21. As a developer, I want clicking the active rail icon to collapse the 240px drawer into Compact Focus Mode, so that 100% of horizontal window space is dedicated to the Log Canvas and Sticky Columns.
22. As a developer in Compact Focus Mode, I want clicking any rail icon to smoothly expand the Contextual Drawer, so that I can access project assets on demand.
23. As a developer, I want keyboard shortcuts (`⌘B` to toggle drawer, `⌘⇧E` for Files, `⌘⇧I` for Investigations) to control sidebar visibility, so that I can expand and collapse drawers without a mouse.
24. As a developer viewing the Files drawer, I want to see registered project files grouped by subfolder or categorized by service, so that large file sets remain organized.
25. As a developer viewing the Files drawer, I want a "+ Add Log File..." action to register external log files into the active project, so that ad-hoc logs can be ingested quickly.
26. As a developer, I want hover actions on files in the Files drawer (Open in New Tab, Close, Remove from Project), so that file management is direct and discoverable.

### Investigation Cases & Evidence Trails
27. As an incident commander, I want to create an Investigation Case (e.g. `INC-142: Payment Gateway Timeout`) inside the active project, so that all evidence related to a specific outage is gathered in one place.
28. As a developer inspecting a row in `api.log`, I want to right-click or press a hotkey (`⌘M` / `⌘K → Pin to Investigation`) to anchor that row into the active Investigation Case, so that I capture the exact moment of failure.
29. As a developer anchoring a log line, I want to attach a markdown note (e.g. "Upstream Stripe TCP SYN timeout observed"), so that I preserve my diagnostic hypothesis alongside the raw log entry.
30. As a developer opening an Investigation Case in the Tab Bar, I want to see a unified Chronological Evidence Trail displaying all anchored lines from all participating files in chronological order, so that I can see the exact timeline of cross-service degradation.
31. As a developer viewing an evidence trail item, I want clicking it to navigate directly to that file, open it in a tab if necessary, and scroll smoothly to the target line, so that verifying raw context is instantaneous.
32. As an SRE concluding an incident, I want an "Export Post-Mortem Markdown" action that converts the Investigation Case and all anchored log snippets into a clean markdown document, so that writing the incident report requires zero manual copy-pasting.
33. As an engineer working with active cases, I want each investigation to have a status state (`Active`, `Root Cause Identified`, `Mitigated`, `Archived`), so that team members can track investigative progress.

### Saved Views & Multi-Log Correlations
34. As a developer who frequently filters for 5xx errors across multiple microservices, I want to save an active filter and column configuration as a "Saved View" within my project, so that I can re-apply complex filter chains with one click.
35. As a developer, I want Saved Views to appear in the dedicated Views drawer, so that I can execute them on any active log file.
36. As a developer correlating distributed events, I want to click a `request_id` or `trace_id` in `api.log` and choose "Search Across All Project Files", so that I can trace a transaction through `worker.log` and `nginx.log` without manual copy-paste.
37. As an engineer inspecting two open tabs, I want an optional "Synchronize Timestamp Scrolling" toggle in the status bar, so that scrolling one log file automatically aligns the other open log tab to the closest timestamp.

### Session Persistence & Lifecycle
38. As a developer closing `log_gpui` at the end of the day, I want the active project, open working set tabs, active filters, search queries, and window split state to be automatically persisted to disk, so that tomorrow morning I resume exactly where I left off.
39. As a developer reopening the app after a crash or OS reboot, I want my active investigations and bookmarked lines to be fully intact, so that forensic progress is never lost.
40. As a developer opening `log_gpui` with CLI arguments (e.g. `log_gpui ./logs/`), I want the app to automatically mount that directory as an ephemeral or named project, so that CLI-driven workflows remain fast and seamless.

## Implementation Decisions

### Architectural Separation: The 3-Tier Domain Model
The codebase decouples project workspace orchestration from raw canvas rendering. A dedicated domain module manages workspace entities independently of GPUI view components:

1. **Tier 1: `WorkspaceState` & `Project`**:
   - `Project`: The root unit of persistence and isolation. Owns a list of `LogFileDescriptor`s, `SavedView`s, `InvestigationCase`s, and `ProjectSessionState`.
   - `WorkspaceState`: Holds all registered projects, an `active_project_id`, and handles switching between projects.
2. **Tier 2: `WorkingSet` & `WorkspaceTab`**:
   - `WorkspaceTab`: Represents an open entity in the tab bar:
     ```rust
     pub enum TabTarget {
         LogFile { file_id: String },
         InvestigationBoard { case_id: String },
         ChronicleStream { file_ids: Vec<String> },
     }
     ```
   - Each tab retains its own independent `TabSessionState`:
     - Active `FilterEngine` condition tree
     - Active `SearchState` (query, case-sensitivity, regex mode, match index)
     - `ColumnLayoutState` (width adjustments, pinned column set)
     - Scroll offsets (`scroll_x`, `UniformListScrollHandle` position)
     - Selected row index and cursor position
3. **Tier 3: `LogCanvas` (The Existing View Surface)**:
   - `LogCanvas` remains a pure rendering projection. When the active tab or project changes, `LogCanvas` is fed the entries and session state of the currently selected tab. All existing Sticky Column, Dynamic Monospace Sizing, and Gutter rules from ADR 0001–0003 continue to govern the canvas without modification.

### Navigation Architecture: Switcher + Activity Rail
- **Project Switcher**: Placed in the top window chrome replacing the static title. Emits a modal palette with recent projects, quick-creation affordance, and keyboard navigation.
- **Activity Rail**: 40px vertical icon bar on the leftmost edge. Tabs:
  - `Files` (`IconName::Folder`): Project file tree and log mounting.
  - `Investigations` (`IconName::Layers` / `IconName::BookOpen`): Incident case files, active investigation boards, and evidence anchors.
  - `SavedViews` (`IconName::Sliders` / `IconName::ListFilter`): Presets for filter pipelines and column arrangements.
  - `Markers` (`IconName::Bookmark`): Global list of bookmarked/flagged lines in the project.
  - `Settings` (`IconName::Settings`): Parser schemas, color rules, and project metadata.
- **Drawer State Engine**:
  - `SidebarState::Hidden` (fully collapsed)
  - `SidebarState::Compact` (40px rail only, drawer hidden)
  - `SidebarState::Expanded` (40px rail + 240px contextual drawer)

### High-Performance Ingestion & Memory Management
- Inactive tabs maintain their parsed entry index structures in memory (Arc-backed) or unload raw line payloads lazily if memory thresholds are reached, ensuring tab switching is instant ($< 16\text{ms}$).
- Time-synchronization uses binary search ($O(\log N)$) across monotonically increasing timestamps in parsed log entries.

## Testing Decisions

### Test Seam
The primary test seam is established at the highest possible headless domain boundary: **`WorkspaceState`**.

- **Why this seam?**
  `WorkspaceState` encapsulates 100% of the project lifecycle, working set operations, tab switching, investigation anchoring, and session persistence.
  Testing at `WorkspaceState` verifies the full domain state machine deterministically without launching a GPUI application window, allocating GPU framebuffers, or depending on OS-level window servers.
- **Quality of Tests**:
  - Tests assert observable user behaviors: opening a file adds a tab; switching tabs restores the exact filter and scroll state; anchoring a line across files attaches it to the active investigation case; switching projects isolates files and working sets.
  - Tests do not assert GPUI rendering internals, DOM-like element trees, or styling tokens.
- **Prior Art**:
  Mirrors the headless state testing pattern established in `tests/column_test.rs` (`ColumnLayoutState`), which exercises pinning invariants, horizontal scrolling calculations, and dynamic monospace widths with zero GPU overhead.

## Out of Scope

- Remote cloud synchronization or multi-user real-time collaborative editing.
- Live SSH / remote agent log streaming (file ingestion remains local filesystem and local stdin/pipes).
- Complex time-series charting / histogram widget rendering in the Tab Bar (reserved for a future Metrics feature).
- Arbitrary text editing of raw log files on disk (the app is an incident annotation editor, not a destructive raw text modifier).

## Further Notes

- Maintains full backwards compatibility with CLI single-file invocations: running `log_gpui /path/to/server.log` automatically creates an ad-hoc project and immediately opens `server.log` in the active working set.
- All keyboard shortcut contracts align with Zed/VS Code conventions (`⌘P` project switcher, `⌘W` close tab, `⌘B` toggle sidebar, `⌘1`-`⌘9` select tab).
