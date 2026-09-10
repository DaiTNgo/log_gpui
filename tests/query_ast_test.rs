use log_gpui::query::ast::{GroupOperator, QueryCondition, QueryExpression};
use log_gpui::query::diagnostic::{Diagnostic, DiagnosticSeverity};
use log_gpui::query::operator::{operators_for_type, QueryOperator};
use log_gpui::query::schema::{FieldType, LogSchema, QueryField};
use log_gpui::query::value::QueryValue;

#[test]
fn test_condition_construction() {
    let cond = QueryCondition::new("level", QueryOperator::Eq, QueryValue::String("ERROR".into()));
    assert_eq!(cond.field, "level");
    assert_eq!(cond.operator, QueryOperator::Eq);
    assert_eq!(cond.value, QueryValue::String("ERROR".into()));
}

#[test]
fn test_group_construction() {
    let c1 = QueryCondition::new("level", QueryOperator::Eq, QueryValue::String("ERROR".into()));
    let c2 = QueryCondition::new("module", QueryOperator::Eq, QueryValue::String("API".into()));
    let group = QueryExpression::group(GroupOperator::And, vec![
        QueryExpression::condition(c1),
        QueryExpression::condition(c2),
    ]);
    assert!(group.is_group());
    assert!(!group.is_condition());
    let g = group.as_group().unwrap();
    assert_eq!(g.operator, GroupOperator::And);
    assert_eq!(g.children.len(), 2);
}

#[test]
fn test_expression_helpers() {
    let cond = QueryExpression::condition(QueryCondition::new("x", QueryOperator::Contains, QueryValue::String("foo".into())));
    assert!(cond.is_condition());
    assert!(!cond.is_group());
    assert!(cond.as_condition().is_some());
    assert!(cond.as_group().is_none());
}

#[test]
fn test_operators_for_string_field() {
    let ops = operators_for_type(FieldType::String);
    assert!(ops.contains(&QueryOperator::Eq));
    assert!(ops.contains(&QueryOperator::Neq));
    assert!(ops.contains(&QueryOperator::Contains));
    assert!(ops.contains(&QueryOperator::StartsWith));
    assert!(ops.contains(&QueryOperator::EndsWith));
    assert!(ops.contains(&QueryOperator::Matches));
    assert!(!ops.contains(&QueryOperator::In));
}

#[test]
fn test_operators_for_enum_field() {
    let ops = operators_for_type(FieldType::Enum);
    assert!(ops.contains(&QueryOperator::Eq));
    assert!(ops.contains(&QueryOperator::Neq));
    assert!(ops.contains(&QueryOperator::In));
    assert!(!ops.contains(&QueryOperator::Contains));
}

#[test]
fn test_operators_for_number_field() {
    let ops = operators_for_type(FieldType::Number);
    assert!(ops.contains(&QueryOperator::Eq));
    assert!(ops.contains(&QueryOperator::Neq));
    assert!(!ops.contains(&QueryOperator::Contains));
}

#[test]
fn test_log_schema_find_field() {
    let schema = LogSchema {
        fields: vec![
            QueryField::new("level", FieldType::Enum).with_enum_values(vec!["ERROR".into(), "WARN".into()]),
            QueryField::new("message", FieldType::String),
        ],
    };
    assert!(schema.find_field("level").is_some());
    assert!(schema.find_field("message").is_some());
    assert!(schema.find_field("nonexistent").is_none());
}

#[test]
fn test_diagnostic_construction() {
    let d = Diagnostic::error("unknown field".into(), 0..5);
    assert_eq!(d.severity, DiagnosticSeverity::Error);
    assert_eq!(d.message, "unknown field");
    assert_eq!(d.span, 0..5);
}

#[test]
fn test_query_value_equality() {
    assert_eq!(QueryValue::String("a".into()), QueryValue::String("a".into()));
    assert_ne!(QueryValue::String("a".into()), QueryValue::String("b".into()));
    assert_eq!(QueryValue::Number(1.0), QueryValue::Number(1.0));
    assert_ne!(QueryValue::Number(1.0), QueryValue::Number(2.0));
    assert_eq!(QueryValue::Boolean(true), QueryValue::Boolean(true));
    assert_ne!(QueryValue::Boolean(true), QueryValue::Boolean(false));
}
