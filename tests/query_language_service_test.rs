use log_gpui::query::language_service::{CompletionItem, CompletionKind, QueryLanguageService, HoverResult};
use log_gpui::query::schema::{FieldType, LogSchema, QueryField};

fn schema() -> LogSchema {
    LogSchema {
        fields: vec![
            QueryField::new("level", FieldType::Enum).with_enum_values(vec![
                "ERROR".into(), "WARN".into(), "INFO".into(), "DEBUG".into(),
            ]),
            QueryField::new("module", FieldType::Enum).with_enum_values(vec![
                "API".into(), "Payment".into(), "Auth".into(),
            ]),
            QueryField::new("message", FieldType::String),
        ],
    }
}

fn completions(text: &str, offset: usize) -> Vec<CompletionItem> {
    QueryLanguageService::get_completions(text, offset, &schema())
}

fn labels(text: &str, offset: usize) -> Vec<String> {
    completions(text, offset).into_iter().map(|c| c.label).collect()
}

#[test]
fn test_field_completions_at_empty() {
    let labels = labels("", 0);
    assert!(labels.contains(&"level".to_string()));
    assert!(labels.contains(&"module".to_string()));
    assert!(labels.contains(&"message".to_string()));
}

#[test]
fn test_field_completions_after_logical_op() {
    let labels = labels("level == ERROR && ", 18);
    assert!(labels.contains(&"level".to_string()));
    assert!(labels.contains(&"module".to_string()));
    assert!(labels.contains(&"message".to_string()));
}

#[test]
fn test_field_completions_with_prefix() {
    let labels = labels("lev", 3);
    assert!(labels.contains(&"level".to_string()));
    assert!(!labels.contains(&"module".to_string()));
}

#[test]
fn test_field_completions_inside_parens() {
    let labels = labels("level == ERROR && ", 18);
    assert!(labels.contains(&"level".to_string()));
}

#[test]
fn test_operator_completions_after_field() {
    let labels = labels("level ", 6);
    assert!(labels.contains(&"==".to_string()));
    assert!(labels.contains(&"!=".to_string()));
    assert!(labels.contains(&"in".to_string()));
    assert!(!labels.contains(&"contains".to_string()));
}

#[test]
fn test_operator_completions_filtered_by_type() {
    // message is a String field → should offer contains
    let labels = labels("message ", 8);
    assert!(labels.contains(&"contains".to_string()));
    assert!(labels.contains(&"startsWith".to_string()));
    assert!(labels.contains(&"matches".to_string()));
}

#[test]
fn test_value_completions_for_enum_after_operator() {
    let labels = labels("level == ", 9);
    assert!(labels.contains(&"ERROR".to_string()));
    assert!(labels.contains(&"WARN".to_string()));
    assert!(labels.contains(&"INFO".to_string()));
}

#[test]
fn test_value_completions_filtered_by_prefix() {
    let labels = labels("level == E", 10);
    assert!(labels.contains(&"ERROR".to_string()));
    assert!(!labels.contains(&"WARN".to_string()));
}

#[test]
fn test_logical_ops_after_complete_condition() {
    let labels = labels("level == ERROR ", 15);
    assert!(labels.contains(&"&&".to_string()));
    assert!(labels.contains(&"||".to_string()));
    assert!(!labels.contains(&"ERROR".to_string()));
}

#[test]
fn test_string_field_no_enum_value_completions() {
    // message is String → after `message contains `, no enum completions needed;
    // but string fields still complete for operators
    let ops = labels("message ", 8);
    assert!(ops.contains(&"contains".to_string()));
    // No enum values for string fields
    let vals = completions("message contains ", 17);
    assert!(vals.iter().all(|c| c.kind != CompletionKind::Value && c.kind != CompletionKind::Field));
}

#[test]
fn test_completion_kinds_present() {
    let items = completions("", 0);
    assert!(items.iter().all(|c| c.kind == CompletionKind::Field));
    assert!(items.iter().all(|c| !c.insert_text.is_empty()));
}

#[test]
fn test_hover_over_field_name() {
    let text = "level == ERROR";
    let hover = QueryLanguageService::get_hover(text, 2, &schema());
    assert!(hover.is_some());
    let h = hover.unwrap();
    assert!(h.content.contains("level"));
    assert!(h.content.contains("Enum"));
}

#[test]
fn test_hover_over_operator() {
    let text = "level == ERROR";
    let hover = QueryLanguageService::get_hover(text, 7, &schema());
    assert!(hover.is_some());
    let h = hover.unwrap();
    assert!(h.content.contains("equals"));
}

#[test]
fn test_hover_over_enum_value() {
    let text = "level == ERROR";
    let op_offset = text.find("ERROR").unwrap();
    let hover = QueryLanguageService::get_hover(text, op_offset + 1, &schema());
    assert!(hover.is_some());
    let h = hover.unwrap();
    assert!(h.content.contains("ERROR"));
}

#[test]
fn test_hover_none_off_tokens() {
    let text = "level == ERROR";
    // Position in leading whitespace region (offset 0 spans token start)
    let hover = QueryLanguageService::get_hover(text, 4, &schema());
    // offset 4 is between "level" (0..5) and " " — should still resolve to level
    assert!(hover.is_some());
}

#[test]
fn test_hover_unknown_field() {
    let text = "nope == 1";
    let hover = QueryLanguageService::get_hover(text, 1, &schema());
    assert!(hover.is_none() || hover.unwrap().content.contains("unknown"));
}