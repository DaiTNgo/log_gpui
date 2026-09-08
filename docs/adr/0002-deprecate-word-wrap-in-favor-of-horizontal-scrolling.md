# 0002. Deprecate Word Wrap in Favor of Horizontal Scrolling with Sticky Columns

## Context

The log canvas previously offered a `word_wrap` toggle. However:
1. The canvas relies on `uniform_list` for high-performance virtualization (100k+ rows), which strictly enforces a uniform row height (e.g. 24px in Compact mode). Wrapped multi-line text cannot cleanly expand row heights in `uniform_list`.
2. As the number of columns grows (custom fields, tags, trace IDs), fitting all columns within the window width (`FitList`) severely crushes column widths and truncates the `MESSAGE` column.
3. Word wrapping removes horizontal scrolling, which renders the Sticky Column feature ineffective.

## Decision

We **completely drop the `word_wrap` feature**. The canvas operates exclusively in unconstrained horizontal mode (`ListHorizontalSizingBehavior::Unconstrained`).

Users inspect long lines via:
1. Smooth horizontal scrolling accompanied by **Sticky Columns** (keeping critical metadata anchored).
2. The dedicated **Inspector Panel (`⌘I`)** for multi-line JSON formatting, full raw payloads, and wrapping.
3. The **Context View (`Enter`)** for surrounding stream lines.

## Consequences

- Simplifies the canvas layout pipeline and removes dead toggle logic.
- Guarantees each log row remains exactly one line high, maximizing rendering performance.
- Establishes horizontal scrolling with Sticky Columns as the sole, unified navigation paradigm.
