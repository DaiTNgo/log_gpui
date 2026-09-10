Status: resolved

# Schema-Aware Query Validation

## Parent

.scratch/query-architecture/PRD.md

## What to build

A validator that takes a parsed `QueryExpression` + `LogSchema` and returns `Vec<Diagnostic>` covering all semantic errors.

Validation rules:

1. **Unknown field** — field name not found in schema → error with span
2. **Invalid operator** — operator not in `operators_for_type(field.field_type)` → error with span
3. **Invalid enum value** — for `FieldType::Enum`, value not in `field.enum_values` → error with span
4. **Type mismatch** — e.g., numeric value compared against string field → error with span
5. **Group validation** — recurse into `QueryGroup.children`, collect all diagnostics

No warnings — only errors. A valid query produces an empty `Vec<Diagnostic>`.

```
src/query/
├── validator.rs    — validate(query: &QueryExpression, schema: &LogSchema) -> Vec<Diagnostic>
```

## Acceptance criteria

- [ ] `validate` returns empty vec for structurally and semantically valid queries
- [ ] Unknown field names produce a diagnostic with the correct byte span
- [ ] Invalid operators for a field type produce a diagnostic
- [ ] Invalid enum values produce a diagnostic listing valid alternatives
- [ ] Nested group conditions are recursively validated
- [ ] Multiple errors in one query are all reported (not fail-fast)
- [ ] Unit tests for each error category + a valid complex query

## Blocked by

Issue 01 — Query AST types
