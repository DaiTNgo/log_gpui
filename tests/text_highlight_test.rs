//! Text Highlights from the color bar (openspec change
//! `text-highlight-and-value-colors`): one bar, two scopes; "Everywhere" or
//! Alt+click paints every place of the selected text, and it is recolored and
//! removed apart from the Markers on it.

mod common;

use gpui::{TestAppContext, point, px};
use log_gpui::app::color::{ColorTarget, MarkScope};
use log_gpui::palette::{HighlightColor, Hue, Shade};
use log_gpui::text_selection::{TextPos, TextSelection};

fn lines() -> Vec<String> {
    vec![
        "2026-10-19 10:00:01 INFO open conn-7f3a host=db-1".to_string(),
        "2026-10-19 10:00:02 INFO GET /orders conn-91bc".to_string(),
        "2026-10-19 10:00:03 WARN CONN-7F3A slow 820ms".to_string(),
        "2026-10-19 10:00:04 INFO GET /users conn-91bc".to_string(),
        "2026-10-19 10:00:05 ERROR conn-7f3a reset by peer".to_string(),
    ]
}

fn color(hue: Hue) -> HighlightColor {
    HighlightColor::palette(hue, Shade::Medium)
}

/// Select `text` in row `row` of the Parsed View and open the bar there.
fn select(h: &mut common::Harness, row: usize, text: &str) {
    h.view.update(&mut h.cx, |app, cx| {
        let line = app.rows_snapshot()[app.visible_indices[row]].raw_line.to_string();
        let start = line.find(text).expect("text on the row");
        app.text_selection = Some(TextSelection::new(TextPos::new(row, start), TextPos::new(row, start + text.len())));
        app.open_mark_bar(point(px(10.0), px(10.0)), cx);
    });
}

fn highlights(h: &common::Harness) -> Vec<(String, HighlightColor)> {
    h.view.read_with(&h.cx, |app, _| app.text_highlights().iter().map(|t| (t.text.clone(), t.color.clone())).collect())
}

fn markers(h: &common::Harness) -> usize {
    h.view.read_with(&h.cx, |app, _| {
        app.workspace_state.active_project().unwrap().log_spaces.last().unwrap().files[0].markers.len()
    })
}

/// Bytes of `text` in row `row`.
fn byte_of(h: &common::Harness, row: usize, text: &str) -> usize {
    h.view.read_with(&h.cx, |app, _| {
        let line = app.rows_snapshot()[app.visible_indices[row]].raw_line.to_string();
        line.find(text).unwrap()
    })
}

#[gpui::test]
fn the_bar_opens_on_this_line_only_and_paints_a_marker(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    let (scope, everywhere) = h.view.read_with(&h.cx, |app, _| {
        (app.color_bar.as_ref().unwrap().scope, app.bar_everywhere_text())
    });
    assert_eq!(scope, MarkScope::Line);
    assert_eq!(everywhere.as_deref(), Some("conn-7f3a"));
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(color(Hue::Amber), false, cx));
    assert_eq!(markers(&h), 1);
    assert!(highlights(&h).is_empty());
}

#[gpui::test]
fn everywhere_makes_one_text_highlight_and_no_marker(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    h.view.update(&mut h.cx, |app, cx| {
        app.set_bar_scope(MarkScope::Everywhere, cx);
        app.apply_bar_color(color(Hue::Purple), false, cx);
    });
    assert_eq!(highlights(&h), [("conn-7f3a".to_string(), color(Hue::Purple))]);
    assert_eq!(markers(&h), 0);
    // Every place, in another case too, paints; another id does not.
    let b2 = byte_of(&h, 2, "CONN-7F3A");
    let b1 = byte_of(&h, 1, "conn-91bc");
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.text_highlight_under(2, b2).as_deref(), Some("conn-7f3a"));
        assert_eq!(app.text_highlight_under(1, b1), None);
    });
    // The next bar opens on "This line only" again.
    select(&mut h, 1, "conn-91bc");
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.color_bar.as_ref().unwrap().scope), MarkScope::Line);
}

#[gpui::test]
fn alt_click_highlights_everywhere_whatever_the_scope(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 1, "conn-91bc");
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(color(Hue::Orange), true, cx));
    assert_eq!(highlights(&h).len(), 1);
    assert_eq!(markers(&h), 0);
    // The same text in another case recolors it instead of making a second.
    select(&mut h, 3, "conn-91bc");
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(color(Hue::Blue), true, cx));
    assert_eq!(highlights(&h), [("conn-91bc".to_string(), color(Hue::Blue))]);
    // Recent remembers the color.
    let recent = h.view.read_with(&h.cx, |app, _| app.bar_colors()[0].clone());
    assert_eq!(recent, color(Hue::Blue));
}

#[gpui::test]
fn a_selection_over_two_lines_offers_no_everywhere(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| {
        app.text_selection = Some(TextSelection::new(TextPos::new(0, 25), TextPos::new(1, 10)));
        app.open_mark_bar(point(px(10.0), px(10.0)), cx);
    });
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.bar_everywhere_text()), None);
    // Alt+click then paints Markers, as a plain click does.
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(color(Hue::Red), true, cx));
    assert_eq!(markers(&h), 2);
    assert!(highlights(&h).is_empty());
}

#[gpui::test]
fn the_bar_counts_the_lines_holding_the_text(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 0, "conn-7f3a");
    for _ in 0..3 {
        h.cx.simulate_resize(gpui::size(px(1200.0), px(800.0)));
        h.cx.run_until_parked();
    }
    let count = h.view.read_with(&h.cx, |app, _| app.color_bar.as_ref().and_then(|b| b.count));
    assert_eq!(count, Some(3));
}

#[gpui::test]
fn recolor_and_remove_a_text_highlight_apart_from_the_marker_on_it(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    select(&mut h, 4, "conn-7f3a");
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(color(Hue::Purple), true, cx));
    select(&mut h, 4, "conn-7f3a");
    h.view.update(&mut h.cx, |app, cx| app.apply_bar_color(color(Hue::Amber), false, cx));
    assert_eq!(markers(&h), 1);
    let byte = byte_of(&h, 4, "conn-7f3a");

    // Removing the Marker leaves the Text Highlight under it.
    h.view.update(&mut h.cx, |app, cx| app.remove_marker_under(4, byte, cx));
    assert_eq!(markers(&h), 0);
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.text_highlight_under(4, byte)).as_deref(), Some("conn-7f3a"));

    // Change highlight color, from the bar the menu opens.
    h.view.update(&mut h.cx, |app, cx| {
        app.open_text_highlight_bar("conn-7f3a".to_string(), point(px(10.0), px(10.0)), cx);
        assert!(matches!(app.color_bar.as_ref().map(|b| &b.target), Some(ColorTarget::RecolorTextHighlight { .. })));
        app.apply_bar_color(color(Hue::Red), false, cx);
    });
    assert_eq!(highlights(&h), [("conn-7f3a".to_string(), color(Hue::Red))]);

    // Remove highlight: every place goes.
    h.view.update(&mut h.cx, |app, cx| app.remove_text_highlight("conn-7f3a", cx));
    assert!(highlights(&h).is_empty());
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.text_highlight_under(0, byte_of(&h, 0, "conn-7f3a"))), None);
}

fn draw(h: &mut common::Harness) {
    for i in 0..3 {
        h.cx.simulate_resize(gpui::size(px(1200.0 + i as f32), px(800.0)));
        h.cx.run_until_parked();
    }
}

#[gpui::test]
fn value_colors_go_to_identifier_columns_and_turn_off_app_wide(cx: &mut TestAppContext) {
    let lines: Vec<String> = (0..40)
        .map(|i| {
            format!(
                r#"{{"time":"2026-10-19T10:00:{:02}Z","level":"info","thread":"T-{}","duration":{},"ok":true,"msg":"step {i}"}}"#,
                i % 60,
                i % 3,
                i * 7
            )
        })
        .collect();
    let mut h = common::open_app_with_lines(cx, &lines);
    draw(&mut h);
    let (fields, laid_out) = h.view.read_with(&h.cx, |app, _| {
        let vc = app.value_colors.clone().expect("on by default");
        (vc.fields.clone(), app.column_layout.fields.clone())
    });
    let key = |k: &str| log_gpui::model::FieldKey::new(k);
    assert!(laid_out.contains(&key("thread")), "JSON fields are laid out: {laid_out:?}");
    assert!(fields.contains(&key("thread")), "{fields:?}");
    for plain in ["duration", "ok", "time", "level", "msg"] {
        assert!(!fields.contains(&key(plain)), "{plain} gets no Value Color");
    }

    h.view.update(&mut h.cx, |app, cx| app.set_show_value_colors(false, cx));
    draw(&mut h);
    assert!(h.view.read_with(&h.cx, |app, _| app.value_colors.is_none()));
}

#[gpui::test]
fn text_highlights_are_ready_to_paint_and_keep_value_colors_away(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.view.update(&mut h.cx, |app, cx| app.add_text_highlight("conn-7f3a", color(Hue::Purple), cx));
    draw(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.th_matcher.ranges("x CONN-7F3A y").len(), 1);
        let avoid = &app.value_colors.as_ref().unwrap().avoid;
        assert_eq!(avoid.len(), 1, "the Text Highlight's hue is kept away from");
    });
}
