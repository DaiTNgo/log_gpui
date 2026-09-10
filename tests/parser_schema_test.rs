use log_gpui::model::LogLevel;
use log_gpui::parser::event::{json_to_log_value, LogEvent, LogValue};
use log_gpui::parser::{parse_file_with_schema, parse_text_bytes_with_max_chars};
use log_gpui::query::schema::FieldType;
use std::collections::HashMap;

#[test]
fn test_json_schema_discovers_fields_and_types() {
    let content = r#"{"level":"ERROR","service":"API","message":"timeout","duration":1500,"retry":true}
{"level":"INFO","service":"DB","message":"connected","duration":12,"retry":false}
{"level":"ERROR","service":"API","message":"refused","duration":999,"retry":true}"#;

    std::fs::write("/tmp/_query_schema_test.log", content.as_bytes()).unwrap();
    let result = parse_file_with_schema("/tmp/_query_schema_test.log").unwrap();
    let level = result.schema.find_field("level").expect("level field");
    assert_eq!(level.field_type, FieldType::Enum);
    let values = level.enum_values.as_ref().expect("enum values");
    assert!(values.iter().any(|v| v == "ERROR"));
    assert!(values.iter().any(|v| v == "INFO"));

    // message should be String
    let message = result.schema.find_field("message").expect("message field");
    assert_eq!(message.field_type, FieldType::String);

    // duration should be Number
    let duration = result.schema.find_field("duration").expect("duration field");
    assert_eq!(duration.field_type, FieldType::Number);

    // retry should be Boolean
    let retry = result.schema.find_field("retry").expect("retry field");
    assert_eq!(retry.field_type, FieldType::Boolean);

    // module (from service alias) present
    assert!(result.schema.find_field("module").is_some());

    assert_eq!(result.events.len(), 3);
}

#[test]
fn test_text_schema_has_canonical_fields() {
    let content = "2026-09-05 12:34:56.789 [ERROR] [api] Connection timeout\n2026-09-05 12:34:57.000 [INFO] [db] Query ok\n";
    std::fs::write("/tmp/_query_schema_text.log", content.as_bytes()).unwrap();
    let result = parse_file_with_schema("/tmp/_query_schema_text.log").unwrap();

    assert_eq!(result.events.len(), 2);
    assert!(result.schema.find_field("level").is_some());
    assert!(result.schema.find_field("module").is_some());
    assert!(result.schema.find_field("time").is_some());
}

#[test]
fn test_level_and_module_schema_types() {
    let content = "2026-09-05 12:34:56.789 [ERROR] [api] boom\n2026-09-05 12:34:57.000 [WARN] [api] slow\n";
    std::fs::write("/tmp/_query_schema_levels.log", content.as_bytes()).unwrap();
    let result = parse_file_with_schema("/tmp/_query_schema_levels.log").unwrap();

    let level = result.schema.find_field("level").unwrap();
    assert_eq!(level.field_type, FieldType::Enum);
}

#[test]
fn test_enum_cardinality_cap_turns_high_cardinality_to_string() {
    let mut lines = Vec::new();
    for i in 0..30 {
        lines.push(format!(r#"{{"level":"INFO","message":"msg{}","unique":"val{}"}}"#, i, i));
    }
    let content = lines.join("\n");
    std::fs::write("/tmp/_query_schema_cardinality.jsonl", content.as_bytes()).unwrap();
    let result = parse_file_with_schema("/tmp/_query_schema_cardinality.jsonl").unwrap();

    // "unique" has 30 distinct values → String, not Enum
    let unique = result.schema.find_field("unique").expect("unique field");
    assert_eq!(unique.field_type, FieldType::String);
}

#[test]
fn test_json_event_fields_and_message() {
    let content = r#"{"level":"ERROR","service":"Payments","message":"processor timeout","amount":42.5}"#;
    std::fs::write("/tmp/_query_schema_event.jsonl", content.as_bytes()).unwrap();
    let result = parse_file_with_schema("/tmp/_query_schema_event.jsonl").unwrap();

    let event = &result.events[0];
    assert_eq!(event.get_field("level").and_then(|v| v.as_str()), Some("ERROR"));
    assert_eq!(event.get_field("module").and_then(|v| v.as_str()), Some("Payments"));
    assert_eq!(event.get_field("message").and_then(|v| v.as_str()), Some("processor timeout"));
    assert_eq!(event.get_field("amount").and_then(|v| v.as_f64()), Some(42.5));
}

#[test]
fn test_logevent_to_logentry_conversion() {
    let mut event = LogEvent::new(1, 100.0, "10:00:00.000".into(), "boom".into(), "raw".into());
    let mut fields = HashMap::new();
    fields.insert("level".to_string(), LogValue::Str("ERROR".into()));
    fields.insert("module".to_string(), LogValue::Str("API".into()));
    fields.insert("requestId".to_string(), LogValue::Str("req-1".into()));
    event.fields = fields;

    let entry = log_gpui::model::LogEntry::from_event(&event);
    assert_eq!(entry.level, LogLevel::Error);
    assert_eq!(entry.tag, "API");
    assert_eq!(entry.payload, "boom");
    assert_eq!(entry.request_id.as_deref(), Some("req-1"));
}

#[test]
fn test_json_to_log_value_conversion() {
    assert_eq!(json_to_log_value(&serde_json::json!("hello")), LogValue::Str("hello".into()));
    assert_eq!(json_to_log_value(&serde_json::json!(42)), LogValue::Number(42.0));
    assert_eq!(json_to_log_value(&serde_json::json!(true)), LogValue::Bool(true));
    assert_eq!(json_to_log_value(&serde_json::Value::Null), LogValue::Null);
}

#[test]
fn test_legacy_parse_result_still_works() {
    let content = "2026-09-05 12:34:56.789 [ERROR] [api] Connection timeout\n";
    std::fs::write("/tmp/_query_schema_legacy.log", content.as_bytes()).unwrap();
    let result = parse_text_bytes_with_max_chars(content.as_bytes()).unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].level, LogLevel::Error);
}