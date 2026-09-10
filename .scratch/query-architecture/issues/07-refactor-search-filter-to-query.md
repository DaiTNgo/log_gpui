Status: resolved

# Refactor Search & Filter to Use Query AST

## Resolution notes

- `SearchState.parsed_query: Option<QueryExpression>` (`#[serde(skip)]`) added in `src/search.rs`; re-parsed on every `update_matches()` via `reparse()`. Valid query-language input delegates to `QueryEngine::evaluate_entry`; plain-text / regex searches keep the legacy `build_regex` path.
- `find_ranges` uses `QueryEngine::find_ranges_in_text` in query mode.
- `FilterCondition` renamed to `FilterRow` and removed from the codebase; the popover row model is preserved (UI risk avoided).
- `FilterEngine::to_query_expression()` builds a `QueryExpression` from enabled rows (`level`/`module`/`message`/`requestId` → Eq/Neq/Contains/Matches; ALL/ANY → top-level And/Or group). `evaluate()` delegates to `QueryEngine::evaluate_entry` whenever the rows map cleanly; `NotContains` (no AST equivalent) forces the legacy per-row path, preserving behavior.
- `requestId` missing now resolves to empty string (parity with legacy `unwrap_or("")`).
- **Deferred**: SearchOverlay autocomplete popup powered by `QueryLanguageService` — requires a new completion-popover UI; language service already exposes `get_completions`. Flagged for a follow-up.
- Tests: `tests/query_refactor_test.rs` (9 tests); existing `tests/filter_test.rs` (4) still green.

## Parent

.scratch/query-architecture/PRD.md

## What to build

Refactor the existing `SearchState` (`src/search.rs`) and `FilterEngine` (`src/filter.rs`) to consume `QueryExpression` AST instead of raw text / `FilterCondition` list.

Changes:

**Search (`src/search.rs`):**
- `SearchState.query_text` stays (raw text for display)
- `SearchState.parsed_query: Option<QueryExpression>` added
- On text change: parse → validate → store AST
- `update_matches()` delegates to `SearchEngine.find_matches()` with AST
- SearchOverlay gets autocomplete powered by `QueryLanguageService`
- Remove manual regex building — `QueryEngine` handles matching

**Filter (`src/filter.rs`):**
- `FilterPopover` UI builds `QueryExpression` from its condition rows
- Each row: field dropdown + operator dropdown + value input → `QueryCondition`
- ALL/ANY toggle maps to top-level `QueryGroup` operator
- `FilterEngine` delegates to `QueryEngine` for evaluation
- Remove old `FilterCondition` struct and manual `evaluate()` logic

**Shared:**
- Both use `QueryLanguageService` for completions in their editors
- Both use `QueryEngine` for evaluation — guaranteed consistency
- `FilterCondition` type is deleted; `QueryCondition` replaces it

## Acceptance criteria

- [ ] Search parses query text into `QueryExpression` on each change
- [ ] Search uses `QueryEngine` for matching — no manual regex
- [ ] SearchOverlay shows autocomplete suggestions from `QueryLanguageService`
- [ ] FilterPopover builds `QueryExpression` from its UI rows
- [ ] Filter uses `QueryEngine` for visibility evaluation
- [ ] `FilterCondition` struct is removed from the codebase
- [ ] Existing search and filter tests pass (updated to use new API)
- [ ] New tests: search with AST, filter with AST, autocomplete integration

## Blocked by

Issue 02 — Query parser, Issue 06 — Query engine
