Status: resolved

# Query Text → AST Parser

## Parent

.scratch/query-architecture/PRD.md

## What to build

A tokenizer + recursive-descent parser in `src/query/parser/` that converts query text into a `QueryExpression` AST.

Query syntax:

```
level == ERROR
module == Payment
message contains "timeout"
level == ERROR && module == Payment
level == ERROR || level == WARN
level == ERROR && (module == Payment || module == Auth)
```

Grammar (informal):

```
query       → group
group       → atom ( (AND|OR) atom )*
atom        → condition | "(" group ")"
condition   → FIELD OPERATOR value
value       → STRING | NUMBER | BOOLEAN | enum_ref
OPERATOR    → == | != | contains | startsWith | endsWith | matches | in
AND         → && | AND
OR          → || | OR
```

Module structure:

```
src/query/parser/
├── mod.rs          — ParseResult { expression, errors }
├── tokenize.rs     — Token enum + Tokenizer iterator
├── parse.rs        — recursive-descent parser
└── syntax_errors.rs — ParseError types with span info
```

Key behaviors:
- `parse("level == ERROR")` → single condition
- `parse("a == 1 && b == 2")` → group with `And` operator
- `parse("a == 1 || (b == 2 && c == 3)")` → group with `Or`, nested group
- Syntax errors include byte offset spans for diagnostics
- Empty input returns an error, not an empty group

## Acceptance criteria

- [ ] Tokenizer correctly splits: identifiers, operators (`==`, `!=`, `contains`, etc.), string literals (`"..."`), numbers, booleans, parens, logical operators (`&&`, `||`)
- [ ] Parser handles operator precedence: `&&` binds tighter than `||`
- [ ] Parenthesized groups create nested `QueryGroup` nodes
- [ ] Syntax errors include byte offset span and descriptive message
- [ ] Round-trip test: parse → AST → (future) serialize produces equivalent structure
- [ ] Unit tests covering: single condition, AND group, OR group, mixed precedence, nested parens, string values with spaces, empty input error, unclosed paren error

## Blocked by

Issue 01 — Query AST types
