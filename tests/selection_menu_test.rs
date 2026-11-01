//! The Selection Menu (openspec change `sidebar-and-selection-actions`):
//! selecting text opens nothing; a right-click opens one menu to copy, paint,
//! filter and make a rule from what it landed on.

mod common;

use common::Harness;
use gpui::{Modifiers, MouseButton, TestAppContext, point, px};
use log_gpui::app::color::ColorTarget;
use log_gpui::app::selection_menu::{CellValue, MenuSource, SelectionTarget, cell_value};
use log_gpui::filter::FilterState;
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::text_selection::{TextPos, TextSelection};
use log_gpui::ui::cells::TextCell;
use log_gpui::ui::selection_menu::condition_label;
use log_gpui::workspace::log_space::LogSpaceKind;
use log_gpui::workspace::working_set::ViewMode;

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 [INFO] [API] open conn-7f3a".to_string(),
        "2026-10-19 10:00:02 [ERROR] [DB] request timeout".to_string(),
        "2026-10-19 10:00:03 [WARN] [API] slow conn-7f3a".to_string(),
        "2026-10-19 10:00:04 [ERROR] [API] reset by peer".to_string(),
        "2026-10-19 10:00:05 [DEBUG] [DB] pool ok".to_string(),
    ]
}

fn red() -> HighlightColor {
    HighlightColor::palette(Hue::Red, Shade::Medium)
}

fn draw(h: &mut Harness) {
    for i in 0..2 {
        h.cx.simulate_resize(gpui::size(px(1000.0 + i as f32), px(700.0)));
        h.cx.run_until_parked();
    }
}

/// Select `text` in row `row` of the Parsed View.
fn select(h: &mut Harness, row: usize, text: &str) {
    h.view.update(&mut h.cx, |app, _| {
        let line = app.rows_snapshot()[app.visible_indices[row]].raw_line.to_string();
        let start = line.find(text).expect("text on the row");
        app.text_selection = Some(TextSelection::new(TextPos::new(row, start), TextPos::new(row, start + text.len())));
    });
}

fn menu_on_selection(h: &mut Harness, everywhere: Option<&str>, cell: Option<CellValue>) {
    h.view.update(&mut h.cx, |app, _| {
        app.selection_menu = Some(SelectionTarget {
            source: MenuSource::Parsed,
            at: point(px(10.0), px(10.0)),
            text: app.selected_text(),
            mark: Some(ColorTarget::MarkSelection),
            everywhere: everywhere.map(str::to_string),
            marker: None,
            highlight: None,
            rule: cell.is_some(),
            cell,
        })
    });
}

fn visible(h: &Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| app.filter_pass.len())
}

#[gpui::test]
fn a_drag_in_the_parsed_view_selects_and_opens_nothing(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    h.view.update(&mut h.cx, |app, cx| {
        app.selecting = true;
        app.finish_text_selection(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.text_selection.is_some(), "the text stays selected to copy");
        assert!(app.color_bar.is_none());
    });
}

#[gpui::test]
fn mark_here_paints_one_marker_and_highlight_everywhere_one_text_highlight(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    menu_on_selection(&mut h, Some("conn-7f3a"), None);
    h.view.update(&mut h.cx, |app, cx| app.menu_mark_here(red(), cx));
    h.view.read_with(&h.cx, |app, _| {
        let file = &app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().files[0];
        assert_eq!(file.markers.len(), 1);
        assert!(app.text_highlights().is_empty());
        assert!(app.color_bar.is_none(), "no bar is left open");
    });

    select(&mut h, 2, "conn-7f3a");
    menu_on_selection(&mut h, Some("conn-7f3a"), None);
    h.view.update(&mut h.cx, |app, cx| app.menu_highlight_everywhere(red(), cx));
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.text_highlights().iter().map(|t| t.text.as_str()).collect::<Vec<_>>(), ["conn-7f3a"]);
    });
}

#[gpui::test]
fn color_opens_the_bar_for_the_selection(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    menu_on_selection(&mut h, Some("conn-7f3a"), None);
    h.view.update(&mut h.cx, |app, cx| app.menu_open_color_bar(cx));
    let target = h.view.read_with(&h.cx, |app, _| app.color_bar.as_ref().map(|b| b.target.clone()));
    assert_eq!(target, Some(ColorTarget::MarkSelection));
}

#[gpui::test]
fn remove_marker_removes_the_marker_under_the_pointer(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    h.view.update(&mut h.cx, |app, cx| app.mark_selection(red(), cx));
    let byte = h.view.read_with(&h.cx, |app, _| app.rows_snapshot()[app.visible_indices[0]].raw_line.find("conn").unwrap());
    h.view.update(&mut h.cx, |app, _| {
        app.selection_menu = Some(SelectionTarget {
            source: MenuSource::Parsed,
            at: point(px(10.0), px(10.0)),
            text: None,
            mark: None,
            everywhere: None,
            marker: Some(ColorTarget::RecolorMarker { row: 0, byte }),
            highlight: None,
            cell: None,
            rule: false,
        })
    });
    h.view.update(&mut h.cx, |app, cx| app.menu_remove_marker(cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().files[0].markers.is_empty());
    });
}

#[test]
fn the_cell_value_follows_the_selection() {
    let line = "2026-10-19 10:00:02 [ERROR] [DB] request timeout";
    let level = line.find("ERROR").unwrap()..line.find("ERROR").unwrap() + 5;
    let message = line.find("request").unwrap()..line.len();
    // No selection: the whole value.
    let whole = cell_value(TextCell::Level, line, level.clone(), None).unwrap();
    assert_eq!((whole.column.as_str(), whole.value.as_str(), whole.whole), ("level", "ERROR", true));
    // Part of the cell: a Custom Value.
    let part = cell_value(TextCell::Level, line, level.clone(), Some(level.start..level.start + 3)).unwrap();
    assert_eq!((part.value.as_str(), part.whole), ("ERR", false));
    // Leaving the cell: nothing to filter.
    assert_eq!(cell_value(TextCell::Level, line, level.clone(), Some(level.start..message.end)), None);
    // MESSAGE: only a selected piece, never whole.
    assert_eq!(cell_value(TextCell::Message, line, message.clone(), None), None);
    let piece = cell_value(TextCell::Message, line, message.clone(), Some(message.end - 7..message.end)).unwrap();
    assert_eq!((piece.value.as_str(), piece.whole), ("timeout", false));
    // TIMESTAMP has no value to filter on.
    assert_eq!(cell_value(TextCell::Timestamp, line, 0..19, None), None);
}

#[test]
fn labels_name_the_condition() {
    let level = CellValue { column: "level".into(), value: "ERROR".into(), whole: true };
    assert_eq!(condition_label(&level, false), "level = ERROR");
    let message = CellValue { column: "message".into(), value: "timeout".into(), whole: false };
    assert_eq!(condition_label(&message, true), "message ∋ “timeout”");
}

#[test]
fn a_filter_from_a_selection_adds_in_the_same_mode_and_replaces_in_the_other() {
    let mut f = FilterState::new();
    f.apply_from_selection("level", "ERROR", false, false);
    f.apply_from_selection("level", "WARN", false, false);
    assert_eq!(f.column("level").unwrap().values, ["error", "warn"]);
    assert!(!f.column("level").unwrap().hide);
    // Hide replaces the "show only" condition.
    f.apply_from_selection("level", "DEBUG", false, true);
    let c = f.column("level").unwrap();
    assert_eq!((c.values.as_slice(), c.hide), (&["debug".to_string()][..], true));
    // A piece of a value is a Custom Value.
    f.apply_from_selection("message", "timeout", true, false);
    assert_eq!(f.column("message").unwrap().custom, ["timeout"]);
}

#[gpui::test]
fn show_only_and_hide_from_a_cell_filter_the_view(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    assert_eq!(visible(&h), 5);
    h.view.update(&mut h.cx, |app, cx| app.filter_from_selection("level", "ERROR", true, false, cx));
    h.cx.run_until_parked();
    assert_eq!(visible(&h), 2);
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.filter_state.column("level").unwrap().values, ["error"]));
    // Hide on the same column replaces it.
    h.view.update(&mut h.cx, |app, cx| app.filter_from_selection("level", "ERROR", true, true, cx));
    h.cx.run_until_parked();
    assert_eq!(visible(&h), 3);
    // MESSAGE takes a Custom Value even for "whole".
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_clear_all(cx);
        app.filter_from_selection("message", "conn-7f3a", true, false, cx);
    });
    h.cx.run_until_parked();
    assert_eq!(visible(&h), 2);
    h.view.read_with(&h.cx, |app, _| assert_eq!(app.filter_state.column("message").unwrap().custom, ["conn-7f3a"]));
}

#[gpui::test]
fn highlight_rows_where_opens_the_form_filled_in(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    menu_on_selection(&mut h, None, Some(CellValue { column: "module".into(), value: "DB".into(), whole: true }));
    h.view.update(&mut h.cx, |app, cx| app.menu_highlight_rows(cx));
    h.view.read_with(&h.cx, |app, _| {
        let form = app.rule_form.as_ref().expect("the New rule form");
        assert_eq!(form.column.as_deref(), Some("module"));
        assert_eq!(form.values, ["DB"]);
        assert!(app.highlight_rules().is_empty(), "nothing is added before Add");
    });
}

#[gpui::test]
fn the_raw_list_menu_has_no_filter_and_a_drag_opens_no_bar(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let path = h.dir.path().join("a.log");
    std::fs::write(&path, lines().join("\n")).unwrap();
    let source = log_gpui::workspace::log_space::ImportSource::Disk(path.to_string_lossy().into_owned());
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("Raw", LogSpaceKind::Static, &[source], cx));
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.set_view_mode(ViewMode::Raw, cx));
    draw(&mut h);
    let (from, to) = h.view.read_with(&h.cx, |app, _| {
        let b = app.raw.list_bounds.get();
        let y = b.origin.y + px(app.text_scale().row_height() / 2.0);
        (point(b.origin.x + px(80.0), y), point(b.origin.x + px(200.0), y))
    });
    h.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    h.cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.raw.list_selection.is_some_and(|s| !s.is_empty()));
        assert!(app.color_bar.is_none());
    });
    let mid = point((from.x + to.x) / 2.0, from.y);
    h.cx.simulate_mouse_down(mid, MouseButton::Right, Modifiers::none());
    h.cx.run_until_parked();
    let target = h.view.read_with(&h.cx, |app, _| app.selection_menu.clone()).expect("the menu's target");
    assert_eq!(target.source, MenuSource::RawList);
    assert!(target.text.is_some() && matches!(target.mark, Some(ColorTarget::RawMark { .. })));
    assert!(target.cell.is_none(), "no filter in the Raw View");
}

#[gpui::test]
fn a_right_click_in_a_scratch_editor_offers_its_selection(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Notes", LogSpaceKind::Scratch, &[], cx).unwrap();
        app.set_view_mode(ViewMode::Raw, cx);
    });
    draw(&mut h);
    h.cx.simulate_input("error at db");
    draw(&mut h);
    let id = h.view.read_with(&h.cx, |app, _| app.raw.editors.keys().next().unwrap().clone());
    let state = h.view.read_with(&h.cx, |app, _| app.raw.editors[&id].state.clone());
    h.cx.update(|_, cx| state.update(cx, |s, cx| s.set_selected_range(0..5, cx)));
    draw(&mut h);
    h.view.update(&mut h.cx, |app, cx| app.on_editor_right_click(&id, point(px(50.0), px(50.0)), cx));
    let target = h.view.read_with(&h.cx, |app, _| app.selection_menu.clone()).unwrap();
    assert_eq!(target.text.as_deref(), Some("error"));
    assert_eq!(target.everywhere.as_deref(), Some("error"));
    h.view.update(&mut h.cx, |app, cx| app.menu_mark_here(red(), cx));
    h.view.read_with(&h.cx, |app, _| {
        let file = &app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().files[0];
        assert_eq!(file.markers_on_line(1).iter().map(|m| m.range.clone()).collect::<Vec<_>>(), [0..5]);
    });
}
