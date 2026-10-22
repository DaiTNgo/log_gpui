//! Search Expression: text-only search over a whole raw line.
//!
//! Grammar:
//! ```text
//! expr   := and ( "||" and )*
//! and    := atom ( "&&" atom )*
//! atom   := phrase | "(" expr ")"
//! phrase := word+            (words typed next to each other)
//! ```
//! A word is a run of characters without whitespace or `& | ( ) "`. Text in
//! double quotes is split on whitespace into words and may contain any of the
//! special characters. `and` / `or` are ordinary words.

use crate::unicode_text::Needle;
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    UnexpectedToken { expected: String, found: String, span: Range<usize> },
    UnclosedParen { span: Range<usize> },
    UnclosedQuote { span: Range<usize> },
    MissingOperand { span: Range<usize> },
    EmptyPhrase { span: Range<usize> },
}

impl SearchError {
    pub fn message(&self) -> String {
        match self {
            SearchError::UnexpectedToken { expected, found, .. } => {
                format!("expected {}, found '{}'", expected, found)
            }
            SearchError::UnclosedParen { .. } => "unclosed parenthesis".to_string(),
            SearchError::UnclosedQuote { .. } => "unclosed quote".to_string(),
            SearchError::MissingOperand { .. } => "missing term".to_string(),
            SearchError::EmptyPhrase { .. } => "empty quotes".to_string(),
        }
    }

    pub fn span(&self) -> Range<usize> {
        match self {
            SearchError::UnexpectedToken { span, .. }
            | SearchError::UnclosedParen { span }
            | SearchError::UnclosedQuote { span }
            | SearchError::MissingOperand { span }
            | SearchError::EmptyPhrase { span } => span.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchExpr {
    Phrase(Vec<String>),
    And(Vec<SearchExpr>),
    Or(Vec<SearchExpr>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String),
    And,
    Or,
    LParen,
    RParen,
}

#[derive(Debug, Clone)]
struct Token {
    tok: Tok,
    span: Range<usize>,
}

fn is_special(c: char) -> bool {
    matches!(c, '&' | '|' | '(' | ')' | '"')
}

fn tokenize(input: &str) -> Result<Vec<Token>, SearchError> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some(&(i, c)) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c == '(' || c == ')' {
            chars.next();
            let tok = if c == '(' { Tok::LParen } else { Tok::RParen };
            tokens.push(Token { tok, span: i..i + 1 });
        } else if c == '&' || c == '|' {
            chars.next();
            if chars.peek().map(|&(_, n)| n) == Some(c) {
                chars.next();
                let tok = if c == '&' { Tok::And } else { Tok::Or };
                tokens.push(Token { tok, span: i..i + 2 });
            } else {
                return Err(SearchError::UnexpectedToken {
                    expected: if c == '&' { "&&" } else { "||" }.to_string(),
                    found: c.to_string(),
                    span: i..i + 1,
                });
            }
        } else if c == '"' {
            chars.next();
            let start = i;
            let body_start = i + 1;
            let mut closed = None;
            for (j, n) in chars.by_ref() {
                if n == '"' {
                    closed = Some(j);
                    break;
                }
            }
            let Some(close) = closed else {
                return Err(SearchError::UnclosedQuote { span: start..input.len() });
            };
            let words: Vec<&str> = input[body_start..close].split_whitespace().collect();
            if words.is_empty() {
                return Err(SearchError::EmptyPhrase { span: start..close + 1 });
            }
            for w in words {
                tokens.push(Token { tok: Tok::Word(w.to_string()), span: start..close + 1 });
            }
        } else {
            let start = i;
            let mut end = i;
            while let Some(&(j, n)) = chars.peek() {
                if n.is_whitespace() || is_special(n) {
                    break;
                }
                end = j + n.len_utf8();
                chars.next();
            }
            tokens.push(Token { tok: Tok::Word(input[start..end].to_string()), span: start..end });
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    len: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.pos)
    }

    fn end_span(&self) -> Range<usize> {
        self.len..self.len
    }

    fn describe(tok: &Tok) -> String {
        match tok {
            Tok::Word(w) => w.clone(),
            Tok::And => "&&".to_string(),
            Tok::Or => "||".to_string(),
            Tok::LParen => "(".to_string(),
            Tok::RParen => ")".to_string(),
        }
    }

    fn parse_or(&mut self) -> Result<SearchExpr, SearchError> {
        let mut parts = vec![self.parse_and()?];
        while matches!(self.peek().map(|t| &t.tok), Some(Tok::Or)) {
            self.pos += 1;
            parts.push(self.parse_and()?);
        }
        Ok(if parts.len() == 1 { parts.remove(0) } else { SearchExpr::Or(parts) })
    }

    fn parse_and(&mut self) -> Result<SearchExpr, SearchError> {
        let mut parts = vec![self.parse_atom()?];
        loop {
            match self.peek().map(|t| &t.tok) {
                Some(Tok::And) => {
                    self.pos += 1;
                    parts.push(self.parse_atom()?);
                }
                Some(Tok::Or) | Some(Tok::RParen) | None => break,
                Some(_) => {
                    let t = self.peek().unwrap();
                    return Err(SearchError::UnexpectedToken {
                        expected: "&& or ||".to_string(),
                        found: Self::describe(&t.tok),
                        span: t.span.clone(),
                    });
                }
            }
        }
        Ok(if parts.len() == 1 { parts.remove(0) } else { SearchExpr::And(parts) })
    }

    fn parse_atom(&mut self) -> Result<SearchExpr, SearchError> {
        let Some(t) = self.peek() else {
            return Err(SearchError::MissingOperand { span: self.end_span() });
        };
        match &t.tok {
            Tok::LParen => {
                let open = t.span.clone();
                self.pos += 1;
                let inner = self.parse_or()?;
                match self.peek().map(|t| &t.tok) {
                    Some(Tok::RParen) => {
                        self.pos += 1;
                        Ok(inner)
                    }
                    Some(_) => {
                        let t = self.peek().unwrap();
                        Err(SearchError::UnexpectedToken {
                            expected: ")".to_string(),
                            found: Self::describe(&t.tok),
                            span: t.span.clone(),
                        })
                    }
                    None => Err(SearchError::UnclosedParen { span: open }),
                }
            }
            Tok::Word(_) => {
                let mut words = Vec::new();
                while let Some(Token { tok: Tok::Word(w), .. }) = self.peek() {
                    words.push(w.clone());
                    self.pos += 1;
                }
                Ok(SearchExpr::Phrase(words))
            }
            _ => Err(SearchError::MissingOperand { span: t.span.clone() }),
        }
    }
}

/// Parse a Search Expression. Empty or whitespace-only input is `Ok(None)`.
pub fn parse(input: &str) -> Result<Option<SearchExpr>, SearchError> {
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Ok(None);
    }
    let mut parser = Parser { tokens: &tokens, pos: 0, len: input.len() };
    let expr = parser.parse_or()?;
    if let Some(t) = parser.peek() {
        return Err(match t.tok {
            Tok::RParen => SearchError::UnexpectedToken {
                expected: "end of input".to_string(),
                found: ")".to_string(),
                span: t.span.clone(),
            },
            _ => SearchError::UnexpectedToken {
                expected: "end of input".to_string(),
                found: Parser::describe(&t.tok),
                span: t.span.clone(),
            },
        });
    }
    Ok(Some(expr))
}

#[derive(Debug, Clone)]
enum Node {
    /// A Phrase and its index among the distinct Phrases of the expression.
    Phrase(usize, Vec<Needle>),
    And(Vec<Node>),
    Or(Vec<Node>),
}

/// A Search Expression compiled once and run on many lines.
#[derive(Debug, Clone)]
pub struct Matcher {
    root: Node,
    /// The distinct Phrases, case-folded, in order of first appearance. Each
    /// is painted in a color of its own, generated from this key.
    keys: Vec<String>,
}

/// A byte range of a line that holds an occurrence of Phrase `phrase` (an
/// index into `Matcher::phrase_keys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhraseRange {
    pub start: usize,
    pub end: usize,
    pub phrase: usize,
}

impl Matcher {
    pub fn compile(expr: &SearchExpr) -> Self {
        fn build(expr: &SearchExpr, keys: &mut Vec<String>) -> Node {
            match expr {
                SearchExpr::Phrase(words) => {
                    let key = crate::unicode_text::fold(&words.join(" "));
                    let id = keys.iter().position(|k| *k == key).unwrap_or_else(|| {
                        keys.push(key);
                        keys.len() - 1
                    });
                    Node::Phrase(id, words.iter().map(|w| Needle::new(w)).collect())
                }
                SearchExpr::And(parts) => Node::And(parts.iter().map(|p| build(p, keys)).collect()),
                SearchExpr::Or(parts) => Node::Or(parts.iter().map(|p| build(p, keys)).collect()),
            }
        }
        let mut keys = Vec::new();
        let root = build(expr, &mut keys);
        Self { root, keys }
    }

    /// The distinct Phrases, case-folded, in order of first appearance.
    /// `||`, `&&` and parentheses do not matter: `a && b || c` has three. A
    /// Phrase's color is generated from its key.
    pub fn phrase_keys(&self) -> Vec<&str> {
        self.keys.iter().map(String::as_str).collect()
    }

    /// Whether `line` satisfies the expression.
    pub fn is_match(&self, line: &str) -> bool {
        eval(&self.root, line)
    }

    /// Indices (into `phrase_keys`) of the Phrases that occur in `line`, in
    /// expression order, whether or not a longer Phrase covers them.
    pub fn phrases_in(&self, line: &str) -> Vec<usize> {
        let mut found = Vec::new();
        collect(&self.root, line, &mut found);
        let mut phrases: Vec<usize> = found.into_iter().map(|(_, _, p)| p).collect();
        phrases.sort_unstable();
        phrases.dedup();
        phrases
    }

    /// Byte ranges of every Phrase occurrence in `line`, each tagged with its
    /// Phrase: sorted and not overlapping. Where occurrences of different
    /// Phrases overlap the longer Phrase owns the shared bytes (a tie goes to
    /// the earlier Phrase). The union of the ranges is `find_ranges`.
    pub fn find_phrase_ranges(&self, line: &str) -> Vec<PhraseRange> {
        let mut occurrences: Vec<(usize, usize, usize)> = Vec::new();
        collect(&self.root, line, &mut occurrences);
        let mut points: Vec<usize> = occurrences.iter().flat_map(|&(s, e, _)| [s, e]).collect();
        points.sort_unstable();
        points.dedup();
        let mut out: Vec<PhraseRange> = Vec::new();
        for w in points.windows(2) {
            let (start, end) = (w[0], w[1]);
            let owner = occurrences
                .iter()
                .filter(|&&(s, e, _)| s <= start && end <= e)
                .min_by_key(|&&(s, e, p)| (std::cmp::Reverse(e - s), p))
                .map(|&(_, _, p)| p);
            let Some(phrase) = owner else { continue };
            match out.last_mut() {
                Some(last) if last.phrase == phrase && last.end == start => last.end = end,
                _ => out.push(PhraseRange { start, end, phrase }),
            }
        }
        out
    }

    /// Byte ranges of every occurrence of every phrase in `line`, sorted and
    /// with overlaps merged.
    pub fn find_ranges(&self, line: &str) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        collect(&self.root, line, &mut found);
        let mut out: Vec<(usize, usize)> = found.into_iter().map(|(s, e, _)| (s, e)).collect();
        out.sort();
        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(out.len());
        for (s, e) in out {
            match merged.last_mut() {
                Some(last) if s <= last.1 => last.1 = last.1.max(e),
                _ => merged.push((s, e)),
            }
        }
        merged
    }
}

fn eval(node: &Node, line: &str) -> bool {
    match node {
        Node::Phrase(_, words) => !phrase_occurrences(words, line, true).is_empty(),
        Node::And(parts) => parts.iter().all(|p| eval(p, line)),
        Node::Or(parts) => parts.iter().any(|p| eval(p, line)),
    }
}

fn collect(node: &Node, line: &str, out: &mut Vec<(usize, usize, usize)>) {
    match node {
        Node::Phrase(id, words) => {
            out.extend(phrase_occurrences(words, line, false).into_iter().map(|(s, e)| (s, e, *id)))
        }
        Node::And(parts) | Node::Or(parts) => {
            for p in parts {
                collect(p, line, out);
            }
        }
    }
}

/// Occurrences of a phrase: words in order, with any run of whitespace between
/// two words. Stops at the first occurrence when `first_only`.
fn phrase_occurrences(words: &[Needle], line: &str, first_only: bool) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let Some(head) = words.first() else {
        return found;
    };
    let mut from = 0;
    while let Some((start, head_end)) = head.find_from(line, from) {
        match match_rest(&words[1..], line, head_end) {
            Some(end) => {
                found.push((start, end));
                if first_only {
                    return found;
                }
                from = end;
            }
            None => {
                // Retry one character after this start so overlapping heads count.
                from = start + line[start..].chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    found
}

fn match_rest(rest: &[Needle], line: &str, mut pos: usize) -> Option<usize> {
    for word in rest {
        let mut after = pos;
        for c in line[pos..].chars() {
            if c.is_whitespace() {
                after += c.len_utf8();
            } else {
                break;
            }
        }
        if after == pos {
            return None;
        }
        pos = word.match_at(line, after)?;
    }
    Some(pos)
}
