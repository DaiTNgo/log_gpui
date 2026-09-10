use crate::query::operator::operators_for_type;
use crate::query::schema::LogSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Field,
    Operator,
    Value,
    LogicalOp,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionItem {
    pub label: String,
    pub kind: CompletionKind,
    pub detail: Option<String>,
    pub insert_text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HoverResult {
    pub content: String,
    pub range: std::ops::Range<usize>,
}

#[derive(Debug, Clone, PartialEq)]
enum LsTokenKind {
    Ident(String),
    StringLit(String),
    Number(f64),
    True,
    False,
    Eq,
    Neq,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
    In,
    And,
    Or,
    LParen,
    RParen,
    Comma,
}

#[derive(Debug, Clone)]
struct LsToken {
    kind: LsTokenKind,
    span: std::ops::Range<usize>,
}

impl LsToken {
    fn is_operator(&self) -> bool {
        matches!(
            self.kind,
            LsTokenKind::Eq
                | LsTokenKind::Neq
                | LsTokenKind::Contains
                | LsTokenKind::StartsWith
                | LsTokenKind::EndsWith
                | LsTokenKind::Matches
                | LsTokenKind::In
        )
    }

    fn is_value(&self) -> bool {
        matches!(
            self.kind,
            LsTokenKind::StringLit(_)
                | LsTokenKind::Number(_)
                | LsTokenKind::True
                | LsTokenKind::False
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
enum CompletionContext {
    Field(String),
    Operator(String),
    Value { field: String, prefix: String },
    LogicalOp,
}

/// Tolerant tokenizer for partial query text (used by the language service).
fn tolerant_tokenize(text: &str) -> Vec<LsToken> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        let start = i;

        match bytes[i] {
            b'(' => {
                tokens.push(LsToken { kind: LsTokenKind::LParen, span: start..i + 1 });
                i += 1;
            }
            b')' => {
                tokens.push(LsToken { kind: LsTokenKind::RParen, span: start..i + 1 });
                i += 1;
            }
            b',' => {
                tokens.push(LsToken { kind: LsTokenKind::Comma, span: start..i + 1 });
                i += 1;
            }
            b'=' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(LsToken { kind: LsTokenKind::Eq, span: start..i + 2 });
                    i += 2;
                } else {
                    // Partial `=` — skip (user is typing)
                    i += 1;
                }
            }
            b'!' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    tokens.push(LsToken { kind: LsTokenKind::Neq, span: start..i + 2 });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    tokens.push(LsToken { kind: LsTokenKind::And, span: start..i + 2 });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    tokens.push(LsToken { kind: LsTokenKind::Or, span: start..i + 2 });
                    i += 2;
                } else {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                let mut s = String::new();
                let mut closed = false;
                while i < bytes.len() && bytes[i] != b'"' {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 1;
                        s.push(bytes[i] as char);
                    } else {
                        s.push(bytes[i] as char);
                    }
                    i += 1;
                }
                if i < bytes.len() {
                    i += 1; // closing quote
                    closed = true;
                }
                // Only emit if string is closed; otherwise leave as partial tail
                if closed {
                    tokens.push(LsToken { kind: LsTokenKind::StringLit(s), span: start..i });
                } else {
                    break;
                }
            }
            b'-' | b'0'..=b'9' => {
                let neg = bytes[i] == b'-';
                let num_start = i;
                if neg { i += 1; }
                let is_valid_start = i < bytes.len() && bytes[i].is_ascii_digit();
                if !is_valid_start {
                    i += 1;
                    continue;
                }
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                if let Ok(n) = text[num_start..i].parse::<f64>() {
                    tokens.push(LsToken { kind: LsTokenKind::Number(n), span: num_start..i });
                }
            }
            _ if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' => {
                let ident_start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                let word = &text[ident_start..i];
                let kind = match word {
                    "contains" => LsTokenKind::Contains,
                    "startsWith" => LsTokenKind::StartsWith,
                    "endsWith" => LsTokenKind::EndsWith,
                    "matches" => LsTokenKind::Matches,
                    "in" => LsTokenKind::In,
                    "AND" | "and" => LsTokenKind::And,
                    "OR" | "or" => LsTokenKind::Or,
                    "true" => LsTokenKind::True,
                    "false" => LsTokenKind::False,
                    _ => LsTokenKind::Ident(word.to_string()),
                };
                tokens.push(LsToken { kind, span: ident_start..i });
            }
            _ => {
                i += 1;
            }
        }
    }

    tokens
}

fn trailing_gap(text: &str, last_end: usize) -> &str {
    &text[last_end..]
}

fn is_token_continuation_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '"'
}

/// Determine completion context from partial query text up to the cursor.
fn completion_context(text: &str, cursor: usize) -> CompletionContext {
    let end = cursor.min(text.len());
    let prefix_text = &text[..end];
    let tokens = tolerant_tokenize(prefix_text);

    let last_end = tokens.last().map(|t| t.span.end).unwrap_or(0);
    let gap = trailing_gap(prefix_text, last_end);

    let last_char_is_continuation = end > 0
        && text[..end]
            .chars()
            .last()
            .is_some_and(|c| is_token_continuation_char(c));
    let has_partial = !gap.trim().is_empty() || last_char_is_continuation;

    // Compute the partial prefix the user is actively typing.
    let partial = if !gap.trim().is_empty() {
        gap.trim().to_string()
    } else if last_char_is_continuation
        && tokens.last().map(|t| t.span.end) == Some(end)
    {
        match &tokens.last().unwrap().kind {
            LsTokenKind::Ident(name) => name.clone(),
            LsTokenKind::Number(n) => n.to_string(),
            _ => String::new(),
        }
    } else {
        String::new()
    };

    if has_partial {
        match tokens.last() {
            Some(tok) if tok.is_operator() => CompletionContext::Value {
                field: field_before_operator(&tokens, tok).unwrap_or_default(),
                prefix: partial,
            },
            Some(tok) if matches!(tok.kind, LsTokenKind::And | LsTokenKind::Or | LsTokenKind::LParen) => {
                CompletionContext::Field(partial)
            }
            Some(tok) if tok.is_value() => CompletionContext::Value {
                field: String::new(),
                prefix: partial,
            },
            Some(tok) if matches!(tok.kind, LsTokenKind::Ident(_)) => {
                let prev_is_operator = tokens
                    .get(tokens.len().saturating_sub(2))
                    .map(|t| t.is_operator())
                    .unwrap_or(false);
                if prev_is_operator {
                    let field = tokens
                        .get(tokens.len().saturating_sub(2))
                        .and_then(|t| field_before_operator(&tokens, t))
                        .unwrap_or_default();
                    CompletionContext::Value { field, prefix: partial }
                } else {
                    CompletionContext::Field(partial)
                }
            }
            _ => CompletionContext::Field(partial),
        }
    } else {
        match tokens.last() {
            None => CompletionContext::Field(String::new()),
            Some(tok) if matches!(tok.kind, LsTokenKind::And | LsTokenKind::Or | LsTokenKind::LParen) => {
                CompletionContext::Field(String::new())
            }
            Some(tok) if tok.is_operator() => {
                let field = field_before_operator(&tokens, tok).unwrap_or_default();
                CompletionContext::Value { field, prefix: String::new() }
            }
            Some(tok) if tok.is_value() => CompletionContext::LogicalOp,
            Some(tok) if matches!(tok.kind, LsTokenKind::Ident(_)) => {
                let prev_is_operator = tokens
                    .get(tokens.len().saturating_sub(2))
                    .map(|t| t.is_operator())
                    .unwrap_or(false);
                if prev_is_operator {
                    CompletionContext::LogicalOp
                } else {
                    let field = match &tok.kind {
                        LsTokenKind::Ident(name) => name.clone(),
                        _ => String::new(),
                    };
                    CompletionContext::Operator(field)
                }
            }
            _ => CompletionContext::Field(String::new()),
        }
    }
}

fn field_before_operator(tokens: &[LsToken], operator_token: &LsToken) -> Option<String> {
    let idx = tokens.iter().position(|t| std::ptr::eq(t, operator_token))?;
    let prev = tokens.get(idx.wrapping_sub(1))?;
    match &prev.kind {
        LsTokenKind::Ident(name) => Some(name.clone()),
        _ => Some(String::new()),
    }
}

pub struct QueryLanguageService;

impl QueryLanguageService {
    pub fn get_completions(text: &str, cursor: usize, schema: &LogSchema) -> Vec<CompletionItem> {
        match completion_context(text, cursor) {
            CompletionContext::Field(prefix) => {
                schema
                    .fields
                    .iter()
                    .filter(|f| f.name.starts_with(&prefix))
                    .map(|f| CompletionItem {
                        label: f.name.clone(),
                        kind: CompletionKind::Field,
                        detail: Some(format!("{:?}", f.field_type)),
                        insert_text: f.name.clone(),
                    })
                    .collect()
            }
            CompletionContext::Operator(field) => {
                let Some(fdef) = schema.find_field(&field) else {
                    return Vec::new();
                };
                operators_for_type(fdef.field_type)
                    .into_iter()
                    .map(|op| CompletionItem {
                        label: op.as_str().to_string(),
                        kind: CompletionKind::Operator,
                        detail: Some(op.description().to_string()),
                        insert_text: format!("{} ", op.as_str()),
                    })
                    .collect()
            }
            CompletionContext::Value { field, prefix } => {
                let Some(fdef) = schema.find_field(&field) else {
                    return Vec::new();
                };
                match &fdef.enum_values {
                    Some(values) => values
                        .iter()
                        .filter(|v| v.starts_with(&prefix))
                        .map(|v| CompletionItem {
                            label: v.clone(),
                            kind: CompletionKind::Value,
                            detail: None,
                            insert_text: v.clone(),
                        })
                        .collect(),
                    None => Vec::new(),
                }
            }
            CompletionContext::LogicalOp => vec![
                CompletionItem {
                    label: "&&".to_string(),
                    kind: CompletionKind::LogicalOp,
                    detail: Some("and".to_string()),
                    insert_text: "&& ".to_string(),
                },
                CompletionItem {
                    label: "||".to_string(),
                    kind: CompletionKind::LogicalOp,
                    detail: Some("or".to_string()),
                    insert_text: "|| ".to_string(),
                },
            ],
        }
    }

    pub fn get_hover(text: &str, offset: usize, schema: &LogSchema) -> Option<HoverResult> {
        let tokens = tolerant_tokenize(text);
        let offset = offset.min(text.len());
        let token = tokens.iter().find(|t| t.span.contains(&offset))?;

        let span = token.span.clone();
        match &token.kind {
            LsTokenKind::Ident(name) => {
                if let Some(field) = schema.find_field(name) {
                    let mut content = format!("field **{}** — type `{:?}`", field.name, field.field_type);
                    if let Some(values) = &field.enum_values {
                        content.push_str(&format!("\n\nvalues: {}", values.join(", ")));
                    }
                    Some(HoverResult { content, range: span })
                } else {
                    // Could be a value position — check the preceding token
                    let idx = tokens.iter().position(|t| std::ptr::eq(t, token)).unwrap_or(0);
                    let prev = tokens.get(idx.wrapping_sub(1));
                    if let Some(prev_tok) = prev {
                        if prev_tok.is_operator() {
                            let field_name = field_before_operator(&tokens, prev_tok).unwrap_or_default();
                            if let Some(fdef) = schema.find_field(&field_name) {
                                if let Some(values) = &fdef.enum_values {
                                    let valid = values.iter().any(|v| v.eq_ignore_ascii_case(name));
                                    let status = if valid { "valid value" } else { "NOT in allowed values" };
                                    return Some(HoverResult {
                                        content: format!(
                                            "value `{}` — {}\n\nallowed: {}",
                                            name,
                                            status,
                                            values.join(", ")
                                        ),
                                        range: span,
                                    });
                                }
                            }
                        }
                    }
                    None
                }
            }
            LsTokenKind::StringLit(s) => hover_for_value(s, &tokens, token, schema),
            LsTokenKind::Number(n) => hover_for_value(&n.to_string(), &tokens, token, schema),
            LsTokenKind::Eq => Some(HoverResult {
                content: "operator **==** — equals".to_string(),
                range: span,
            }),
            LsTokenKind::Neq => Some(HoverResult {
                content: "operator **!=** — does not equal".to_string(),
                range: span,
            }),
            LsTokenKind::Contains => Some(HoverResult {
                content: "operator **contains** — contains substring".to_string(),
                range: span,
            }),
            LsTokenKind::StartsWith => Some(HoverResult {
                content: "operator **startsWith** — starts with".to_string(),
                range: span,
            }),
            LsTokenKind::EndsWith => Some(HoverResult {
                content: "operator **endsWith** — ends with".to_string(),
                range: span,
            }),
            LsTokenKind::Matches => Some(HoverResult {
                content: "operator **matches** — matches regex".to_string(),
                range: span,
            }),
            LsTokenKind::In => Some(HoverResult {
                content: "operator **in** — is one of".to_string(),
                range: span,
            }),
            _ => None,
        }
    }
}

fn hover_for_value(
    value: &str,
    tokens: &[LsToken],
    value_token: &LsToken,
    schema: &LogSchema,
) -> Option<HoverResult> {
    let idx = tokens.iter().position(|t| std::ptr::eq(t, value_token)).unwrap_or(0);
    let prev = tokens.get(idx.wrapping_sub(1))?;
    if !prev.is_operator() {
        return None;
    }
    let field_name = field_before_operator(tokens, prev).unwrap_or_default();
    let fdef = schema.find_field(&field_name)?;

    let content = match &fdef.enum_values {
        Some(values) => {
            let valid = values.iter().any(|v| v.eq_ignore_ascii_case(value));
            let status = if valid { "valid value" } else { "NOT in allowed values" };
            format!("value `{}` — {}\n\nallowed: {}", value, status, values.join(", "))
        }
        None => format!("value `{}` for field `{}` (type {:?})", value, field_name, fdef.field_type),
    };
    Some(HoverResult {
        content,
        range: value_token.span.clone(),
    })
}