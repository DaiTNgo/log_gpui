# 0004. Search Is Text-Only

_Partly superseded by ADR-0008: Search and Filter now narrow the Focus Pane, and the view always shows every line._

## Context

The Search box used the same field-aware query language as Filter (`level == "error" && module contains "db"`), which read like SQL. Users had to learn a syntax just to find a word, and the line between Search and Filter was blurry.

## Decision

**Search** only matches text on the whole raw line. A Search Expression is made of Phrases combined with `&&`, `||` and parentheses. Words typed next to each other form one Phrase. There is no regex, no negation, no match-case option, and `and`/`or` are ordinary words, not operators. Matching is case-insensitive per Unicode and diacritic-sensitive (`loi` does not match `lỗi`); composed and decomposed forms of a character match each other. Search never changes which lines are shown, and it only counts lines that Filter currently shows.

Filtering by column is a separate feature, see ADR-0005.

Search has its own parser and matcher. It compiles the expression once and reuses the matcher for both matching and highlight ranges.

## Considered Options

- **Keep field queries in Search** and hide the complexity: rejected, the learning cost stays and Search and Filter stay redundant.
- **Add regex and negation to Search**: rejected to keep one simple mental model. Hiding noisy lines is done with Filter's Hide Mode.
- **Diacritic-insensitive matching** (`loi` matches `lỗi`): deferred. It adds noise and can be added later without changing Search syntax.
- **Reuse the old query AST with a pseudo-field for the raw line**: rejected, "any whitespace run satisfies a space in a Phrase" cannot be expressed by a plain `Contains` operator.

## Consequences

- Users lose field queries in the Search box.
- The field-query parser, validator, `language_service` and `completion_provider` are removed.
- Search cannot exclude lines; that is Filter's job.
