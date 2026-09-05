use log_gpui::filter::{FilterEngine, FilterRule, FilterType};
use log_gpui::model::{LogEntry, LogLevel};

#[test]
fn test_filter_engine_include_exclude_marker() {
    let mut entries = vec![
        LogEntry::new(1, 100.0, "100.0".into(), "NAVI".into(), LogLevel::Info, "Navigation init".into(), "raw 1".into()),
        LogEntry::new(2, 100.1, "100.1".into(), "NAVI".into(), LogLevel::Warn, "GPS signal weak".into(), "raw 2".into()),
        LogEntry::new(3, 100.2, "100.2".into(), "BODY".into(), LogLevel::Error, "Door latch failure".into(), "raw 3".into()),
        LogEntry::new(4, 100.3, "100.3".into(), "ENG_".into(), LogLevel::Verbose, "RPM: 2200".into(), "raw 4".into()),
    ];

    // 1. Exclude rule on Verbose
    let mut exclude_rule = FilterRule::new(1, "Exclude Verbose".into(), FilterType::Exclude);
    exclude_rule.exact_level = Some(LogLevel::Verbose);

    // 2. Marker rule on Error
    let mut marker_rule = FilterRule::new(2, "Highlight Error".into(), FilterType::Marker);
    marker_rule.min_level = Some(LogLevel::Error);
    marker_rule.marker_color = Some("red".into());

    let mut rules = vec![exclude_rule, marker_rule];

    let visible = FilterEngine::apply_filters(&mut rules, &mut entries, "");

    // Entry 4 is Verbose and should be excluded
    assert_eq!(visible.len(), 3);
    assert_eq!(visible, vec![0, 1, 2]);

    // Entry 3 (Error) should have marker_color Some("red")
    assert_eq!(entries[2].marker_color.as_deref(), Some("red"));

    // Entry 2 delta time: 100.1 - 100.0 = 0.1s = 100ms
    assert!((entries[1].delta_time_ms.unwrap() - 100.0).abs() < 0.01);
}

#[test]
fn test_quick_search() {
    let mut entries = vec![
        LogEntry::new(1, 10.0, "10.0".into(), "NAVI".into(), LogLevel::Info, "Connected to GPS".into(), "raw 1".into()),
        LogEntry::new(2, 10.1, "10.1".into(), "BODY".into(), LogLevel::Info, "Door opened".into(), "raw 2".into()),
    ];

    let mut rules = Vec::new();
    let visible = FilterEngine::apply_filters(&mut rules, &mut entries, "GPS");

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0], 0);
}
