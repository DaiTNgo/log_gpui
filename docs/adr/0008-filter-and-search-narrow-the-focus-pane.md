# 0008. Filter and Search Narrow the Focus Pane, Not the View

## Context

Search only marked Matches and jumped between them, and Filter hid lines in the Parsed View itself (ADR-0004, ADR-0005). To read the lines around a Match, users had to clear the Filter and lost their place. To see all Matches at once, they had to step through them one by one. Users wanted the whole log and the narrowed lines on screen together, as in klogg.

## Decision

The view (Raw or Parsed) always shows every line. A full-width **Focus Pane** under the view and both sidebars lists only the lines that pass Filter and the Search Expression, each as its raw line with its line number and Source File. Filter no longer hides lines in the view: it narrows only the Focus Pane, and only while the view is the Parsed View. In the Raw View the Focus Pane uses the Search Expression alone. The Match count, next/previous, the overview ruler and the highlights in the view all use the same set as the Focus Pane, which means Matches on lines that fail Filter do not count.

The Focus Pane holds numbered **Focus Tabs** (1, 2, 3, …). Each tab has its own Search Expression, which the toolbar Search field edits, and its own **Focus Find**, which finds and jumps between the lines the tab already lists without narrowing them further. All tabs share the view's Filter. The view and the Focus Pane always follow each other's current line, and opening a tab jumps the view to that tab's current line. Focus Tabs are remembered per view, like Filter. The last tab cannot be closed.

## Considered Options

- **Keep Filter hiding lines in the view**: rejected, the view would no longer be the full log, and the reason for the second pane is to read narrowed lines next to their context.
- **Chained drill-down panes** (each new search narrows the previous pane): rejected, chaining `A` then `B` is the same as `A && B`, and a stack of panes is hard to manage.
- **Marking the Matches of every Focus Tab in the view, one color per tab**: rejected, it would be confused with Markers and Highlight Rules. Only the open tab's Matches are marked.
- **Filter also narrowing the Focus Pane in the Raw View**: rejected, the Raw View ignores Filter everywhere.

## Consequences

- ADR-0004 no longer holds where it says Search never changes what is shown: Search now fills the Focus Pane, though it still never hides lines in the view.
- ADR-0005's Filter keeps its columns, values and rules, but it acts on the Focus Pane instead of the view.
- Copying from the view takes every selected line. Copying from the Focus Pane takes only the lines it lists.
- The Inspector shows only the stored text of the selected line, in both views.
