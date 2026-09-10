Status: resolved

# Query Language Service — Completions + Hover

## Parent

.scratch/query-architecture/PRD.md

## What to build

A `QueryLanguageService` that provides IDE-like assistance for the query editor, powered by the current `LogSchema` and cursor position.

Interface:

```rust
struct CompletionContext {
    query_text: String,
    cursor_offset: usize,
    schema: LogSchema,
}

struct CompletionItem {
    label: String,
    kind: CompletionKind, // Field, Operator, Value, LogicalOp
    detail: Option<String>,
    insert_text: String,
}

struct HoverContext {
    query_text: String,
    cursor_offset: usize,
    schema: LogSchema,
}

struct HoverResult {
    content: String,
    range: Range<usize>,
}
```

Completion behavior by context:

| Context | Cursor at | Suggestions |
|---|---|---|
| Start of query | `lev▌` | Field names from schema |
| After field | `level ▌` | Operators valid for that field type |
| After operator | `level == ▌` | Enum values (for enum fields), or placeholder |
| After condition | `level == ERROR ▌` | `&&`, `||` |
| After logical op | `level == ERROR && ▌` | Field names |
| Inside parens | `level == ERROR && (▌` | Field names |

Hover behavior:

- Hover over a field name → show field type and (for enums) valid values
- Hover over an operator → show description ("equals", "contains substring", etc.)
- Hover over an enum value → confirm validity

```
src/query/
├── language_service.rs — QueryLanguageService struct + methods
```

## Acceptance criteria

- [ ] Completions correctly identify context (field/operator/value/logical) based on cursor position and surrounding tokens
- [ ] Field completions show all schema fields
- [ ] Operator completions filter by field type (only valid operators shown)
- [ ] Value completions show enum values for enum fields
- [ ] Logical operator completions (`&&`, `||`) appear after a complete condition
- [ ] Hover over field name shows type info
- [ ] Hover over operator shows human-readable description
- [ ] Empty/invalid query text still produces field completions at offset 0
- [ ] Unit tests for each completion context + hover scenarios

## Blocked by

Issue 01 — Query AST types, Issue 03 — Query validator (for context detection reuse)
