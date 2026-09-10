use log_gpui::query::ast::{GroupOperator, QueryExpression};
use log_gpui::query::operator::QueryOperator;
use log_gpui::query::parser::parse;
use log_gpui::query::value::QueryValue;

fn condition(expr: &QueryExpression) -> &log_gpui::query::ast::QueryCondition {
    expr.as_condition().expect("expected condition")
}

fn group(expr: &QueryExpression) -> &log_gpui::query::ast::QueryGroup {
    expr.as_group().expect("expected group")
}

#[test]
fn test_single_equals_condition() {
    let result = parse("level == ERROR").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.field, "level");
    assert_eq!(cond.operator, QueryOperator::Eq);
    assert_eq!(cond.value, QueryValue::String("ERROR".into()));
}

#[test]
fn test_single_not_equals_condition() {
    let result = parse("module != Auth").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.field, "module");
    assert_eq!(cond.operator, QueryOperator::Neq);
    assert_eq!(cond.value, QueryValue::String("Auth".into()));
}

#[test]
fn test_contains_condition() {
    let result = parse("message contains \"timeout\"").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.field, "message");
    assert_eq!(cond.operator, QueryOperator::Contains);
    assert_eq!(cond.value, QueryValue::String("timeout".into()));
}

#[test]
fn test_starts_with_condition() {
    let result = parse("message startsWith \"Connection\"").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.operator, QueryOperator::StartsWith);
}

#[test]
fn test_ends_with_condition() {
    let result = parse("message endsWith \"refused\"").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.operator, QueryOperator::EndsWith);
}

#[test]
fn test_matches_condition() {
    let result = parse("payload matches \"err\\d+\"").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.operator, QueryOperator::Matches);
    assert_eq!(cond.value, QueryValue::String("err\\d+".into()));
}

#[test]
fn test_and_group() {
    let result = parse("level == ERROR && module == Payment").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::And);
    assert_eq!(g.children.len(), 2);
    assert_eq!(g.children[0].as_condition().unwrap().field, "level");
    assert_eq!(g.children[1].as_condition().unwrap().field, "module");
}

#[test]
fn test_or_group() {
    let result = parse("level == ERROR || level == WARN").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::Or);
    assert_eq!(g.children.len(), 2);
}

#[test]
fn test_and_keyword() {
    let result = parse("level == ERROR AND module == API").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::And);
}

#[test]
fn test_or_keyword() {
    let result = parse("level == ERROR OR level == WARN").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::Or);
}

#[test]
fn test_parenthesized_group() {
    let result = parse("level == ERROR && (module == Payment || module == Auth)").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::And);
    assert_eq!(g.children.len(), 2);

    let first = condition(&g.children[0]);
    assert_eq!(first.field, "level");

    let inner = group(&g.children[1]);
    assert_eq!(inner.operator, GroupOperator::Or);
    assert_eq!(inner.children.len(), 2);
}

#[test]
fn test_operator_precedence_and_tighter_than_or() {
    // a || b && c should parse as a || (b && c)
    let result = parse("level == ERROR || module == Payment && message contains \"err\"").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::Or);
    // Second child should be a group (&& binds tighter)
    assert!(g.children[1].is_group());
    let inner = g.children[1].as_group().unwrap();
    assert_eq!(inner.operator, GroupOperator::And);
}

#[test]
fn test_string_value_with_spaces() {
    let result = parse("message contains \"hello world\"").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.value, QueryValue::String("hello world".into()));
}

#[test]
fn test_numeric_value() {
    let result = parse("status == 500").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.value, QueryValue::Number(500.0));
}

#[test]
fn test_boolean_value() {
    let result = parse("active == true").unwrap();
    let cond = condition(&result);
    assert_eq!(cond.value, QueryValue::Boolean(true));
}

#[test]
fn test_in_operator() {
    // `in [a, b, c]` desugars to a group of Eq conditions
    let result = parse("level in (ERROR, WARN, FATAL)").unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::Or);
    assert_eq!(g.children.len(), 3);
    for child in &g.children {
        let cond = child.as_condition().unwrap();
        assert_eq!(cond.field, "level");
        assert_eq!(cond.operator, QueryOperator::Eq);
    }
}

#[test]
fn test_empty_input_error() {
    let result = parse("");
    assert!(result.is_err());
}

#[test]
fn test_whitespace_only_error() {
    let result = parse("   ");
    assert!(result.is_err());
}

#[test]
fn test_unclosed_paren_error() {
    let result = parse("level == ERROR && (module == Payment");
    assert!(result.is_err());
}

#[test]
fn test_trailing_and_error() {
    let result = parse("level == ERROR &&");
    assert!(result.is_err());
}

#[test]
fn test_complex_query() {
    let input = r#"level == ERROR && module == Payment && (message contains "timeout" || payload contains "refused")"#;
    let result = parse(input).unwrap();
    let g = group(&result);
    assert_eq!(g.operator, GroupOperator::And);
    assert_eq!(g.children.len(), 3);
    // Third child is a group
    assert!(g.children[2].is_group());
    let inner = g.children[2].as_group().unwrap();
    assert_eq!(inner.operator, GroupOperator::Or);
    assert_eq!(inner.children.len(), 2);
}
