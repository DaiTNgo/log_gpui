Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Replace the Activity Rail + Contextual Drawer with two independent sidebars (the Inspector remains a third, separate panel):

1. **Left sidebar — Projects** (240px, resizable 180–500px). Shows the project tree: each project expands into its registered log files. **Settings** is pinned at the bottom of this sidebar.
2. **Right context sidebar — Files / Investigations / Saved Views / Markers** (260px). Scoped to the active project and the focused log file. Only available while a **log file tab** is focused; its toggle appears in the window header only then.

Window header (36px, macOS, transparent titlebar so traffic lights float over it):

`[traffic lights] [toggle left sidebar] [project badge + name ⌄] [file pill] ……… [⌘K Palette] [Open…] [toggle right sidebar*]`

\* shown only when a log file tab is focused.

- When not in full screen, the header reserves ~80px on the left for the traffic lights. In full screen the inset drops to 12px and the toggle sits at the left edge.
- Left sidebar has two states only: **Visible** and **Hidden** (no rail, no compact mode). Hidden removes it entirely.
- The header is the window drag area.
- Layout (left → right): left sidebar | log canvas | Inspector | right context sidebar.

Interactions & shortcuts:
- `⌘B`: toggle left (Projects) sidebar. `⌘⌥B`: toggle right context sidebar.
- `⌘⇧B` / `⌘⇧I` / `⌘⇧S`: show the context sidebar on Files / Investigations / Saved Views. (`⌘⇧E` is taken by "previous error"; `⌘⇧M` by the Marker panel.)
- Settings button at the bottom of the left sidebar swaps the project tree for the Settings panel and back.
- Left sidebar project rows: click to switch/expand, `+` to add a log file to that project; header actions: new project, collapse all, expand all. File rows highlight open/active files and offer a hover close-tab action.
- Context sidebar Files section lists the active project's files with open/active indicators, hover close action, and a trailing "+ Add Log File..." affordance. A 4-icon tab strip switches sections.

## Acceptance criteria

- [ ] No activity rail is rendered; the left sidebar is either fully visible or fully hidden.
- [ ] The header toggle for the left sidebar sits next to the macOS traffic lights (not in full screen) and at the left edge in full screen.
- [ ] `⌘B` and the header toggle show/hide the left sidebar; its Settings button (bottom) opens/closes the Settings panel.
- [ ] The left sidebar renders projects with their log files; clicking a file opens/focuses its tab.
- [ ] The right sidebar toggle appears in the header only when a log file tab is focused; the sidebar is not rendered otherwise.
- [ ] The right sidebar exposes Files, Investigations, Saved Views, Markers for the active project and is independent from the Inspector.
- [ ] Headless unit tests cover `LeftSidebarState` (toggle, settings, width clamp) and `ContextSidebarState` (toggle, tab selection, log-file gating).

## Blocked by

- .scratch/log-editor-workspace/issues/01-multi-tab-working-set-bar.md
