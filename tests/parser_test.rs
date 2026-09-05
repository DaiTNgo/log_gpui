use log_gpui::model::LogLevel;
use log_gpui::parser::text::parse_text_bytes;
use log_gpui::ui::inspector::format_hex_dump;

#[test]
fn test_parse_text_logs() {
    let log_content = b"2026-09-05 12:00:01.123 [WARN] [NAVI] GPS satellite count low\n2026-09-05 12:00:02.456 [ERROR] [BODY] Power window pinch detect\n";
    let entries = parse_text_bytes(log_content).expect("Parsing failed");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].index, 1);
    assert_eq!(entries[0].level, LogLevel::Warn);
    assert_eq!(entries[0].tag, "NAVI");
    assert_eq!(entries[0].payload, "GPS satellite count low");

    assert_eq!(entries[1].index, 2);
    assert_eq!(entries[1].level, LogLevel::Error);
    assert_eq!(entries[1].tag, "BODY");
}

#[test]
fn test_hex_dump_formatting() {
    let data = b"DLT\x01Hello World!";
    let dump = format_hex_dump(data);

    assert!(dump.contains("44 4c 54 01"));
    assert!(dump.contains("|DLT.Hello World!|"));
}
