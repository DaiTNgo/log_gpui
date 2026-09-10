use log_gpui::query::diagnostic::DiagnosticSeverity;
use log_gpui::query::parser::parse;
use log_gpui::query::schema::{FieldType, LogSchema, QueryField};
use log_gpui::query::validator::validate;

fn schema() -> LogSchema {
    LogSchema {
        fields: vec![
            QueryField::new("level", FieldType::Enum).with_enum_values(vec![
                "TRACE".into(), "DEBUG".into(), "INFO".into(), "WARN".into(), "ERROR".into(),
            ]),
            QueryField::new("module", FieldType::Enum).with_enum_values(vec![
                "API".into(), "Payment".into(), "Auth".into(),
            ]),
            QueryField::new("message", FieldType::String),
            QueryField::new("status", FieldType::Number),
        ],
    }
}

#[test]
fn test_valid_query_no_diagnostics() {
    let expr = parse("level == ERROR && module == Payment").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert!(diagnostics.is_empty());
}

#[test]
fn test_valid_complex_query_no_diagnostics() {
    let expr = parse(r#"level == ERROR && (module == Payment || module == Auth)"#).unwrap();
    let diagnostics = validate(&expr, &schema());
    assert!(diagnostics.is_empty());
}

#[test]
fn test_unknown_field_diagnostic() {
    let expr = parse("levle == ERROR").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 1);
    let d = &diagnostics[0];
    assert_eq!(d.severity, DiagnosticSeverity::Error);
    assert!(d.message.contains("levle"), "message: {}", d.message);
    assert_eq!(d.span, 0..5);
}

#[test]
fn test_invalid_operator_for_field_type() {
    // Enum fields don't allow `contains`
    let expr = parse("level contains ERROR").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("contains"), "message: {}", diagnostics[0].message);
}

#[test]
fn test_invalid_enum_value() {
    let expr = parse("level == NOPE").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("NOPE"), "message: {}", diagnostics[0].message);
}

#[test]
fn test_type_mismatch() {
    // message is a String field; comparing a number should fail
    let expr = parse("message == 123").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 1);
    assert!(!diagnostics.is_empty());
}

#[test]
fn test_number_field_accepts_number() {
    let expr = parse("status == 500").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert!(diagnostics.is_empty());
}

#[test]
fn test_multiple_errors_all_reported() {
    let expr = parse("nosuchfield == X && level == NOPE").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 2);
}

#[test]
fn test_nested_group_errors_reported() {
    let expr = parse(r#"level == ERROR && (fallse == true)"#).unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("fallse"), "message: {}", diagnostics[0].message);
}

#[test]
fn test_number_type_mismatch() {
    // status is a number; passing a string literal is a mismatch
    let expr = parse("status == \"abc\"").unwrap();
    let diagnostics = validate(&expr, &schema());
    assert_eq!(diagnostics.len(), 1);
}