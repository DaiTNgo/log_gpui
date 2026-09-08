Status: ready-for-agent

# Dynamic Column Sizing via Ingestion Scan & Safety Floor

## Parent

.scratch/column-pinning/PRD.md

## What to build

Implement the dynamic column width computation specified in ADR 0003. When a log file is parsed or ingested, track the maximum string length (`max_chars`) for each structured field (`timestamp`, `level`, `tag/module`) in an $O(1)$ pass.

Calculate each column's display width using the formula:
$$\text{Width} = \max(\text{min\_width}, (\text{max\_chars} \times W_{\text{char}}) + \text{padding})$$
where $W_{\text{char}} = 0.6 \times \text{font\_size}$.

Enforce a minimal safety floor (`min_width` of 36–48px) to protect interactive hitboxes from collapse. Render Column Headers and row data cells according to these dynamically calculated widths instead of fixed hardcoded pixels.

## Acceptance criteria

- [ ] Parsing or loading a log stream tracks maximum character length per field in a single $O(1)$ ingestion pass.
- [ ] Column widths for `TIMESTAMP`, `LEVEL`, and `MODULE` expand to accommodate their longest content without manual intervention.
- [ ] Column width computation respects font size changes across Comfortable, Compact, and UltraCompact densities.
- [ ] A minimum safety floor width (~36–48px) is enforced so no column can shrink below its title and interactive affordances.
- [ ] The `MESSAGE` column continues to expand to fill remaining horizontal space and unconstrained text length.
- [ ] Loading large log files (100k+ entries) exhibits zero measurable UI lag during width calculation.

## Blocked by

- .scratch/column-pinning/issues/01-prefactor-deprecate-word-wrap-and-anchor-gutter.md
