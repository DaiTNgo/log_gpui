Status: resolved

# Query AST Types + LogSchema Foundation

## Parent

.scratch/query-architecture/PRD.md

## What to build

Introduce `src/query/` module containing the core data types that form the shared vocabulary for the entire query system. This is pure data definition — no parsing, no evaluation, no UI.

Types to define:

- `QueryExpression` — enum: `Condition(QueryCondition)` | `Group(QueryGroup)`
- `QueryCondition` — struct with `field: String`, `operator: QueryOperator`, `value: QueryValue`
- `QueryGroup` — struct with `operator: GroupOperator` (`And` | `Or`), `children: Vec<QueryExpression>`
- `QueryOperator` — enum: `Eq`, `Neq`, `Contains`, `StartsWith`, `EndsWith`, `Matches`, `In`
- `QueryValue` — enum: `String(String)`, `Number(f64)`, `Boolean(bool)`
- `GroupOperator` — enum: `And`, `Or`
- `FieldType` — enum: `String`, `Enum`, `Number`, `Boolean`, `Timestamp`, `Json`
- `QueryField` — struct with `name: String`, `field_type: FieldType`, `enum_values: Option<Vec<String>>`
- `LogSchema` — struct with `fields: Vec<QueryField>`
- `Diagnostic` — struct with `message: String`, `span: Range<usize>`, `severity: DiagnosticSeverity`

Also define `operators_for_type(field_type: FieldType) -> Vec<QueryOperator>` — returns the valid operators for a given field type.

Module structure:

```
src/query/
├── mod.rs          — re-exports
├── ast.rs          — QueryExpression, QueryCondition, QueryGroup, GroupOperator
├── operator.rs     — QueryOperator, operators_for_type
├── value.rs        — QueryValue
├── schema.rs       — LogSchema, QueryField, FieldType
└── diagnostic.rs   — Diagnostic, DiagnosticSeverity
```

## Acceptance criteria

- [ ] `src/query/` module compiles and is declared in `lib.rs`
- [ ] All types derive `Debug`, `Clone`, `PartialEq`
- [ ] `QueryExpression` has helper methods: `is_condition()`, `is_group()`, `as_condition()`, `as_group()`
- [ ] `operators_for_type` returns correct operator sets per `FieldType`
- [ ] Unit tests covering type construction and `operators_for_type` for each field type

## Blocked by

None - can start immediately
