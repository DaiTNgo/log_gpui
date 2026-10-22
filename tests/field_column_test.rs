//! Field Columns: the fields read from lines, shown as columns of the Parsed
//! View next to the fixed ones.

use log_gpui::filter_index::FilterIndex;
use log_gpui::model::{FieldKey, LogEntry};
use log_gpui::parser::text::parse_text_bytes;

fn rows() -> Vec<LogEntry> {
    parse_text_bytes(
        br#"{"time":"2026-10-18T10:00:01Z","level":"info","msg":"start","region":"us","user":"alice"}
{"time":"2026-10-18T10:00:02Z","level":"error","msg":"boom","region":"eu","user":"bob"}
  at Pool.get(Pool.java:10)
{"time":"2026-10-18T10:00:03Z","level":"info","msg":"ok","user":"carol"}
{"time":"2026-10-18T10:00:04Z","level":"warn","msg":"slow","user":"dave"}
"#,
    )
    .unwrap()
}

// ─── Coverage ────────────────────────────────────────────────────────────────

#[test]
fn coverage_counts_parent_lines_with_a_value() {
    let index = FilterIndex::build(&rows());
    assert_eq!(index.parent_lines(), 4);
    assert_eq!(index.coverage("region"), (2, 4));
    assert_eq!(index.coverage("user"), (4, 4));
    assert_eq!(index.coverage("nope"), (0, 4));
}

#[test]
fn coverage_follows_rows_leaving_and_coming_back() {
    let entries = rows();
    let mut index = FilterIndex::build(&entries);
    index.truncate(&entries[..3], &entries[3..]);
    assert_eq!(index.coverage("region"), (2, 2));
    assert_eq!(index.coverage("user"), (2, 2));
    index.append(&entries[3..]);
    assert_eq!(index.coverage("region"), (2, 4));
}

#[test]
fn coverage_is_kept_when_a_column_overflows() {
    let text: String = (0..600).map(|i| format!("{{\"msg\":\"m\",\"id\":\"v{i}\"}}\n")).collect();
    let entries = parse_text_bytes(text.as_bytes()).unwrap();
    let index = FilterIndex::build(&entries);
    let id = index.column("id").unwrap();
    assert!(id.overflow);
    assert_eq!(index.coverage("id").0, 600);
}

#[test]
fn field_columns_are_the_fields_read_from_lines() {
    let index = FilterIndex::build(&rows());
    let fields: Vec<FieldKey> = index.field_columns().filter_map(|c| c.field()).collect();
    let names: Vec<&str> = fields.iter().map(|k| k.as_str()).collect();
    assert_eq!(names, vec!["msg", "region", "user"]);
}

// ─── Layout ──────────────────────────────────────────────────────────────────

use gpui::px;
use log_gpui::column::{ColumnLayoutState, FieldMaxChars, PinnableColumn};
use log_gpui::density::Density;
use log_gpui::ui::cells::{TextCell, cell_text, shows_placeholder};
use std::collections::BTreeSet;

fn field(name: &str) -> PinnableColumn {
    PinnableColumn::Field(FieldKey::new(name))
}

fn layout_with(fields: &[&str]) -> ColumnLayoutState {
    let mut layout = ColumnLayoutState::new();
    layout.max_chars = FieldMaxChars::scan_entries(&rows());
    layout.set_view_columns(BTreeSet::new(), fields.iter().map(|f| FieldKey::new(f)).collect(), Density::default());
    layout
}

#[test]
fn field_columns_come_after_the_fixed_ones_in_their_order() {
    let layout = layout_with(&["user", "region"]);
    assert_eq!(
        layout.columns(),
        vec![PinnableColumn::Timestamp, PinnableColumn::Level, PinnableColumn::Module, field("user"), field("region")]
    );
    let message = layout.natural_message_left();
    let region = layout.natural_column_left(field("region"));
    assert_eq!(region + layout.width(field("region")), message);
}

#[test]
fn pinned_columns_keep_the_view_order() {
    let mut layout = layout_with(&["user"]);
    layout.toggle_pin(field("user"));
    layout.toggle_pin(PinnableColumn::Level);
    assert_eq!(layout.pinned_columns(), &[PinnableColumn::Level, field("user")]);
}

#[test]
fn a_pinned_field_column_left_out_of_the_view_keeps_its_pin() {
    let mut layout = layout_with(&["user"]);
    layout.toggle_pin(field("user"));
    layout.set_view_columns(BTreeSet::new(), vec![], Density::default());
    assert!(layout.pinned_columns().is_empty());
    assert!(layout.is_pinned(field("user")));
    layout.set_view_columns(BTreeSet::new(), vec![FieldKey::new("user")], Density::default());
    assert_eq!(layout.pinned_columns(), &[field("user")]);
}

#[test]
fn hidden_fixed_columns_are_not_laid_out() {
    let mut layout = layout_with(&[]);
    layout.set_view_columns(BTreeSet::from([PinnableColumn::Level]), vec![], Density::default());
    assert_eq!(layout.columns(), vec![PinnableColumn::Timestamp, PinnableColumn::Module]);
}

#[test]
fn a_field_column_fits_its_longest_value() {
    let layout = layout_with(&["user"]);
    assert_eq!(layout.max_chars.of(field("user")), 5);
    let expected = log_gpui::column::compute_column_width(field("user"), 5, Density::default().font_size(), px(48.0));
    assert_eq!(layout.width(field("user")), expected);
}

#[test]
fn layout_round_trips_through_a_session_file() {
    let mut layout = layout_with(&["user"]);
    layout.toggle_pin(field("user"));
    layout.set_width(field("user"), px(200.0));
    let json = serde_json::to_string(&layout).unwrap();
    assert!(json.contains("field:user"));
    let back: ColumnLayoutState = serde_json::from_str(&json).unwrap();
    assert!(back.is_pinned(field("user")));
    assert_eq!(back.width(field("user")), px(200.0));
}

#[test]
fn an_old_session_file_still_opens() {
    let old = r#"{"pinned_columns":["Level"],"ordered_pinned":["Level"],"widths":{"Timestamp":120.0},"min_widths":{},
        "max_chars":{"timestamp":1,"level":1,"module":1},"gutter_width":88.0,"is_resizing":null,"resize_start_x":null,
        "hovered_header":null,"hovered_toggle":null}"#;
    let layout: ColumnLayoutState = serde_json::from_str(old).unwrap();
    assert!(layout.is_pinned(PinnableColumn::Level));
}

// ─── Cells ───────────────────────────────────────────────────────────────────

#[test]
fn a_field_cell_is_the_text_of_the_line() {
    let es = parse_text_bytes(br#"{"msg":"m","note":"say \"hi\"","n":42}"#).unwrap();
    let (text, span) = cell_text(&es[0], TextCell::Field(FieldKey::new("note"))).unwrap();
    assert_eq!(text, r#"say \"hi\""#);
    assert_eq!(&es[0].raw_line[span.unwrap()], text);
    assert_eq!(cell_text(&es[0], TextCell::Field(FieldKey::new("n"))).unwrap().0, "42");
}

#[test]
fn a_line_without_the_field_has_an_empty_cell() {
    let es = rows();
    assert!(cell_text(&es[3], TextCell::Field(FieldKey::new("region"))).is_none());
    assert!(!shows_placeholder(&es[3], TextCell::Field(FieldKey::new("region"))));
}

#[test]
fn a_continuation_line_shows_the_placeholder() {
    let es = rows();
    assert!(es[2].is_continuation());
    assert!(shows_placeholder(&es[2], TextCell::Field(FieldKey::new("user"))));
}

// ─── Columns section ─────────────────────────────────────────────────────────

use log_gpui::columns::{ColumnSettings, coverage_label, coverage_tooltip};

#[test]
fn settings_of_an_old_project_are_empty() {
    let s: ColumnSettings = serde_json::from_str("{}").unwrap();
    assert_eq!(s, ColumnSettings::default());
}

#[test]
fn coverage_rounds_down_and_hides_at_full() {
    assert_eq!(coverage_label(1204, 1530).as_deref(), Some("78%"));
    assert_eq!(coverage_label(1529, 1530).as_deref(), Some("99%"));
    assert_eq!(coverage_label(1530, 1530), None);
    assert_eq!(coverage_tooltip(1204, 1530), "1,204 of 1,530 lines");
}

// ─── Matches in hidden columns ───────────────────────────────────────────────

use log_gpui::ui::cells::{hidden_matches, hidden_matches_tooltip};

fn ranges(line: &str, word: &str) -> Vec<(usize, usize)> {
    line.match_indices(word).map(|(s, w)| (s, s + w.len())).collect()
}

#[test]
fn a_match_in_a_shown_cell_needs_no_mark() {
    let es = rows();
    let layout = layout_with(&["user"]);
    assert!(hidden_matches(&es[0], &ranges(&es[0].raw_line, "alice"), &layout).is_none());
    assert!(hidden_matches(&es[0], &[], &layout).is_none());
}

#[test]
fn a_match_only_in_hidden_columns_names_them() {
    let es = rows();
    let mut layout = layout_with(&["user"]);
    layout.set_view_columns(BTreeSet::from([PinnableColumn::Level]), vec![], Density::default());
    let hidden = hidden_matches(&es[1], &ranges(&es[1].raw_line, "error"), &layout).unwrap();
    assert_eq!(hidden, vec![PinnableColumn::Level]);
    let hidden = hidden_matches(&es[0], &ranges(&es[0].raw_line, "alice"), &layout).unwrap();
    assert_eq!(hidden, vec![field("user")]);
    assert_eq!(hidden_matches_tooltip(&hidden, &layout), "Matches in hidden column: user");
}

#[test]
fn a_match_in_text_no_column_shows_is_said_so() {
    let es = rows();
    let layout = layout_with(&[]);
    let hidden = hidden_matches(&es[0], &ranges(&es[0].raw_line, "\"msg\""), &layout).unwrap();
    assert!(hidden.is_empty());
    assert_eq!(hidden_matches_tooltip(&hidden, &layout), "Matches only text no column shows");
}

#[test]
fn saved_column_labels_and_pane_order_are_ignored() {
    let s: ColumnSettings = serde_json::from_str(r#"{"labels":{"module":"Service"},"fields":["user"],"pane":{"user":true}}"#).unwrap();
    assert_eq!(s.pane_choice("user"), Some(true));
    assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"pane":{"user":true}}"#);
}
