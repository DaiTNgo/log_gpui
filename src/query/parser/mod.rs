mod tokenize;

pub use tokenize::{Token, TokenKind};

use crate::query::ast::{GroupOperator, QueryCondition, QueryExpression, QueryGroup};
use crate::query::operator::QueryOperator;
use crate::query::value::QueryValue;

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    EmptyInput,
    UnexpectedToken { expected: String, found: String, span: std::ops::Range<usize> },
    UnclosedParen { span: std::ops::Range<usize> },
    TrailingOperator { span: std::ops::Range<usize> },
    InvalidValue { span: std::ops::Range<usize> },
}

impl ParseError {
    pub fn message(&self) -> String {
        match self {
            ParseError::EmptyInput => "empty query".to_string(),
            ParseError::UnexpectedToken { expected, found, .. } => {
                format!("expected {}, found '{}'", expected, found)
            }
            ParseError::UnclosedParen { .. } => "unclosed parenthesis".to_string(),
            ParseError::TrailingOperator { .. } => "trailing operator".to_string(),
            ParseError::InvalidValue { .. } => "invalid value".to_string(),
        }
    }

    pub fn span(&self) -> Option<std::ops::Range<usize>> {
        match self {
            ParseError::UnexpectedToken { span, .. } => Some(span.clone()),
            ParseError::UnclosedParen { span } => Some(span.clone()),
            ParseError::TrailingOperator { span } => Some(span.clone()),
            ParseError::InvalidValue { span } => Some(span.clone()),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<QueryExpression, ParseError> {
    let tokens = tokenize::tokenize(input)?;
    if tokens.is_empty() {
        return Err(ParseError::EmptyInput);
    }
    let mut parser = Parser::new(&tokens);
    let expr = parser.parse_or()?;
    if parser.current < parser.tokens.len() {
        return Err(ParseError::UnexpectedToken {
            expected: "end of input".to_string(),
            found: parser.tokens[parser.current].kind.display().to_string(),
            span: parser.tokens[parser.current].span.clone(),
        });
    }
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [Token],
    current: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, current: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.current)
    }

    fn advance(&mut self) -> Option<&Token> {
        let tok = self.tokens.get(self.current);
        if tok.is_some() {
            self.current += 1;
        }
        tok
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        let span = self.peek().map(|t| t.span.clone()).unwrap_or(0..0);
        let tok = self.advance().ok_or_else(|| ParseError::UnexpectedToken {
            expected: kind.display().to_string(),
            found: "end of input".to_string(),
            span,
        })?;
        if tok.kind == kind {
            Ok(tok.clone())
        } else {
            Err(ParseError::UnexpectedToken {
                expected: kind.display().to_string(),
                found: tok.kind.display().to_string(),
                span: tok.span.clone(),
            })
        }
    }

    fn parse_or(&mut self) -> Result<QueryExpression, ParseError> {
        let mut left = self.parse_and()?;
        while self.peek().map_or(false, |t| t.kind == TokenKind::Or) {
            self.advance().unwrap();
            let right = self.parse_and()?;
            let span = span_of(&left, &right);
            let children = match left {
                QueryExpression::Group(ref g) if g.operator == GroupOperator::Or => {
                    let mut kids = g.children.clone();
                    kids.push(right);
                    kids
                }
                _ => vec![left, right],
            };
            left = QueryExpression::Group(QueryGroup {
                operator: GroupOperator::Or,
                children,
                span,
            });
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<QueryExpression, ParseError> {
        let mut left = self.parse_atom()?;
        while self.peek().map_or(false, |t| t.kind == TokenKind::And) {
            self.advance().unwrap();
            let right = self.parse_atom()?;
            let span = span_of(&left, &right);
            let children = match left {
                QueryExpression::Group(ref g) if g.operator == GroupOperator::And => {
                    let mut kids = g.children.clone();
                    kids.push(right);
                    kids
                }
                _ => vec![left, right],
            };
            left = QueryExpression::Group(QueryGroup {
                operator: GroupOperator::And,
                children,
                span,
            });
        }
        Ok(left)
    }

    fn parse_atom(&mut self) -> Result<QueryExpression, ParseError> {
        match self.peek().map(|t| t.kind.clone()) {
            Some(TokenKind::LParen) => {
                let open_tok = self.advance().unwrap();
                let open_span = open_tok.span.clone();
                let mut expr = self.parse_or()?;
                let close_tok = self.expect(TokenKind::RParen)?;
                // Extend the group span to cover the parens wrapping it
                if let QueryExpression::Group(g) = &mut expr {
                    g.span = open_span.start..close_tok.span.end;
                } else if let QueryExpression::Condition(c) = &mut expr {
                    c.field_span = open_span.start..c.field_span.end;
                    c.value_span = c.value_span.start..close_tok.span.end;
                    c.operator_span = c.operator_span.start..c.operator_span.end;
                }
                Ok(expr)
            }
            Some(TokenKind::Ident(_)) => self.parse_condition(),
            _ => Err(ParseError::UnexpectedToken {
                expected: "identifier or '('".to_string(),
                found: self.peek().map_or("end of input".to_string(), |t| t.kind.display().to_string()),
                span: self.peek().map(|t| t.span.clone()).unwrap_or(0..0),
            }),
        }
    }

    fn parse_condition(&mut self) -> Result<QueryExpression, ParseError> {
        let field_tok = self.advance().unwrap().clone();
        let field = match &field_tok.kind {
            TokenKind::Ident(s) => s.clone(),
            _ => unreachable!(),
        };
        let field_span = field_tok.span.clone();

        let op = self.parse_operator()?;
        let operator_span = self.tokens.get(self.current - 1).map(|t| t.span.clone()).unwrap_or(0..0);

        // Handle `in (val1, val2)` — special case: produce grouped Eq conditions
        if op == QueryOperator::In {
            let values = self.parse_value_list()?;
            if values.is_empty() {
                return Err(ParseError::InvalidValue { span: field_span });
            }
            let children: Vec<QueryExpression> = values
                .into_iter()
                .map(|v| {
                    QueryExpression::Condition(QueryCondition::new(&field, QueryOperator::Eq, v))
                })
                .collect();
            let mut g = crate::query::ast::QueryGroup::new(GroupOperator::Or, children);
            g.span = field_span.start..self.tokens[self.current - 1].span.end;
            return Ok(QueryExpression::Group(g));
        }

        let value = self.parse_value()?;
        let value_span = self.tokens.get(self.current - 1).map(|t| t.span.clone()).unwrap_or_else(|| field_span.clone());
        Ok(QueryExpression::Condition(QueryCondition {
            field,
            operator: op,
            value,
            field_span,
            operator_span,
            value_span,
        }))
    }

    fn parse_operator(&mut self) -> Result<QueryOperator, ParseError> {
        let span = self.peek().map(|t| t.span.clone()).unwrap_or(0..0);
        let tok = self.advance().ok_or_else(|| ParseError::UnexpectedToken {
            expected: "operator".to_string(),
            found: "end of input".to_string(),
            span,
        })?;
        match &tok.kind {
            TokenKind::Eq => Ok(QueryOperator::Eq),
            TokenKind::Neq => Ok(QueryOperator::Neq),
            TokenKind::Contains => Ok(QueryOperator::Contains),
            TokenKind::StartsWith => Ok(QueryOperator::StartsWith),
            TokenKind::EndsWith => Ok(QueryOperator::EndsWith),
            TokenKind::Matches => Ok(QueryOperator::Matches),
            TokenKind::In => Ok(QueryOperator::In),
            _ => Err(ParseError::UnexpectedToken {
                expected: "operator".to_string(),
                found: tok.kind.display().to_string(),
                span: tok.span.clone(),
            }),
        }
    }

    fn parse_value(&mut self) -> Result<QueryValue, ParseError> {
        let span = self.peek().map(|t| t.span.clone()).unwrap_or(0..0);
        let tok = self.advance().ok_or_else(|| ParseError::UnexpectedToken {
            expected: "value".to_string(),
            found: "end of input".to_string(),
            span,
        })?;
        match &tok.kind {
            TokenKind::StringLit(s) => Ok(QueryValue::String(s.clone())),
            TokenKind::Number(n) => Ok(QueryValue::Number(*n)),
            TokenKind::True => Ok(QueryValue::Boolean(true)),
            TokenKind::False => Ok(QueryValue::Boolean(false)),
            TokenKind::Ident(s) => Ok(QueryValue::String(s.clone())),
            _ => Err(ParseError::UnexpectedToken {
                expected: "value".to_string(),
                found: tok.kind.display().to_string(),
                span: tok.span.clone(),
            }),
        }
    }

    fn parse_value_list(&mut self) -> Result<Vec<QueryValue>, ParseError> {
        self.expect(TokenKind::LParen)?;
        let mut values = Vec::new();
        if self.peek().map_or(false, |t| t.kind == TokenKind::RParen) {
            self.advance();
            return Ok(values);
        }
        values.push(self.parse_value()?);
        while self.peek().map_or(false, |t| t.kind == TokenKind::Comma) {
            self.advance();
            values.push(self.parse_value()?);
        }
        self.expect(TokenKind::RParen)?;
        Ok(values)
    }
}

fn span_of(left: &QueryExpression, right: &QueryExpression) -> std::ops::Range<usize> {
    let start = expr_start(left);
    let end = expr_end(right);
    start..end
}

fn expr_start(expr: &QueryExpression) -> usize {
    match expr {
        QueryExpression::Condition(c) => c.field_span.start,
        QueryExpression::Group(g) => g.span.start,
    }
}

fn expr_end(expr: &QueryExpression) -> usize {
    match expr {
        QueryExpression::Condition(c) => c.value_span.end,
        QueryExpression::Group(g) => g.span.end,
    }
}
