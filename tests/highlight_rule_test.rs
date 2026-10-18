use log_gpui::highlight::{
    HighlightRule, MESSAGE_COLUMN, RuleError, count_matches, move_rule, next_rule_id, paint_for_row, paint_rows,
};
use log_gpui::model::LogEntry;
use log_gpui::palette::{HighlightColor, Hue, Shade, Tone, Weight, next_rule_color};
use log_gpui::parser::text::parse_text_bytes;
use log_gpui::space_view::{ParsedFile, build_space_view};
use log_gpui::workspace::log_space::{LogSpace, LogSpaceKind};
use log_gpui::workspace::project::{Project, ProjectColorAccent};
use log_gpui::workspace::session::SessionManifest;
use log_gpui::workspace::state::WorkspaceState;
use std::collections::HashMap;

fn color(h: Hue) -> HighlightColor {
    HighlightColor::palette(h, Shade::Medium)
}

fn rule(id: u64, column: &str, values: &[&str], custom: &[&str], c: HighlightColor) -> HighlightRule {
    let v: Vec<String> = values.iter().map(|s| s.to_string()).collect();
    let cu: Vec<String> = custom.iter().map(|s| s.to_string()).collect();
    HighlightRule::new(id, column, &v, &cu, c).unwrap()
}

fn rows() -> Vec<LogEntry> {
    parse_text_bytes(
        b"2026-10-18 10:00:01 [INFO] [DB] query ok\n\
          2026-10-18 10:00:02 [ERROR] [API] timeout calling upstream\n\
          \x20 at Client.call(Client.java:1)\n\
          \x20 at Router.handle(Router.java:2)\n\
          2026-10-18 10:00:03 [INFO] [CACHE] hit\n\
          2026-10-18 10:00:04 [WARN] [DB] slow query: timeout soon\n",
    )
    .unwrap()
}

// ─── A rule ──────────────────────────────────────────────────────────────────

#[test]
fn several_values_of_one_rule_are_ored() {
    let es = rows();
    let r = rule(1, "module", &["DB", "CACHE"], &[], color(Hue::Orange));
    let painted: Vec<bool> = (0..es.len()).map(|i| paint_for_row(&[r.clone()], &es, i).is_some()).collect();
    assert_eq!(painted, vec![true, false, false, false, true, true]);
}

#[test]
fn a_value_is_matched_without_regard_to_case() {
    let es = rows();
    let r = rule(1, "module", &["db"], &[], color(Hue::Red));
    assert!(paint_for_row(&[r], &es, 0).is_some());
}

#[test]
fn message_takes_custom_values_only() {
    let es = rows();
    let by_text = rule(1, MESSAGE_COLUMN, &[], &["timeout"], color(Hue::Red));
    assert!(paint_for_row(&[by_text.clone()], &es, 1).is_some());
    assert!(paint_for_row(&[by_text], &es, 0).is_none());
    assert_eq!(
        HighlightRule::new(2, MESSAGE_COLUMN, &["x".into()], &[], color(Hue::Red)),
        Err(RuleError::MessageNeedsCustomValue)
    );
}

#[test]
fn timestamp_cannot_have_a_rule_and_a_rule_needs_a_condition() {
    assert_eq!(
        HighlightRule::new(1, "timestamp", &["x".into()], &[], color(Hue::Red)),
        Err(RuleError::TimestampColumn)
    );
    assert_eq!(HighlightRule::new(1, "module", &[], &["  ".into()], color(Hue::Red)), Err(RuleError::NoCondition));
}

#[test]
fn a_row_without_a_value_never_matches() {
    let es = parse_text_bytes(b"2026-10-18 10:00:01 no module here\n").unwrap();
    let r = rule(1, "module", &["DB"], &[], color(Hue::Red));
    assert!(paint_for_row(&[r], &es, 0).is_none());
}

// ─── Priority and switches ───────────────────────────────────────────────────

#[test]
fn the_rule_higher_in_the_list_wins() {
    let es = rows();
    let orange = rule(1, "module", &["DB"], &[], color(Hue::Orange));
    let blue = rule(2, MESSAGE_COLUMN, &[], &["timeout"], color(Hue::Blue));
    // Row 5 is a DB row whose message holds `timeout`: both rules match it.
    let mut rules = vec![orange.clone(), blue.clone()];
    assert_eq!(paint_for_row(&rules, &es, 5).unwrap().color, color(Hue::Orange));
    // Dragging the blue rule above the orange one flips the winner.
    move_rule(&mut rules, 1, 0);
    assert_eq!(paint_for_row(&rules, &es, 5).unwrap().color, color(Hue::Blue));
}

#[test]
fn a_switched_off_rule_paints_nothing_but_stays_in_the_list() {
    let es = rows();
    let mut r = rule(1, "module", &["DB"], &[], color(Hue::Orange));
    r.enabled = false;
    let rules = vec![r];
    assert!(paint_rows(&rules, &es).iter().all(Option::is_none));
    assert_eq!(rules.len(), 1);
}

#[test]
fn a_disabled_rule_lets_the_next_one_paint() {
    let es = rows();
    let mut first = rule(1, "module", &["DB"], &[], color(Hue::Orange));
    first.enabled = false;
    let second = rule(2, "level", &["INFO"], &[], color(Hue::Green));
    assert_eq!(paint_for_row(&[first, second], &es, 0).unwrap().color, color(Hue::Green));
}

// ─── Continuation lines ──────────────────────────────────────────────────────

#[test]
fn a_stack_trace_is_painted_like_its_parent_line() {
    let es = rows();
    let r = rule(1, "level", &["ERROR"], &[], color(Hue::Red));
    let paints = paint_rows(&[r], &es);
    assert!(paints[1].is_some());
    assert!(paints[2].is_some(), "stack trace line 1");
    assert!(paints[3].is_some(), "stack trace line 2");
    assert!(paints[4].is_none());
}

#[test]
fn match_counts_skip_continuation_lines() {
    let es = rows();
    let r = rule(1, "level", &["ERROR"], &[], color(Hue::Red));
    assert_eq!(count_matches(&r, &es, 0..es.len()), 1);
}

// ─── Scope ───────────────────────────────────────────────────────────────────

#[test]
fn a_rule_can_use_the_source_column_of_a_log_space() {
    let mut space = LogSpace::new("s".into(), "s".into(), LogSpaceKind::Imported);
    space.add_imported_files(&["/a.log".to_string(), "/worker.log".to_string()]).unwrap();
    let mut parsed = HashMap::new();
    parsed.insert("/a.log".to_string(), ParsedFile::from_text("2026-10-18 10:00:01 [INFO] [A] one\n"));
    parsed.insert("/worker.log".to_string(), ParsedFile::from_text("2026-10-18 10:00:02 [INFO] [W] two\n"));
    let view = build_space_view(&space, &parsed);
    let r = rule(1, "source", &["worker.log"], &[], color(Hue::Purple));
    let paints = paint_rows(&[r], &view.entries);
    assert!(paints[0].is_none());
    assert!(paints[1].is_some());
}

#[test]
fn rules_belong_to_the_project_and_survive_a_restart() {
    let mut ws = WorkspaceState::new();
    ws.create_project("Payment", ProjectColorAccent::Blue);
    let project: &mut Project = ws.active_project_mut().unwrap();
    project.highlight_rules.push(rule(1, "module", &["DB"], &[], color(Hue::Orange)));
    project.recent_colors.push(color(Hue::Orange));

    let json = SessionManifest::from_workspace(&ws).to_json().unwrap();
    let restored = SessionManifest::from_json_current(&json).unwrap().into_workspace();
    let p = restored.active_project().unwrap();
    assert_eq!(p.highlight_rules.len(), 1);
    assert_eq!(p.highlight_rules[0].values, vec!["db".to_string()]);
    assert_eq!(p.recent_colors.colors(), &[color(Hue::Orange)]);
}

#[test]
fn a_version_three_session_still_opens_without_its_old_markers() {
    let json = r#"{"version":3,"active_project_id":"p","projects":[{"id":"p","name":"P","color":"Blue",
        "log_spaces":[{"id":"s","name":"S","kind":"Imported","files":[
            {"file_id":"a","display_name":"a","path":null,"added_seq":1,
             "markers":[{"line":2,"marker":"Bookmark","note":"n","content_hash":1}]}]}],
        "working_set":{"tabs":[],"active_tab":null},"investigation_cases":[]}]}"#;
    let manifest = SessionManifest::from_json_current(json).expect("version 3 is still read");
    let ws = manifest.into_workspace();
    let file = &ws.project("p").unwrap().log_spaces[0].files[0];
    assert!(file.markers.is_empty());
    assert!(ws.project("p").unwrap().highlight_rules.is_empty());
}

#[test]
fn new_rules_take_the_next_colors_and_ids() {
    assert_eq!(next_rule_color(0), color(Hue::Red));
    assert_eq!(next_rule_color(10), color(Hue::Red), "the palette cycles");
    assert_eq!(next_rule_id(&[]), 1);
    assert_eq!(next_rule_id(&[rule(7, "module", &["DB"], &[], color(Hue::Red))]), 8);
}

#[test]
fn a_rule_paints_the_row_lighter_than_the_cell() {
    let c = color(Hue::Orange);
    let row = c.with_weight(Weight::Row, Tone::Light);
    let cell = c.with_weight(Weight::Cell, Tone::Light);
    assert_ne!(row, cell);
    let lum = |rgb: u32| ((rgb >> 16) & 0xFF) + ((rgb >> 8) & 0xFF) + (rgb & 0xFF);
    assert!(lum(row) > lum(cell), "the row is the lighter shade on the light theme");
}
