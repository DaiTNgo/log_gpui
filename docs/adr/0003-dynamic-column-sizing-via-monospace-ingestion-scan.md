# 0003. Dynamic Column Sizing via Monospace Ingestion Scan and Interactive Resize

## Context

Log data contains variable-length metadata across rows (e.g. varying timestamp formats, long module or package names, and future extensible fields). Hardcoding column widths leads to truncated data or wasted space, while measuring text elements on every frame across hundreds of thousands of rows causes severe UI freezing.

Imposing an arbitrary `max_width` ceiling is problematic: it introduces artificial magic numbers and truncates legitimate long identifiers (e.g. deep Java/Rust package paths) unnecessarily, especially since the canvas fully supports unconstrained horizontal scrolling.

## Decision

We adopt an open-ended dynamic column sizing strategy:
1. **$O(1)$ Ingestion Scan with Monospace Character Advance**: During the initial log parsing loop, track the maximum character count for each field. Column widths are derived via:
   $$\text{Width} = \max(\text{min\_width}, (\text{max\_chars} \times W_{\text{char}}) + \text{padding})$$
   where $W_{\text{char}}$ is the advance of one character (`0`) of the theme's monospace font, measured from the font when the app starts (`ui::mono`). The font differs by platform (Menlo ≈ 0.60 em, Consolas ≈ 0.55 em, DejaVu Sans Mono ≈ 0.60 em), so the advance is never a fixed ratio of the font size.
2. **Safety Floor (`min_width`) as Hitbox Protection**: We enforce a minimal safety floor (~36–48px) strictly to prevent columns from being collapsed into zero-width oblivion during manual drag-resize. This ensures column headers, pin toggles, and resize handles always remain visible and interactive.
3. **No Arbitrary `max_width`**: Columns expand to fit their real data naturally without arbitrary truncation.
4. **Interactive Column Resizing**: Column headers expose a resize handle on their right boundary. If a user wishes to narrow a wide column, they can drag to adjust widths manually or double-click to auto-fit back to maximum content width.

## Consequences

- Completely eliminates arbitrary magic numbers (`max_width`).
- Prevents columns from disappearing into 0px unclickable states.
- Guarantees no metadata or message is prematurely clipped upon load.
- Fully leverages unconstrained horizontal scrolling while leaving width control in the user's hands via drag handles.
- A column holding glyphs the monospace font lacks (CJK, emoji, drawn from a fallback font) can fit a little narrow, since the scan counts characters and not their shaped width; the user can widen it by dragging.
- This estimate is for sizing only. Placing a pointer in text (selection, right-click) shapes the line as it is drawn instead of counting characters.
