#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    StringLit(String),
    Number(f64),
    True,
    False,
    Eq,        // ==
    Neq,       // !=
    Contains,
    StartsWith,
    EndsWith,
    Matches,
    In,
    And,       // && or AND
    Or,        // || or OR
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
}

impl TokenKind {
    pub fn display(&self) -> &str {
        match self {
            TokenKind::Ident(s) => s,
            TokenKind::StringLit(s) => s,
            TokenKind::Number(_) => "number",
            TokenKind::True => "true",
            TokenKind::False => "false",
            TokenKind::Eq => "==",
            TokenKind::Neq => "!=",
            TokenKind::Contains => "contains",
            TokenKind::StartsWith => "startsWith",
            TokenKind::EndsWith => "endsWith",
            TokenKind::Matches => "matches",
            TokenKind::In => "in",
            TokenKind::And => "&&",
            TokenKind::Or => "||",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::LBracket => "[",
            TokenKind::RBracket => "]",
            TokenKind::Comma => ",",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: std::ops::Range<usize>,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, super::ParseError> {
    let mut tokens = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }

        let start = i;

        // Parentheses
        if bytes[i] == b'(' {
            tokens.push(Token { kind: TokenKind::LParen, span: start..i + 1 });
            i += 1;
            continue;
        }
        if bytes[i] == b')' {
            tokens.push(Token { kind: TokenKind::RParen, span: start..i + 1 });
            i += 1;
            continue;
        }

        // Brackets
        if bytes[i] == b'[' {
            tokens.push(Token { kind: TokenKind::LBracket, span: start..i + 1 });
            i += 1;
            continue;
        }
        if bytes[i] == b']' {
            tokens.push(Token { kind: TokenKind::RBracket, span: start..i + 1 });
            i += 1;
            continue;
        }

        // Comma
        if bytes[i] == b',' {
            tokens.push(Token { kind: TokenKind::Comma, span: start..i + 1 });
            i += 1;
            continue;
        }

        // Operators starting with =
        if bytes[i] == b'=' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                tokens.push(Token { kind: TokenKind::Eq, span: start..i + 2 });
                i += 2;
            } else {
                // Single = treated as == (common typo)
                tokens.push(Token { kind: TokenKind::Eq, span: start..i + 1 });
                i += 1;
            }
            continue;
        }

        // Operators starting with !
        if bytes[i] == b'!' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                tokens.push(Token { kind: TokenKind::Neq, span: start..i + 2 });
                i += 2;
            } else {
                return Err(super::ParseError::UnexpectedToken {
                    expected: "!=".to_string(),
                    found: "!".to_string(),
                    span: start..i + 1,
                });
            }
            continue;
        }

        // Operators starting with &
        if bytes[i] == b'&' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                tokens.push(Token { kind: TokenKind::And, span: start..i + 2 });
                i += 2;
            } else {
                return Err(super::ParseError::UnexpectedToken {
                    expected: "&&".to_string(),
                    found: "&".to_string(),
                    span: start..i + 1,
                });
            }
            continue;
        }

        // Operators starting with |
        if bytes[i] == b'|' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                tokens.push(Token { kind: TokenKind::Or, span: start..i + 2 });
                i += 2;
            } else {
                return Err(super::ParseError::UnexpectedToken {
                    expected: "||".to_string(),
                    found: "|".to_string(),
                    span: start..i + 1,
                });
            }
            continue;
        }

        // String literal
        if bytes[i] == b'"' {
            i += 1;
            let mut s = String::new();
            while i < bytes.len() && bytes[i] != b'"' {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    i += 1;
                    match bytes[i] {
                        b'n' => s.push('\n'),
                        b't' => s.push('\t'),
                        b'\\' => s.push('\\'),
                        b'"' => s.push('"'),
                        c => {
                            s.push('\\');
                            s.push(c as char);
                        }
                    }
                } else {
                    s.push(bytes[i] as char);
                }
                i += 1;
            }
            if i >= bytes.len() {
                return Err(super::ParseError::UnexpectedToken {
                    expected: "\"".to_string(),
                    found: "end of input".to_string(),
                    span: start..i,
                });
            }
            i += 1; // closing "
            tokens.push(Token { kind: TokenKind::StringLit(s), span: start..i });
            continue;
        }

        // Number (negative or positive)
        if bytes[i].is_ascii_digit() || (bytes[i] == b'-' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit()) {
            let neg = bytes[i] == b'-';
            if neg { i += 1; }
            let num_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            let num_str = &input[num_start..i];
            if let Ok(n) = num_str.parse::<f64>() {
                tokens.push(Token {
                    kind: TokenKind::Number(if neg { -n } else { n }),
                    span: start..i,
                });
            } else {
                return Err(super::ParseError::InvalidValue { span: start..i });
            }
            continue;
        }

        // Identifiers and keywords
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let ident_start = i;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &input[ident_start..i];
            let kind = match word {
                "contains" => TokenKind::Contains,
                "startsWith" => TokenKind::StartsWith,
                "endsWith" => TokenKind::EndsWith,
                "matches" => TokenKind::Matches,
                "in" => TokenKind::In,
                "AND" | "and" => TokenKind::And,
                "OR" | "or" => TokenKind::Or,
                "true" => TokenKind::True,
                "false" => TokenKind::False,
                _ => TokenKind::Ident(word.to_string()),
            };
            tokens.push(Token { kind, span: ident_start..i });
            continue;
        }

        // Unknown character
        return Err(super::ParseError::UnexpectedToken {
            expected: "token".to_string(),
            found: (bytes[i] as char).to_string(),
            span: start..i + 1,
        });
    }

    Ok(tokens)
}
