use crate::model::LogEntry;
use crate::parser::event::{LogEvent, LogValue};
use crate::query::ast::{GroupOperator, QueryCondition, QueryExpression};
use crate::query::operator::QueryOperator;
use crate::query::value::QueryValue;
use std::collections::HashMap;

/// Shared evaluation engine that both Search and Filter delegate to.
pub struct QueryEngine;

impl QueryEngine {
    /// Evaluate a query against a structured `LogEvent`.
    pub fn evaluate_event(event: &LogEvent, expr: &QueryExpression) -> bool {
        Self::evaluate_with(&|name| field_value(event, name), expr)
    }

    /// Evaluate a query against a bare field map (e.g. unit tests, cheap paths).
    pub fn evaluate_fields(fields: &HashMap<String, LogValue>, expr: &QueryExpression) -> bool {
        Self::evaluate_with(&|name| fields.get(name).cloned(), expr)
    }

    /// Evaluate a query against the rendering model `LogEntry`.
    pub fn evaluate_entry(entry: &LogEntry, expr: &QueryExpression) -> bool {
        Self::evaluate_with(&|name| entry_value(entry, name), expr)
    }

    /// Returns `true`/`false` matching `expr` using the given field lookup.
    pub fn evaluate_with(lookup: &dyn Fn(&str) -> Option<LogValue>, expr: &QueryExpression) -> bool {
        eval_expression(lookup, expr)
    }

    fn condition_matches(lookup: &dyn Fn(&str) -> Option<LogValue>, cond: &QueryCondition) -> bool {
        let Some(target) = lookup(&cond.field) else {
            return false;
        };
        if target.is_null() {
            return false;
        }
        apply_operator(&target, cond.operator, &cond.value)
    }

    /// Build highlight ranges over the raw line for the string literals and
    /// regexes found in `expr` (case-insensitive for literals).
    pub fn find_ranges(event: &LogEvent, expr: &QueryExpression) -> Vec<(usize, usize)> {
        Self::find_ranges_in_text(&event.raw_line, expr)
    }

    /// Build highlight ranges over an arbitrary text string.
    pub fn find_ranges_in_text(text: &str, expr: &QueryExpression) -> Vec<(usize, usize)> {
        let mut ranges = Vec::new();
        if text.is_empty() {
            return ranges;
        }
        collect_ranges(text, expr, &mut ranges);
        ranges.sort();
        ranges.dedup();
        ranges
    }
}

/// Recursive evaluation routed through a concrete non-generic helper to avoid
/// rustc's recursive-trait-object instantiation abort during monomorphization.
fn eval_expression(lookup: &dyn Fn(&str) -> Option<LogValue>, expr: &QueryExpression) -> bool {
    match expr {
        QueryExpression::Condition(cond) => QueryEngine::condition_matches(lookup, cond),
        QueryExpression::Group(group) => {
            if group.children.is_empty() {
                return true;
            }
            match group.operator {
                GroupOperator::And => group
                    .children
                    .iter()
                    .all(|c| eval_expression(lookup, c)),
                GroupOperator::Or => group
                    .children
                    .iter()
                    .any(|c| eval_expression(lookup, c)),
            }
        }
    }
}

fn field_value(event: &LogEvent, name: &str) -> Option<LogValue> {
    if let Some(v) = event.fields.get(name) {
        return Some(v.clone());
    }
    match name {
        "message" => Some(LogValue::Str(event.message.clone())),
        "raw_line" | "raw" => Some(LogValue::Str(event.raw_line.clone())),
        _ => None,
    }
}

fn entry_value(entry: &LogEntry, name: &str) -> Option<LogValue> {
    match name {
        "level" => Some(LogValue::Str(entry.level.as_str().to_string())),
        "module" => Some(LogValue::Str(entry.tag.clone())),
        "message" => Some(LogValue::Str(entry.payload.clone())),
        "raw_line" | "raw" => Some(LogValue::Str(entry.raw_line.clone())),
        "requestId" | "request_id" => Some(LogValue::Str(
            entry.request_id.clone().unwrap_or_default(),
        )),
        _ => entry
            .json_fields
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| LogValue::Str(v.clone())),
    }
}

fn apply_operator(target: &LogValue, op: QueryOperator, value: &QueryValue) -> bool {
    match op {
        QueryOperator::Eq => value_eq(target, value),
        QueryOperator::Neq => !value_eq(target, value),
        QueryOperator::Contains => {
            let target = target.to_string_value().to_lowercase();
            let needle = value.to_string_value().to_lowercase();
            target.contains(&needle)
        }
        QueryOperator::StartsWith => {
            let target = target.to_string_value().to_lowercase();
            let needle = value.to_string_value().to_lowercase();
            target.starts_with(&needle)
        }
        QueryOperator::EndsWith => {
            let target = target.to_string_value().to_lowercase();
            let needle = value.to_string_value().to_lowercase();
            target.ends_with(&needle)
        }
        QueryOperator::Matches => {
            let pattern = value.to_string_value();
            regex::Regex::new(&pattern)
                .map(|re| re.is_match(&target.to_string_value()))
                .unwrap_or(false)
        }
        // `In` is desugared into an OR-of-Eq group by the parser; defensive false.
        QueryOperator::In => false,
    }
}

fn value_eq(target: &LogValue, value: &QueryValue) -> bool {
    match (target, value) {
        (LogValue::Number(n), QueryValue::Number(q)) => n == q,
        (LogValue::Number(n), QueryValue::String(s)) => {
            s.parse::<f64>().map(|f| f == *n).unwrap_or(false)
        }
        (LogValue::Bool(b), QueryValue::Boolean(q)) => b == q,
        // Cross-type equality is a non-match; boolean fields must be queried
        // with a real boolean literal (`retry == true`), not a string.
        (LogValue::Bool(_), QueryValue::String(_)) => false,
        (LogValue::Str(s), QueryValue::String(q)) => s.eq_ignore_ascii_case(q),
        (LogValue::Str(_), QueryValue::Number(_)) => false,
        (LogValue::Str(_), QueryValue::Boolean(_)) => false,
        (LogValue::Number(_), QueryValue::Boolean(_)) => false,
        (LogValue::Bool(_), QueryValue::Number(_)) => false,
        (LogValue::Null, _) => false,
    }
}

fn collect_ranges(raw: &str, expr: &QueryExpression, out: &mut Vec<(usize, usize)>) {
    match expr {
        QueryExpression::Condition(cond) => collect_condition_ranges(raw, cond, out),
        QueryExpression::Group(group) => {
            for child in &group.children {
                collect_ranges(raw, child, out);
            }
        }
    }
}

fn collect_condition_ranges(raw: &str, cond: &QueryCondition, out: &mut Vec<(usize, usize)>) {
    match cond.operator {
        QueryOperator::Eq | QueryOperator::Contains | QueryOperator::StartsWith
        | QueryOperator::EndsWith => {
            let needle = cond.value.to_string_value();
            if needle.is_empty() {
                return;
            }
            let lowercase = raw.to_lowercase();
            let needle_lower = needle.to_lowercase();
            let mut from = 0;
            while let Some(rel) = lowercase[from..].find(&needle_lower) {
                let start = from + rel;
                let end = start + needle_lower.len();
                out.push((start, end));
                from = start + needle_lower.len();
            }
        }
        QueryOperator::Matches => {
            let pattern = cond.value.as_str().unwrap_or("");
            if let Ok(re) = regex::Regex::new(pattern) {
                for m in re.find_iter(raw) {
                    out.push((m.start(), m.end()));
                }
            }
        }
        QueryOperator::Neq | QueryOperator::In => {}
    }
}