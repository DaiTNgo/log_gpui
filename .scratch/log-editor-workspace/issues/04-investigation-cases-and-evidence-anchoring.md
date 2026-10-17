Status: ready-for-agent

## Parent

.scratch/log-editor-workspace/PRD.md

## What to build

Implement incident-centric **Investigation Cases** and cross-file **Evidence Anchoring**, enabling engineers to capture and correlate root-cause findings across multiple disparate log files.

An Investigation Case represents an active incident inquiry (e.g. `INC-142: Payment Upstream Timeout`). It owns:
- Case identifier (`INC-142`), title, severity, and status (`Active`, `Root Cause Identified`, `Mitigated`, `Archived`)
- Chronological collection of `LogAnchor` items gathered across any number of project log files
- Diagnostic markdown notes attached to individual anchors or the case overall

Interactions & Workflow:
1. Users can create a new Investigation Case from the `Investigations` drawer or via Command Palette (`⌘K → New Investigation Case`).
2. While reading any log file in the canvas, pressing `⌘M` (or clicking a context affordance) anchors the selected log line to the active Investigation Case, prompting for an optional diagnostic note.
3. The Investigations drawer lists all cases with their status, severity badges, and anchor counts.
4. Opening an Investigation Case renders a dedicated **Investigation Board** tab in the Tab Bar.
5. The Investigation Board presents a unified **Chronological Evidence Trail**, listing anchored log entries from all participating services sorted strictly by timestamp.
6. Clicking any anchored item in the evidence trail automatically opens that log file in the working set (if not already open) and scrolls the canvas smoothly to the exact line number.
7. An "Export Post-Mortem Markdown" action generates a structured markdown summary containing incident metadata, diagnostic notes, and exact log excerpts formatted for team incident reviews.

## Acceptance criteria

- [ ] Users can create, rename, archive, and delete Investigation Cases within the active project.
- [ ] Users can anchor any selected log line into an investigation case, preserving file reference, line number, timestamp, log level, and payload.
- [ ] Users can attach, edit, and view Markdown notes on any anchored log line.
- [ ] Investigation Board renders as a tab in the Tab Bar with an evidence trail view sorted chronologically.
- [ ] Clicking an anchor item in the board switches to the target log file tab and scrolls directly to the anchored line.
- [ ] Tabs in the Tab Bar display an indicator dot if lines from that file are attached to the active investigation.
- [ ] An "Export Post-Mortem Markdown" button copies or exports the case summary and formatted log snippets.
- [ ] Headless unit tests verify case creation, multi-file anchoring, timestamp sorting across heterogeneous logs, navigation resolution, and markdown export formatting.

## Blocked by

- .scratch/log-editor-workspace/issues/01-multi-tab-working-set-bar.md
- .scratch/log-editor-workspace/issues/02-project-switcher-and-workspace-isolation.md
