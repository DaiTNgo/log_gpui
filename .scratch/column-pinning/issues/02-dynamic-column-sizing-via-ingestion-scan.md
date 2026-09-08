Status: resolved

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

- [x] Parsing or loading a log stream tracks maximum character length per field in a single $O(1)$ ingestion pass.
- [x] Column widths for `TIMESTAMP`, `LEVEL`, and `MODULE` expand to accommodate their longest content without manual intervention.
- [x] Column width computation respects font size changes across Comfortable, Compact, and UltraCompact densities.
- [x] A minimum safety floor width (~36–48px) is enforced so no column can shrink below its title and interactive affordances.
- [x] The `MESSAGE` column continues to expand to fill remaining horizontal space and unconstrained text length.
- [x] Loading large log files (100k+ entries) exhibits zero measurable UI lag during width calculation.

## Blocked by

- .scratch/column-pinning/issues/01-prefactor-deprecate-word-wrap-and-anchor-gutter.md

## Comments

Resolved in Issue 02 implementation:
1. Implemented `FieldMaxChars` to track maximum field lengths in a single O(1) pass per entry.
2. Implemented `compute_column_width` using ADR 0003 formula: `max(min_width, (max_chars * W_char) + padding)`.
3. Enforced safety floor `min_width` (36px for LEVEL, 48px for TIMESTAMP and MODULE) so columns cannot collapse into unclickable states.
4. Wired automatic dynamic width recalculation on log ingestion (`update_from_entries`) and density changes (`update_density`).
5. Unit tested in `tests/column_test.rs`, including 100k entries performance benchmark (<50ms).
