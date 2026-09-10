Status: ready-for-agent

# PRD: Query Architecture — AST, Language Service, and Schema-Driven Evaluation

## Problem Statement

The current search and filter systems are hard-coded to a fixed set of fields (`level`, `module`, `message`, `request_id`) and use separate, incompatible evaluation paths. Search uses raw regex; filter uses a simple condition list with AND/OR logic. Neither can support:

- Dynamic fields discovered per log file (JSON logs have arbitrary keys)
- Nested/grouped conditions (`level == ERROR && (module == "Payment" || module == "Auth")`)
- IDE-like query editing experience (autocomplete, hover docs, validation)
- A shared evaluation engine that both search and filter delegate to

Without a unified query language, adding new field types or condition combinations requires duplicating logic across search, filter, and any future query consumer (AI, saved views, cross-log correlation).

## Solution

Introduce a **Query Domain** as the core abstraction layer:

1. **Query AST** — A simple, serializable AST (`QueryExpression = QueryCondition | QueryGroup`) representing structured queries.
2. **Query Parser** — Tokenizer + recursive-descent parser converting query text into AST.
3. **Query Validator** — Schema-aware validation producing diagnostics (unknown field, invalid operator, invalid enum value).
4. **Query Language Service** — Completions and hover information powered by schema + cursor position, providing an LSP-like editing experience.
5. **Schema-Aware Parser** — Log parser outputs a `LogSchema` per file, enabling dynamic field discovery.
6. **Query Engine** — Shared evaluation core used by both Search and Filter, plus specialized `SearchEngine` (match positions) and `FilterEngine` (visible event IDs).
7. **Refactored Search & Filter** — Both consume `QueryExpression` AST and delegate to `QueryEngine`.

## Architecture Boundary

```
                 Query Language
                       │
                  Query AST
                       │
              ┌────────┴────────┐
              │                 │
           SEARCH             FILTER
              │                 │
        find matches       hide events
              │                 │
              ▼                 ▼
          highlight         visible set
```

Search and Filter never call each other. Both depend only on `domains/query`.

## User Stories

1. As a developer, I want to type `level == ERROR` in the search bar and see autocomplete suggestions for field names, so I don't have to memorize the schema.
2. As a developer, I want to type `level == ` and see the valid enum values (`ERROR`, `WARN`, etc.) as suggestions, so I can build correct queries quickly.
3. As a developer, I want invalid queries to show inline diagnostics (red underline), so I know immediately what's wrong.
4. As a developer working with JSON logs, I want the parser to discover field names automatically, so search/filter works with any JSON structure without hard-coding.
5. As a developer, I want to write `level == ERROR && (module == "Payment" || module == "Auth")`, so I can express complex investigation queries.
6. As a developer, I want search and filter to share the same query engine, so results are consistent and I only learn one query syntax.
7. As a developer, I want each log file to have its own schema, so autocomplete and validation are always contextually correct.

## Implementation Decisions

- AST is kept minimal: only `QueryCondition` (field + operator + value) and `QueryGroup` (operator + children). No nesting beyond groups.
- Operators are type-dependent (string: eq/neq/contains/startsWith/endsWith/matches; enum: eq/neq/in). No comparison operators (`>`, `<`) — this is a search tool, not an analytics database.
- `LogSchema` is produced by the parser at ingestion time, not hard-coded.
- `QueryExpression` is serializable — enables saved views, AI-generated queries, and cross-log correlation later.
- Language service is an abstraction, not a full LSP implementation.

## Dependencies

- Existing parser (`src/parser/`) for format detection patterns
- Existing search (`src/search.rs`) and filter (`src/filter.rs`) for current behavior baseline
- Existing model (`src/model.rs`) for `LogEntry` / `LogLevel`
