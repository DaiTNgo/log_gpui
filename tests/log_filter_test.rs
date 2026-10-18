use log_gpui::filter::{FilterState, parse_time_bound};
use log_gpui::filter_index::{FilterIndex, MAX_DISTINCT};
use log_gpui::marker::MarkerType;
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::workspace::log_space::{LogFile, LogSpace, LogSpaceKind};

fn entry(index: usize, ts: f64, module: &str, level: LogLevel, message: &str) -> LogEntry {
    LogEntry::new(
        index,
        ts,
        format!("{ts}"),
        module.into(),
        level,
        message.into(),
        format!("{ts} {module} {message}"),
    )
}

fn sample() -> Vec<LogEntry> {
    vec![
        entry(1, 100.0, "API", LogLevel::Info, "start"),
        entry(2, 101.0, "DB", LogLevel::Debug, "query"),
        entry(3, 102.0, "API", LogLevel::Error, "timeout"),
        entry(4, 103.0, "PAYMENT", LogLevel::Warn, "slow"),
        entry(5, 104.0, "paypal", LogLevel::Info, "ok"),
        entry(6, 105.0, "", LogLevel::Info, "no module"),
    ]
}

fn visible(state: &FilterState, entries: &mut [LogEntry]) -> Vec<usize> {
    let index = FilterIndex::build(entries);
    state.evaluate(&index, entries)
}

// ─── Detection ───────────────────────────────────────────────────────────────

#[test]
fn detects_columns_values_and_counts() {
    let entries = sample();
    let index = FilterIndex::build(&entries);
    let module = index.column("module").unwrap();
    assert!(!module.overflow);
    assert_eq!(module.distinct_label(), "4");
    let api = module.values.iter().find(|v| v.display == "API").unwrap();
    assert_eq!(api.count, 2);
    let level = index.column("level").unwrap();
    assert_eq!(level.values.len(), 4);
}

#[test]
fn json_fields_become_columns() {
    let mut entries = sample();
    entries[0].json_fields.push(("region".into(), "us".into()));
    entries[2].json_fields.push(("region".into(), "eu".into()));
    entries[3].json_fields.push(("region".into(), "us".into()));
    let index = FilterIndex::build(&entries);
    let region = index.column("region").expect("json column");
    assert_eq!(region.values.len(), 2);
    assert_eq!(region.label, "region");
}

#[test]
fn timestamp_and_message_are_never_columns() {
    let index = FilterIndex::build(&sample());
    assert!(index.column("timestamp").is_none());
    assert!(index.column("message").is_none());
    assert!(index.columns_in_use().all(|c| c.key != "message" && c.key != "timestamp"));
}

#[test]
fn columns_without_any_value_are_not_listed() {
    let index = FilterIndex::build(&sample());
    assert!(index.columns_in_use().all(|c| c.key != "request_id" && c.key != "trace_id"));
}

#[test]
fn case_variants_are_one_value_shown_in_the_most_frequent_form() {
    let entries = vec![
        entry(1, 1.0, "api", LogLevel::Info, "a"),
        entry(2, 2.0, "API", LogLevel::Info, "b"),
        entry(3, 3.0, "API", LogLevel::Info, "c"),
        entry(4, 4.0, "Api", LogLevel::Info, "d"),
    ];
    let index = FilterIndex::build(&entries);
    let module = index.column("module").unwrap();
    assert_eq!(module.values.len(), 1);
    assert_eq!(module.values[0].display, "API");
    assert_eq!(module.values[0].count, 4);
}

#[test]
fn diacritics_stay_separate_values() {
    let entries = vec![
        entry(1, 1.0, "lỗi", LogLevel::Info, "a"),
        entry(2, 2.0, "loi", LogLevel::Info, "b"),
    ];
    let index = FilterIndex::build(&entries);
    assert_eq!(index.column("module").unwrap().values.len(), 2);
}

#[test]
fn a_column_over_the_limit_is_unavailable() {
    let entries: Vec<LogEntry> = (0..MAX_DISTINCT + 20)
        .map(|i| entry(i + 1, (i + 1) as f64, &format!("mod{i}"), LogLevel::Info, "x"))
        .collect();
    let index = FilterIndex::build(&entries);
    let module = index.column("module").unwrap();
    assert!(module.overflow);
    assert!(!module.is_filterable());
    assert_eq!(module.distinct_label(), format!("> {}", MAX_DISTINCT));
    assert!(module.values.is_empty());
}

#[test]
fn exactly_the_limit_is_still_available() {
    let entries: Vec<LogEntry> = (0..MAX_DISTINCT)
        .map(|i| entry(i + 1, (i + 1) as f64, &format!("mod{i}"), LogLevel::Info, "x"))
        .collect();
    let index = FilterIndex::build(&entries);
    assert!(index.column("module").unwrap().is_filterable());
}

// ─── Filter Values, Custom Values, Hide Mode ─────────────────────────────────

#[test]
fn ticking_values_ors_within_a_column() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.toggle_value("module", "api");
    f.toggle_value("module", "DB");
    assert_eq!(visible(&f, &mut entries), vec![0, 1, 2]);
}

#[test]
fn columns_and_together() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    f.toggle_value("level", "ERROR");
    assert_eq!(visible(&f, &mut entries), vec![2]);
}

#[test]
fn toggling_a_value_twice_removes_it() {
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    assert!(f.is_value_ticked("module", "api"));
    f.toggle_value("module", "api");
    assert!(f.is_empty());
}

#[test]
fn custom_value_matches_contains_case_insensitively() {
    let mut entries = sample();
    let mut f = FilterState::new();
    assert!(f.add_custom("module", "PAY"));
    assert_eq!(visible(&f, &mut entries), vec![3, 4]);
}

#[test]
fn custom_value_is_not_a_regex() {
    let mut entries = vec![
        entry(1, 1.0, "a.c", LogLevel::Info, "x"),
        entry(2, 2.0, "abc", LogLevel::Info, "y"),
    ];
    let mut f = FilterState::new();
    f.add_custom("module", "a.c");
    assert_eq!(visible(&f, &mut entries), vec![0]);
}

#[test]
fn blank_and_duplicate_custom_values_are_ignored() {
    let mut f = FilterState::new();
    assert!(!f.add_custom("module", "   "));
    assert!(f.add_custom("module", "pay"));
    assert!(!f.add_custom("module", "PAY"));
    assert_eq!(f.column("module").unwrap().custom.len(), 1);
}

#[test]
fn ticked_values_and_custom_values_or_inside_a_column() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.toggle_value("module", "DB");
    f.add_custom("module", "pay");
    assert_eq!(visible(&f, &mut entries), vec![1, 3, 4]);
}

#[test]
fn hide_mode_hides_ticked_values() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.toggle_value("level", "DEBUG");
    f.set_hide("level", true);
    assert_eq!(visible(&f, &mut entries), vec![0, 2, 3, 4, 5]);
}

#[test]
fn hide_mode_hides_custom_matches() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.add_custom("module", "pay");
    f.set_hide("module", true);
    assert_eq!(visible(&f, &mut entries), vec![0, 1, 2, 5]);
}

#[test]
fn row_without_a_value_never_matches() {
    let mut entries = sample(); // row 5 has an empty module
    let mut show = FilterState::new();
    show.toggle_value("module", "API");
    assert!(!visible(&show, &mut entries).contains(&5));

    let mut hide = FilterState::new();
    hide.toggle_value("module", "API");
    hide.set_hide("module", true);
    assert!(visible(&hide, &mut entries).contains(&5), "stays visible in Hide Mode");
}

#[test]
fn unknown_level_counts_as_missing() {
    let mut entries = vec![
        entry(1, 1.0, "api", LogLevel::Unknown, "a"),
        entry(2, 2.0, "api", LogLevel::Error, "b"),
    ];
    let mut f = FilterState::new();
    f.toggle_value("level", "ERROR");
    f.set_hide("level", true);
    assert_eq!(visible(&f, &mut entries), vec![0]);
}

#[test]
fn counts_do_not_change_with_other_filters() {
    let mut entries = sample();
    let before = FilterIndex::build(&entries);
    let api = before.column("module").unwrap().values.iter().find(|v| v.display == "API").unwrap().count;
    let mut f = FilterState::new();
    f.toggle_value("level", "ERROR");
    let _ = f.evaluate(&before, &mut entries);
    let after = FilterIndex::build(&entries);
    assert_eq!(
        after.column("module").unwrap().values.iter().find(|v| v.display == "API").unwrap().count,
        api
    );
}

#[test]
fn clearing_a_column_removes_its_conditions() {
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    f.add_custom("level", "err");
    f.clear_column("module");
    assert!(f.column("module").is_none());
    assert!(f.column("level").is_some());
    f.clear();
    assert!(f.is_empty());
}

#[test]
fn active_count_counts_columns_and_the_time_range() {
    let mut f = FilterState::new();
    assert_eq!(f.active_count(), 0);
    f.toggle_value("module", "API");
    f.add_custom("module", "x");
    f.toggle_value("level", "ERROR");
    assert_eq!(f.active_count(), 2);
    f.set_time_range("10:00", "");
    assert_eq!(f.active_count(), 3);
}

#[test]
fn filter_values_match_unicode_case() {
    let mut entries = vec![
        entry(1, 1.0, "ĐÃ", LogLevel::Info, "a"),
        entry(2, 2.0, "khác", LogLevel::Info, "b"),
    ];
    let mut f = FilterState::new();
    f.toggle_value("module", "đã");
    assert_eq!(visible(&f, &mut entries), vec![0]);
    let mut c = FilterState::new();
    c.add_custom("module", "đ");
    assert_eq!(visible(&c, &mut entries), vec![0]);
}

#[test]
fn a_condition_on_a_column_missing_from_the_file_hides_everything() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.toggle_value("region", "us");
    assert!(visible(&f, &mut entries).is_empty());
    f.set_hide("region", true);
    assert_eq!(visible(&f, &mut entries).len(), entries.len());
}

#[test]
fn stale_index_falls_back_to_comparing_text() {
    let mut entries = sample();
    let index = FilterIndex::build(&entries);
    entries.push(entry(7, 106.0, "API", LogLevel::Info, "late"));
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    assert_eq!(f.evaluate(&index, &mut entries), vec![0, 2, 6]);
}

#[test]
fn overflowed_column_conditions_still_apply() {
    let mut entries: Vec<LogEntry> = (0..MAX_DISTINCT + 5)
        .map(|i| entry(i + 1, (i + 1) as f64, &format!("mod{i}"), LogLevel::Info, "x"))
        .collect();
    let mut f = FilterState::new();
    f.toggle_value("module", "mod3");
    f.add_custom("module", "mod50");
    let v = visible(&f, &mut entries);
    assert!(v.contains(&3));
    assert!(v.contains(&50));
    assert!(!v.contains(&4));
}

#[test]
fn evaluate_refreshes_delta_time_of_visible_rows() {
    let mut entries = sample();
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    let _ = visible(&f, &mut entries);
    assert_eq!(entries[0].delta_time_ms, Some(0.0));
    assert_eq!(entries[2].delta_time_ms, Some(2000.0));
}

// ─── Tail ────────────────────────────────────────────────────────────────────

#[test]
fn tail_appends_new_values_and_counts() {
    let mut entries = sample();
    let mut index = FilterIndex::build(&entries);
    let before: Vec<String> = index.column("module").unwrap().values.iter().map(|v| v.display.clone()).collect();

    let new = vec![
        entry(7, 106.0, "SEARCH", LogLevel::Info, "new"),
        entry(8, 107.0, "API", LogLevel::Info, "again"),
    ];
    index.append(&new);
    entries.extend(new);

    let module = index.column("module").unwrap();
    let shown: Vec<&str> = module.values.iter().map(|v| v.display.as_str()).collect();
    assert_eq!(&shown[..before.len()], before.iter().map(String::as_str).collect::<Vec<_>>().as_slice());
    assert_eq!(shown.last(), Some(&"SEARCH"), "new value is appended last");
    assert_eq!(module.values.iter().find(|v| v.display == "API").unwrap().count, 3);
    assert_eq!(index.rows(), entries.len());

    let mut f = FilterState::new();
    f.toggle_value("module", "SEARCH");
    assert_eq!(f.evaluate(&index, &mut entries), vec![6]);
}

#[test]
fn tail_crossing_the_limit_keeps_existing_conditions() {
    let mut entries: Vec<LogEntry> = (0..MAX_DISTINCT)
        .map(|i| entry(i + 1, (i + 1) as f64, &format!("mod{i}"), LogLevel::Info, "x"))
        .collect();
    let mut index = FilterIndex::build(&entries);
    assert!(index.column("module").unwrap().is_filterable());

    let mut f = FilterState::new();
    f.toggle_value("module", "mod7");

    let new = vec![entry(MAX_DISTINCT + 1, 9999.0, "brand-new", LogLevel::Info, "x")];
    index.append(&new);
    entries.extend(new);

    assert!(!index.column("module").unwrap().is_filterable());
    assert_eq!(f.evaluate(&index, &mut entries), vec![7], "condition keeps working");
}

// ─── Time Range ──────────────────────────────────────────────────────────────

fn timed() -> Vec<LogEntry> {
    // Day anchored at 86400 * 20000 so time-of-day math is easy.
    let day = 86400.0 * 20000.0;
    let mk = |i: usize, secs: f64, msg: &str| {
        let mut e = entry(i, day + secs, "API", LogLevel::Info, msg);
        e.time_str = format!("{secs}");
        e
    };
    vec![
        mk(1, 10.0 * 3600.0, "10:00:00"),
        mk(2, 10.0 * 3600.0 + 120.0, "10:02:00"),
        mk(3, 10.0 * 3600.0 + 300.0, "10:05:00"),
        mk(4, 10.0 * 3600.0 + 600.0, "10:10:00"),
    ]
}

#[test]
fn time_range_with_both_bounds() {
    let mut entries = timed();
    let mut f = FilterState::new();
    f.set_time_range("10:01:00", "10:05:00");
    assert_eq!(visible(&f, &mut entries), vec![1, 2]);
}

#[test]
fn time_range_with_one_bound() {
    let mut entries = timed();
    let mut from = FilterState::new();
    from.set_time_range("10:05:00", "");
    assert_eq!(visible(&from, &mut entries), vec![2, 3]);
    let mut to = FilterState::new();
    to.set_time_range("", "10:00:00");
    assert_eq!(visible(&to, &mut entries), vec![0]);
}

#[test]
fn invalid_time_bound_is_unbounded() {
    let mut entries = timed();
    let mut f = FilterState::new();
    f.set_time_range("not a time", "");
    assert_eq!(visible(&f, &mut entries).len(), 4);
}

#[test]
fn rows_without_a_timestamp_inherit_the_row_above() {
    let mut entries = timed();
    // Row 1 (10:02) gets a continuation row with no timestamp of its own.
    let mut cont = entry(5, 0.0, "API", LogLevel::Info, "  at stack frame");
    cont.timestamp = 0.0;
    entries.insert(2, cont);
    let mut f = FilterState::new();
    f.set_time_range("10:01:00", "10:03:00");
    // rows: 10:00, 10:02, (cont → 10:02), 10:05, 10:10
    assert_eq!(visible(&f, &mut entries), vec![1, 2]);
}

#[test]
fn rows_before_the_first_timestamp_are_always_shown() {
    let mut entries = timed();
    let mut head = entry(0, 0.0, "API", LogLevel::Info, "banner");
    head.timestamp = 0.0;
    entries.insert(0, head);
    let mut f = FilterState::new();
    f.set_time_range("11:00:00", "12:00:00");
    assert_eq!(visible(&f, &mut entries), vec![0]);
}

#[test]
fn time_range_ands_with_columns() {
    let mut entries = timed();
    entries[1].tag = "DB".into();
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    f.set_time_range("10:01:00", "10:06:00");
    assert_eq!(visible(&f, &mut entries), vec![2]);
}

#[test]
fn parse_time_bound_formats() {
    let day = 86400.0 * 10.0;
    assert_eq!(parse_time_bound("", day), Ok(None));
    assert_eq!(parse_time_bound("  ", day), Ok(None));
    assert_eq!(parse_time_bound("10:42:03", day), Ok(Some(day + 38523.0)));
    assert_eq!(parse_time_bound("10:42", day), Ok(Some(day + 38520.0)));
    assert_eq!(parse_time_bound("10:42:03.250", day), Ok(Some(day + 38523.25)));
    let full = parse_time_bound("2026-10-17 10:42:03.250", day).unwrap().unwrap();
    let iso = parse_time_bound("2026-10-17T10:42:03.250Z", day).unwrap().unwrap();
    assert_eq!(full, iso);
    assert_eq!(full, 1_792_233_723.25);
    assert_eq!(parse_time_bound("1757048400.5", day), Ok(Some(1757048400.5)));
    assert!(parse_time_bound("nope", day).is_err());
    assert!(parse_time_bound("25:99", day).is_err());
}

#[test]
fn time_hint_shows_the_first_and_last_time() {
    let index = FilterIndex::build(&timed());
    let (first, last) = index.time_hint().unwrap();
    assert_eq!(first, "36000");
    assert_eq!(last, "36600");
}

// ─── Per-file Filter and Markers ─────────────────────────────────────────────

#[test]
fn markers_round_trip_through_the_file_descriptor() {
    let mut entries = sample();
    entries[1].marker = Some(MarkerType::Bookmark);
    entries[3].note = Some("investigate".into());
    entries[3].marker = Some(MarkerType::Note);

    let mut file = LogFile::new("app.log".into(), "app.log".into());
    file.capture_markers(&entries);
    assert_eq!(file.markers.len(), 2, "passive error/warn markers are not stored");

    let mut fresh = sample();
    file.apply_markers(&mut fresh);
    assert_eq!(fresh[1].marker, Some(MarkerType::Bookmark));
    assert_eq!(fresh[3].note.as_deref(), Some("investigate"));
    assert_eq!(fresh[3].marker, Some(MarkerType::Note));
    // Untouched rows keep their passive markers.
    assert_eq!(fresh[2].marker, Some(MarkerType::Error));
}

#[test]
fn a_log_space_and_each_file_view_have_their_own_filter() {
    let mut space = LogSpace::new("s".into(), "Payment".into(), LogSpaceKind::Imported);
    space.add_imported_files(&["/a.log".to_string(), "/b.log".to_string()]).unwrap();
    space.filter.toggle_value("level", "ERROR");

    // The file views start without a Filter and do not touch the space's.
    assert!(space.file_filter("/a.log").is_empty());
    let mut view_filter = space.file_filter("/a.log");
    view_filter.toggle_value("module", "API");
    space.set_file_filter("/a.log", view_filter);

    assert!(!space.file_filter("/a.log").is_empty());
    assert!(space.file_filter("/b.log").is_empty());
    assert!(space.filter.column("module").is_none());
    assert!(space.filter.is_value_ticked("level", "error"));
}

#[test]
fn filter_state_serializes_and_old_shapes_default_to_empty() {
    let mut f = FilterState::new();
    f.toggle_value("module", "API");
    f.add_custom("module", "pay");
    f.set_hide("module", true);
    f.set_time_range("10:00", "11:00");
    let json = serde_json::to_string(&f).unwrap();
    let back: FilterState = serde_json::from_str(&json).unwrap();
    assert_eq!(f, back);

    let old: FilterState =
        serde_json::from_str(r#"{"conditions":[{"id":1}],"logic_mode":"All"}"#).unwrap();
    assert!(old.is_empty());
}
