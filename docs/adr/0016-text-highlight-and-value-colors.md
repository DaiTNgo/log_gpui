# 0016. Text Highlight Paints Every Place; Value Colors Are Automatic

## Context

A Marker (ADR-0007) colors one stretch of one line. Following an id such as `conn-7f3a` through thousands of lines had no help on the main view: Search Highlight paints only the Focus Pane, and a Highlight Rule tints whole rows by one column's value. In a Log Space that interleaves several files, threads, requests and hosts run through each other with nothing that tells them apart at a glance. Other log viewers answer both: klogg colors every occurrence of selected text, lnav gives every identifier value a stable color of its own.

## Decision

- A **Text Highlight** is a stretch of text and a color, kept by the Project. Every place that text appears (ignoring case, no regex, as Search is text only per ADR-0004) is painted behind, in the Raw View, Parsed View and Focus Pane. It is made from the same color bar as a Marker: the bar opens on "This line only" every time and offers "Everywhere “text” · N", and Alt+click on a color makes a Text Highlight directly. It is recolored and removed from the context menu of any place it paints, and removing it removes it everywhere; there is no list of them.
- A Marker stays: it is the place the user found, a Text Highlight helps find every place. Painting a common word like `failed` everywhere cannot point at the one line that matters.
- A **Value Color** is a text color the app works out from a cell's value (case-folded, the same FNV-1a hash as Phrase colors), for every column except timestamp, message/payload, level and columns of numbers, booleans or JSON. It is automatic: no per-column switch and no per-value color, only one app-wide switch in Settings. Its hue moves away from the colors of the Project's Text Highlights.
- Layers: backgrounds are Marker, then Search Match (Focus Pane), then Text Highlight, then the selected row and Highlight Rule. Text color is chosen for contrast on a Marker or Text Highlight, else the Value Color, else the plain color. What the user paints has a background; what the app paints only colors text.

## Considered Options

- **Text Highlight as a Marker with a scope flag**: rejected, a Marker is keyed by line and Generation and is lost when its line leaves the Live Window; a Text Highlight is keyed by text and outlives lines. One type would carry two sets of rules.
- **"Every value of column X" as a third bar scope, to pin a Value Color**: rejected, Value Colors are automatic and need no per-value control; a Text Highlight covers the rare case.
- **Value Colors only on Filterable Columns (at most 500 values)**: rejected, identifier columns such as `request_id` are the main case and have far more values. The color is worked out while painting, so the count does not matter.
- **Two rows of colors in the bar, or Alt+click only**: rejected, two rows double the bar that opens on every selection; Alt alone hides the feature.

## Consequences

- The Parsed View is more colorful by default; Value Colors can be turned off for the whole app.
- Two different values can share a Value Color; it is a visual hint, not an identity.
- A value's color can move when the user adds a Text Highlight of a close hue.
