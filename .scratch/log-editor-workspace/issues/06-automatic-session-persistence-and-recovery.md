Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Implement a lightweight, non-blocking **Session Persistence & Recovery Engine** to ensure that engineers never lose their active investigative context across application lifecycles.

When working on complex incidents, engineers accumulate hours of carefully tuned filters, row selections, and cross-file tabs. Closing the application or experiencing a system restart currently wipes this entire working memory.

### Persistence Engine
- Automatically writes a compact JSON state file (`~/.config/log_gpui/session.json` or project-local state) on clean application exit and upon switching projects.
- Debounces persistence writes to prevent I/O disk thrashing during rapid filter updates.
- Serialized session payload preserves:
  - List of registered projects and their configurations
  - ID of the currently active project
  - Working set tabs for each project (ordered file paths / case IDs, active tab index)
  - Independent tab sessions (filter conditions, search query & case options, column widths, pinned columns, and scroll positions)
  - Investigation cases, attached notes, and anchors

### Recovery Engine
- On application launch, checks for an existing session file and automatically restores the last active project and its working set.
- If a registered log file was moved or deleted on disk while the app was closed, flags the tab with a non-destructive missing-file warning indicator rather than crashing or dropping the tab.
- When launched via CLI with directory arguments (e.g. `log_gpui ./var/log/`), mounts the target directory as an active project without destroying existing saved project sessions.

## Acceptance criteria

- [ ] Workspace state is serialized to disk upon project change and application termination.
- [ ] Launching the app automatically restores all previously registered projects, the active project, and all open working set tabs.
- [ ] Restored tabs accurately recover their previous filter pipelines, search queries, pinned column states, and scroll positions.
- [ ] If a log file referenced in a session is missing on disk, the UI gracefully renders a "File not found" indicator in the tab and drawer with an option to locate or dismiss it.
- [ ] Launching via CLI with directory arguments creates/mounts the directory as a workspace without corrupting previously saved projects.
- [ ] Headless unit tests verify round-trip serialization and deserialization of workspace, project, working set, and investigation case state.

## Blocked by

- .scratch/log-editor-workspace/issues/01-multi-tab-working-set-bar.md
- .scratch/log-editor-workspace/issues/02-project-switcher-and-workspace-isolation.md
