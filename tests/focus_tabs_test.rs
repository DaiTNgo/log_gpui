use log_gpui::focus::{
    FocusFind, FocusRow, FocusTabs, LineRef, compute_focus_rows, nearest_row_before,
};
use log_gpui::model::{LogEntry, LogLevel};
use log_gpui::search_expr::{Matcher, parse};

fn entry(i: usize, raw: &str) -> LogEntry {
    LogEntry::new(i, i as f64, format!("{i}"), "API".into(), LogLevel::Info, raw.into(), raw.into())
}

fn matcher(q: &str) -> Matcher {
    Matcher::compile(&parse(q).unwrap().unwrap())
}

fn entries() -> Vec<LogEntry> {
    vec![
        entry(1, "ERROR db timeout"),
        entry(2, "DEBUG db timeout retry"),
        entry(3, "ERROR api ok"),
        entry(4, "INFO db timeout"),
        entry(5, "ERROR db timeout user=42"),
    ]
}

fn rows(positions: &[usize]) -> Vec<FocusRow> {
    positions.iter().map(|&p| FocusRow::Entry(p)).collect()
}

// ─── Focus Tabs ──────────────────────────────────────────────────────────────

#[test]
fn default_is_one_empty_tab_numbered_1() {
    let tabs = FocusTabs::default();
    assert_eq!(tabs.tabs.len(), 1);
    assert_eq!(tabs.active_tab().number, 1);
    assert!(tabs.search().query.is_empty());
    assert!(!tabs.can_close());
}

#[test]
fn add_takes_largest_number_plus_one_and_opens_it() {
    let mut tabs = FocusTabs::default();
    tabs.add(); // 2
    tabs.add(); // 3
    tabs.close(1); // closes 2 -> tabs 1 and 3
    let numbers: Vec<u32> = tabs.tabs.iter().map(|t| t.number).collect();
    assert_eq!(numbers, vec![1, 3]);
    let i = tabs.add();
    assert_eq!(tabs.tabs[i].number, 4);
    assert_eq!(tabs.active_tab().number, 4);
    assert!(tabs.active_tab().search.query.is_empty());
}

#[test]
fn closing_a_middle_tab_keeps_the_other_numbers() {
    let mut tabs = FocusTabs::default();
    tabs.add();
    tabs.add();
    assert!(tabs.close(1));
    let numbers: Vec<u32> = tabs.tabs.iter().map(|t| t.number).collect();
    assert_eq!(numbers, vec![1, 3]);
}

#[test]
fn the_last_tab_cannot_be_closed() {
    let mut tabs = FocusTabs::default();
    assert!(!tabs.close(0));
    assert_eq!(tabs.tabs.len(), 1);
}

#[test]
fn closing_keeps_a_valid_open_tab() {
    let mut tabs = FocusTabs::default();
    tabs.add();
    tabs.add(); // open: index 2
    tabs.close(2);
    assert_eq!(tabs.active, 1);
    tabs.set_active(0);
    tabs.close(1);
    assert_eq!(tabs.active, 0);
    tabs.add();
    tabs.set_active(1);
    tabs.close(0);
    assert_eq!(tabs.active, 0, "the open tab stays the same one");
}

#[test]
fn label_is_number_and_start_of_expression() {
    let mut tabs = FocusTabs::default();
    assert_eq!(tabs.active_tab().label(), "1");
    tabs.search_mut().query = "timeout".into();
    assert_eq!(tabs.active_tab().label(), "1 timeout");
    tabs.search_mut().query = "a very long search expression".into();
    assert!(tabs.active_tab().label().ends_with('…'));
}

#[test]
fn tabs_round_trip_through_json() {
    let mut tabs = FocusTabs::default();
    tabs.search_mut().query = "timeout".into();
    tabs.find_mut().query = "user=42".into();
    tabs.active_tab_mut().current = Some(LineRef::new("api", 7));
    tabs.add();
    tabs.add();
    tabs.search_mut().query = "refused".into();
    tabs.close(1);
    let json = serde_json::to_string(&tabs).unwrap();
    let back: FocusTabs = serde_json::from_str(&json).unwrap();
    let numbers: Vec<u32> = back.tabs.iter().map(|t| t.number).collect();
    assert_eq!(numbers, vec![1, 3]);
    assert_eq!(back.active, 1);
    assert_eq!(back.tabs[0].search.query, "timeout");
    assert_eq!(back.tabs[0].find.query, "user=42");
    assert_eq!(back.tabs[0].current, Some(LineRef::new("api", 7)));
    assert_eq!(back.tabs[1].search.query, "refused");
    assert!(back.tabs[1].search.matcher.is_some(), "matcher is compiled again");
}

#[test]
fn stored_tabs_without_any_tab_open_as_one_empty_tab() {
    let back: FocusTabs = serde_json::from_str("{}").unwrap();
    assert_eq!(back.tabs.len(), 1);
    assert_eq!(back.tabs[0].number, 1);
}

// ─── Focus Find ──────────────────────────────────────────────────────────────

fn lines() -> Vec<&'static str> {
    vec!["a user=42", "b", "c user=43", "d user=42 x", "e"]
}

#[test]
fn find_computes_hits_without_narrowing() {
    let mut f = FocusFind::default();
    f.set_query("user=42", lines().into_iter());
    assert_eq!(f.hits, vec![0, 3]);
    assert_eq!(f.display(), (1, 2));
}

#[test]
fn find_uses_search_expression_syntax() {
    let mut f = FocusFind::default();
    f.set_query("user=42 || user=43", lines().into_iter());
    assert_eq!(f.hits, vec![0, 2, 3]);
}

#[test]
fn find_next_prev_wrap() {
    let mut f = FocusFind::default();
    f.set_query("user", lines().into_iter());
    assert_eq!(f.hits, vec![0, 2, 3]);
    assert_eq!(f.next_hit(), Some(2));
    assert_eq!(f.next_hit(), Some(3));
    assert_eq!(f.next_hit(), Some(0));
    assert_eq!(f.prev_hit(), Some(3));
    assert_eq!(f.display(), (3, 3));
}

#[test]
fn find_keeps_last_valid_expression_while_typing() {
    let mut f = FocusFind::default();
    f.set_query("user=42", lines().into_iter());
    f.set_query("user=42 ||", lines().into_iter());
    assert_eq!(f.hits, vec![0, 3]);
    assert!(f.parse_error.is_some());
    f.set_query("user=42 || user=43", lines().into_iter());
    assert!(f.parse_error.is_none());
    assert_eq!(f.hits, vec![0, 2, 3]);
}

#[test]
fn find_with_no_hits_has_nothing_to_jump_to() {
    let mut f = FocusFind::default();
    f.set_query("zzz", lines().into_iter());
    assert_eq!(f.display(), (0, 0));
    assert_eq!(f.next_hit(), None);
    assert_eq!(f.prev_hit(), None);
    f.set_query("", lines().into_iter());
    assert!(f.hits.is_empty());
}

#[test]
fn recomputing_hits_keeps_the_selected_one() {
    let mut f = FocusFind::default();
    f.set_query("user", lines().into_iter());
    f.next_hit();
    f.compute_hits(lines().into_iter(), false);
    assert_eq!(f.current, 1);
    f.select_row(3);
    assert_eq!(f.current, 2);
}

// ─── Rows of a tab ───────────────────────────────────────────────────────────

#[test]
fn filter_and_search_both_apply() {
    let e = entries();
    let errors = vec![0, 2, 4];
    let got = compute_focus_rows(&e, &errors, true, Some(&matcher("timeout")));
    assert_eq!(got, rows(&[0, 4]));
}

#[test]
fn filter_alone_lists_the_lines_that_pass() {
    let e = entries();
    let got = compute_focus_rows(&e, &[0, 3], true, None);
    assert_eq!(got, rows(&[0, 3]));
}

#[test]
fn search_alone_lists_the_matches() {
    let e = entries();
    let got = compute_focus_rows(&e, &[0, 1, 2, 3, 4], false, Some(&matcher("timeout")));
    assert_eq!(got, rows(&[0, 1, 3, 4]));
}

#[test]
fn neither_filter_nor_search_lists_nothing() {
    let e = entries();
    assert!(compute_focus_rows(&e, &[0, 1, 2, 3, 4], false, None).is_empty());
}

#[test]
fn a_continuation_line_is_listed_only_when_it_matches() {
    let mut e = vec![
        entry(1, "ERROR failed"),
        entry(2, "\tat Foo.bar(Foo.java:1)"),
        entry(3, "\tat java.lang.NullPointerException"),
        entry(4, "INFO done"),
    ];
    e[1].parent = Some(1);
    e[2].parent = Some(1);
    let got = compute_focus_rows(&e, &[0, 1, 2, 3], false, Some(&matcher("NullPointerException")));
    assert_eq!(got, rows(&[2]));
}

// ─── Nearest row ─────────────────────────────────────────────────────────────

#[test]
fn nearest_row_before_finds_the_row_at_or_before() {
    let r = rows(&[100, 300]);
    assert_eq!(nearest_row_before(&r, FocusRow::Entry(250)), Some(0));
    assert_eq!(nearest_row_before(&r, FocusRow::Entry(300)), Some(1));
    assert_eq!(nearest_row_before(&r, FocusRow::Entry(999)), Some(1));
    assert_eq!(nearest_row_before(&r, FocusRow::Entry(50)), None);
    assert_eq!(nearest_row_before(&[], FocusRow::Entry(5)), None);
}

#[test]
fn raw_rows_order_by_section_then_line() {
    let r = vec![
        FocusRow::Raw { section: 0, line: 4 },
        FocusRow::Raw { section: 1, line: 1 },
    ];
    assert_eq!(nearest_row_before(&r, FocusRow::Raw { section: 0, line: 9 }), Some(0));
    assert_eq!(nearest_row_before(&r, FocusRow::Raw { section: 1, line: 0 }), Some(0));
    assert_eq!(nearest_row_before(&r, FocusRow::Raw { section: 1, line: 2 }), Some(1));
}
