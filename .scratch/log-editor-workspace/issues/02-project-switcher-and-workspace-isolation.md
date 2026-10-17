Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Elevate working sets into isolated `Project` containers governed by a top-level `WorkspaceState`, and add an interactive **Project Switcher** to the window header.

Each Project represents an isolated investigation space (e.g. `Payment Service`, `E-Commerce Core`) containing:
- Project identity (unique ID, display name, color accent badge)
- File registry (registered log files belonging to this project)
- Active Working Set (ordered open tabs and active tab index)
- Project-level configurations

The window header places an interactive Project Switcher affordance immediately after the left-sidebar toggle (which sits next to the macOS traffic lights), displaying the project's colored badge and name with a dropdown chevron (e.g. `[🟦 Payment Service ⌄]`). Clicking this trigger or pressing `⌘P` / `⌘⌥P` opens a modal project switcher palette that supports:
- Fuzzy-filtering across existing projects
- Highlighting the currently active project
- Adding a new project by selecting a local directory containing log files
- Creating an empty project or renaming an existing project

Switching projects cleanly swaps the active working set, file drawer, and canvas view in under 16ms with strict isolation—ensuring zero data, filter, or marker leakage between distinct microservice environments.

## Acceptance criteria

- [ ] `WorkspaceState` manages multiple `Project` instances, each with its own independent `WorkingSet` and file registry.
- [ ] The window header renders the active project's color badge, name, and a dropdown chevron affordance.
- [ ] Clicking the Project Switcher header or pressing `⌘P` / `⌘⌥P` opens a modal switcher palette with keyboard navigation.
- [ ] Selecting a different project cleanly switches the active context: only that project's open tabs, files, and logs are displayed.
- [ ] Switching back to a previously active project restores its exact working set and tab states as they were left.
- [ ] Users can create a new project by specifying a name and mounting a directory of log files.
- [ ] Users can rename an existing project or remove a project without deleting underlying files on disk.
- [ ] Headless unit tests verify project creation, switching, context isolation, and workspace state integrity.

## Blocked by

- .scratch/log-editor-workspace/issues/01-multi-tab-working-set-bar.md
