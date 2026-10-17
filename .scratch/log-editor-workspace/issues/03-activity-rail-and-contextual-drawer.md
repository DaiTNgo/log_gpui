Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Refactor the navigation sidebar into a dedicated **40px Activity Rail** paired with an expandable **240px Contextual Drawer**, delivering progressive disclosure that preserves screen space for the log stream.

The Activity Rail features 5 dedicated modules:
1. `Files` (`IconName::Folder`): Project file tree, subdirectories, and log registration
2. `Investigations` (`IconName::Layers` / `BookOpen`): Incident case files and active investigations
3. `Saved Views` (`IconName::Sliders` / `ListFilter`): Preset filter and column pipelines
4. `Markers` (`IconName::Bookmark`): Aggregated bookmarks and line flags across all project logs
5. `Settings` (`IconName::Settings`): Project schema, parser settings, and color configurations

The system implements progressive disclosure with two primary modes:
- **Compact Focus Mode**: The contextual drawer collapses to 0px, dedicating maximum viewport width to the Log Canvas and Sticky Columns.
- **Expanded Mode**: The drawer expands to 240px with smooth micro-typography headers and interactive tree rows.

Interactions:
- Clicking an inactive rail icon activates that tab and expands the drawer.
- Clicking the currently active rail icon collapses the drawer into Compact Focus Mode.
- Global keyboard shortcuts (`⌘B` to toggle drawer, `⌘⇧E` for Files, `⌘⇧I` for Investigations) provide rapid keyboard accessibility.
- Files drawer displays registered project files, highlights which files are currently open in the Tab Bar, and provides hover actions to open in new tab or add external log files.

## Acceptance criteria

- [ ] Activity Rail renders 5 distinct icon tabs (`Files`, `Investigations`, `Saved Views`, `Markers`, `Settings`) with hover and active selection styling.
- [ ] Clicking the active rail icon toggles the contextual drawer between Expanded (240px) and Compact Focus Mode (0px).
- [ ] Pressing `⌘B` toggles drawer visibility regardless of focus location.
- [ ] Pressing `⌘⇧E` expands the Files drawer; pressing `⌘⇧I` expands the Investigations drawer.
- [ ] Files drawer displays registered project logs, optionally grouped by subfolder/service directory.
- [ ] Files currently open in the active Working Set display an open indicator badge in the drawer.
- [ ] Hovering over a file in the drawer exposes direct action triggers ("Open in New Tab", "Close Tab").
- [ ] An "+ Add Log File..." affordance at the bottom of the Files drawer allows registering additional log files into the project.
- [ ] Headless unit tests verify rail state transitions, drawer toggle logic, and active tab routing.

## Blocked by

- .scratch/log-editor-workspace/issues/01-multi-tab-working-set-bar.md
