Status: ready-for-agent

# Follow-ups from query-architecture code review

## Parent

.scratch/query-architecture/PRD.md

## What to build

Deferred findings from the post-implementation Standards/Spec review. Each item improves the query-architecture code without changing observable behavior.

1. **Unify `LogValue` / `QueryValue`**: `src/parser/event.rs` `LogValue` and `src/query/value.rs` `QueryValue` are structurally identical enums with identical accessors (`as_str`/`as_f64`/`as_bool`/`to_string_value`). Derive one from the other or merge.

2. **Canonical-field registry**: The literals `level`, `module`, `message`, `time`, `requestId` are hard-coded across `src/query/engine.rs` (`entry_value`), `src/filter.rs` (`to_query_condition`), `src/parser/text.rs` (JSON extraction + skip list), and `src/model.rs` (`LogEntry::from_event`). Adding a canonical field touches 4+ files (shotgun surgery). Introduce a single `CanonicalField` constants/registry module.

3. **Model `get_field` symmetry**: `LogEntry` and `LogEvent` should each expose `get_field(&str) -> Option<LogValue>` so `entry_value` in the engine and `from_event` in the model stop reaching into each other's fields by string key.

4. **Tolerant tokenizer reuse**: `src/query/language_service.rs` `tolerant_tokenize` re-implements `src/query/parser/tokenize.rs` with its own `LsTokenKind`. The tolerant variant is a subset; share the tokenizer / enum instead of duplicating the loop.

5. **`FilterEngine` name collision**: `src/infrastructure/filter_engine.rs` `FilterEngine` and `src/filter.rs` `FilterEngine` have the same name with different purposes. Rename the infrastructure wrapper (e.g. `QueryFilterEngine`) or fold it into the filter module.

6. **Dead bracket tokens / `in` syntax**: `TokenKind::LBracket`/`RBracket` in the tokenizer are consumed by nothing (the `in` list uses parens). Remove or wire up.

7. **Dead `evaluate_fields`**: `QueryEngine::evaluate_fields` is only exercised by a unit test. Either mark `#[cfg(test)]` or keep it documented as the cheap shared path.

8. **SearchOverlay autocomplete** (from issue 07): wire `QueryLanguageService::get_completions` into a completion popover — deferred from issue 07.

## Acceptance criteria

- [ ] No behavior change to parsing, validation, engine evaluation, or the filter/search UI
- [ ] All query-architecture tests stay green
- [ ] Each item either fixed, or explicitly descoped with a note here