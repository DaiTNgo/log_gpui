Status: resolved

# Shared Query Evaluation Engine

## Resolution notes

- `QueryEngine` in `src/query/engine.rs` evaluates via a shared `evaluate_with(&dyn Fn(&str) -> Option<LogValue>, expr)` path, exposed as `evaluate_event(&LogEvent, …)`, `evaluate_fields(&HashMap, …)`, and `evaluate_entry(&LogEntry, …)` bridging.
- Missing/null fields → false; `Contains`/`StartsWith`/`EndsWith` case-insensitive; `Eq` case-insensitive for strings, numeric for Numbers; `Matches` = regex; `In` is desugared by the parser into an OR-of-Eq group (defensive `false` in the engine). Cross-type `Eq` is a non-match — boolean fields must be queried with a boolean literal (`retry == true`), never a string.
- `SearchEngine::find_matches(events, query) -> Vec<SearchMatch { event_id, ranges }>` in `src/infrastructure/search_engine.rs`; ranges are case-insensitive byte offsets of string literals (Eq/Contains/StartsWith/EndsWith) or regex matches (`Matches`) over `raw_line`.
- `FilterEngine::get_visible(events, query) -> Vec<usize>` in `src/infrastructure/filter_engine.rs`.
- Modules wired via `src/infrastructure/mod.rs`; `pub mod infrastructure;` added to `lib.rs` and `main.rs`.
- Tests: `tests/query_engine_test.rs` (18 tests).

## Parent

.scratch/query-architecture/PRD.md

## What to build

The core evaluation engine that both Search and Filter delegate to. Three layers:

1. **`QueryEngine`** — evaluates a `QueryExpression` against a single `LogEvent`, returns `bool`. This is the foundation.

2. **`SearchEngine`** — wraps `QueryEngine`, iterates events, returns `Vec<SearchMatch>` with byte-offset ranges for highlighting.

3. **`FilterEngine`** — wraps `QueryEngine`, iterates events, returns `Vec<usize>` of visible event indices.

```
src/query/
├── engine.rs     — QueryEngine.evaluate(event, query) -> bool
src/infrastructure/
├── search_engine.rs — SearchEngine.find_matches(events, query) -> Vec<SearchMatch>
├── filter_engine.rs — FilterEngine.get_visible(events, query) -> Vec<usize>
```

`QueryEngine` evaluation rules:
- `Condition`: look up `field` in `event.fields`, apply operator against value
- `Group`: evaluate children with AND/OR logic
- Missing field → `false` (event doesn't match)
- `Contains`/`StartsWith`/`EndsWith` → case-insensitive string comparison
- `Matches` → regex match against string representation
- `In` → check if value is in the list

`SearchMatch`:

```rust
struct SearchMatch {
    event_id: usize,
    ranges: Vec<(usize, usize)>, // byte offset ranges to highlight
}
```

`SearchEngine` also builds highlight ranges by scanning the matched field's string value.

## Acceptance criteria

- [ ] `QueryEngine.evaluate` handles all operator types correctly
- [ ] `QueryEngine.evaluate` handles nested groups
- [ ] `QueryEngine.evaluate` returns false for missing fields
- [ ] `SearchEngine.find_matches` returns correct byte-offset ranges for contains/eq matches
- [ ] `FilterEngine.get_visible` returns correct visible indices
- [ ] Both engines share the same `QueryEngine` — no duplicate evaluation logic
- [ ] Unit tests: engine evaluation for each operator, nested groups, missing fields; search match ranges; filter visible set

## Blocked by

Issue 01 — Query AST types, Issue 05 — Schema-aware parser (for `LogEvent` / `LogValue`)
