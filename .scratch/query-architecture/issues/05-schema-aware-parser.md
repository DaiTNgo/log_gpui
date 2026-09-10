Status: resolved

# Schema-Aware Parser — LogParser Outputs LogSchema Per File

## Resolution notes

- `LogEvent`/`LogValue` live in `src/parser/event.rs`; schema as `LogSchema` in `src/query/schema.rs`.
- `schema_detector.rs` folded into `src/parser/text.rs` as an internal `SchemaBuilder` (sampled over first 1000 events).
- JSON parser discovers all unique keys, infers types, and normalizes aliases (`service`→`module`, `message`/`msg`/`log`/`payload`/`text`, `level`/`severity`, `timestamp`/`time`/`@timestamp`).
- Enum detection: distinct string values ≤ 20 AND distinct < occurrences; canonical `level`/`module` are always Enum when pure strings.
- `LogEntry::from_event` adapter keeps the rendering model unchanged; `max_chars` still computed; legacy `ParseResult` path (`parse_text_bytes_with_max_chars`) preserved.
- Tests: `tests/parser_schema_test.rs` (8 tests), plus existing `tests/parser_test.rs` still green.

## Parent

.scratch/query-architecture/PRD.md

## What to build

Refactor `src/parser/` so that each format detector produces a `LogSchema` alongside the parsed entries. This enables dynamic field discovery — JSON logs with arbitrary keys get auto-discovered, text logs infer schema from their pattern.

Current state: parser returns `ParseResult { entries, max_chars }` with hard-coded fields on `LogEntry`.

Target state: parser returns `SchemaParseResult { schema: LogSchema, events: Vec<LogEvent>, max_chars }` where `LogEvent` has a dynamic `fields: HashMap<String, LogValue>`.

Key changes:

1. **JSON format detector**: Scan first N entries to discover all unique keys. Infer `FieldType` from value types. Enum fields detected when cardinality is low (<= 20 unique values from sampled entries).

2. **Text format detectors** (Standard, Logcat, Syslog): Infer schema from the regex capture groups — `timestamp` (Timestamp), `level` (Enum), `module` (Enum), `payload` (String), plus any JSON fields parsed from the payload.

3. **`LogEvent` domain model**: New struct with `id`, `source_id`, `line_start`, `line_end`, `timestamp`, `fields: HashMap<String, LogValue>`, `message`, `raw_line`.

4. **`LogEntry` stays**: The rendering model remains unchanged. A `From<LogEvent>` or adapter maps domain events to the existing `LogEntry` for the UI.

5. **`LogValue`**: Enum mirroring `QueryValue` — `String`, `Number`, `Bool`, `Null`. Used in both `LogEvent.fields` and query evaluation.

Module structure:

```
src/parser/
├── mod.rs             — SchemaParseResult, LogEvent, LogValue
├── text.rs            — (refactored) returns schema + events
├── schema_detector.rs — Schema inference logic per format
```

## Acceptance criteria

- [ ] JSON parser discovers all unique keys from first 100 entries and builds `LogSchema` with inferred types
- [ ] Text parsers produce schema from regex capture groups
- [ ] `LogEvent.fields` is a `HashMap<String, LogValue>` — no hard-coded field names
- [ ] Existing `LogEntry` rendering continues to work (adapter from `LogEvent`)
- [ ] `max_chars` computation still works for column sizing
- [ ] Unit tests: JSON schema detection, text schema inference, LogEvent → LogEntry conversion

## Blocked by

Issue 01 — Query AST types (for `LogSchema` / `FieldType` / `LogValue` definitions)
