//! The Time Histogram of a view: counted on a worker thread from the rows'
//! times and levels, and a drag or a click on it goes through the Time
//! Range and Go to as typing would (openspec change `add-time-histogram`).
mod common;

use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::app::time_histogram::HistogramDrag;
use log_gpui::focus::FocusRow;

/// 2026-10-19 10:00:00 UTC.
const TEN: f64 = 1_792_404_000.0;

/// An hour of lines, one a minute: every fourth an ERROR, with a stack line
/// under each ERROR.
fn lines() -> Vec<String> {
    let mut out = Vec::new();
    for m in 0..60 {
        let level = if m % 4 == 1 { "ERROR" } else { "INFO" };
        out.push(format!("2026-10-19 10:{m:02}:00 {level} request {m}"));
        if level == "ERROR" {
            out.push("    at service.call(Service.java:42)".to_string());
        }
    }
    out
}

fn totals(app: &AppView) -> [u32; 3] {
    app.histogram.shown.counts.iter().fold([0; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]])
}

#[gpui::test]
fn counts_parent_lines_by_level_on_the_views_span(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.histogram_shown());
        let axis = app.histogram.axis.as_ref().expect("an axis");
        assert_eq!(axis.full, TEN..TEN + 59.0 * 60.0);
        // Stack lines are not counted: 15 ERROR lines, 45 others.
        assert_eq!(totals(app), [45, 0, 15]);
        assert_eq!(app.histogram.shown.untimed, 0);
        assert!(app.histogram.shown.filtered.is_none());
    });

    // Filter: ERROR only. The columns keep every line, the solid part the ERRORs.
    h.view.update(&mut h.cx, |app, cx| app.filter_toggle_value("level", "error", cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let filtered = app.histogram.shown.filtered.as_ref().expect("Filter counts");
        let sum = filtered.iter().fold([0; 3], |a, c| [a[0] + c[0], a[1] + c[1], a[2] + c[2]]);
        assert_eq!(sum, [0, 0, 15]);
        assert_eq!(totals(app), [45, 0, 15]);
    });
}

#[gpui::test]
fn a_drag_sets_the_time_range_and_a_click_jumps(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.cx.run_until_parked();
    let at = |m: f64| TEN + m * 60.0;

    // Drag from 10:30 to 10:35.
    h.view.update(&mut h.cx, |app, cx| {
        app.histogram_mouse_down(at(30.0), 100.0, 1.0, cx);
        app.histogram_mouse_drag(at(35.0), 200.0, cx);
        assert!(matches!(app.histogram.drag, Some(HistogramDrag::Select { .. })));
        // Nothing narrows while dragging.
        assert!(!app.filter_state.time_range.is_active());
        app.histogram_mouse_up(cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let range = &app.filter_state.time_range;
        assert_eq!((range.from.as_str(), range.to.as_str()), ("10:30:00", "10:35:00"));
        let pane: Vec<usize> = app.focus_rows.iter().filter_map(|r| if let FocusRow::Entry(ix) = r { Some(*ix) } else { None }).collect();
        let times: Vec<f64> = pane.iter().map(|&r| app.index().effective_timestamp(r).unwrap()).collect();
        assert!(!times.is_empty() && times.iter().all(|&t| t >= at(30.0) && t <= at(35.0)), "{times:?}");
        assert_eq!(app.histogram_time_range(), Some((Some(at(30.0)), Some(at(35.0)))));
    });

    // Drag the right edge to 10:40.
    h.view.update(&mut h.cx, |app, cx| {
        app.histogram_mouse_down(at(35.0), 200.0, 1.0, cx);
        assert!(matches!(app.histogram.drag, Some(HistogramDrag::Edge { .. })));
        app.histogram_mouse_drag(at(40.0), 260.0, cx);
        app.histogram_mouse_up(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.filter_state.time_range.to, "10:40:00");
        assert_eq!(app.filter_state.time_range.from, "10:30:00");
    });

    // Typing in To moves the band.
    h.view.update(&mut h.cx, |app, cx| {
        app.filter_state.set_time_range("10:30:00", "10:45:00");
        app.apply_filter_change(cx);
        assert_eq!(app.histogram_time_range(), Some((Some(at(30.0)), Some(at(45.0)))));
    });

    // A click inside the range jumps to its column, the Filter kept.
    let pass_before = h.view.read_with(&h.cx, |app, _| app.filter_pass.clone());
    h.view.update(&mut h.cx, |app, cx| {
        // In the column of 10:33:00, whatever its width.
        app.histogram_mouse_down(at(33.0) + 0.5, 150.0, 1.0, cx);
        app.histogram_mouse_up(cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let row = app.selected_row_pos.expect("a selected row");
        assert_eq!(app.index().effective_timestamp(row), Some(at(33.0)));
        assert_eq!(app.filter_pass, pass_before);
        assert!(app.filter_state.time_range.is_active());
    });

    // A click outside the range clears it.
    h.view.update(&mut h.cx, |app, cx| {
        app.histogram_mouse_down(at(5.0), 20.0, 1.0, cx);
        app.histogram_mouse_up(cx);
    });
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.filter_state.time_range.is_active());
        assert_eq!(app.histogram_time_range(), None);
    });
}

#[gpui::test]
fn zoom_changes_the_axis_only(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &lines());
    h.cx.run_until_parked();
    let (pass, focus) = h.view.read_with(&h.cx, |app, _| (app.filter_pass.clone(), app.focus_rows.clone()));
    h.view.update(&mut h.cx, |app, cx| app.histogram_set_visible(TEN + 20.0 * 60.0..TEN + 50.0 * 60.0, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let axis = app.histogram.axis.as_ref().unwrap();
        assert_eq!(axis.visible, TEN + 20.0 * 60.0..TEN + 50.0 * 60.0);
        assert!(!axis.shows_all());
        // Only the lines from 10:20 to 10:50 are on the columns now.
        assert_eq!(totals(app).iter().sum::<u32>(), 31);
        assert_eq!(app.filter_pass, pass);
        assert_eq!(app.focus_rows, focus);
    });
    h.view.update(&mut h.cx, |app, cx| app.histogram_show_all(cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(app.histogram.axis.as_ref().unwrap().shows_all()));
}

#[gpui::test]
fn no_histogram_without_times_or_when_turned_off(cx: &mut TestAppContext) {
    let untimed: Vec<String> = (0..20).map(|i| format!("plain line {i}")).collect();
    let h = common::open_app_with_lines(cx, &untimed);
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(!app.histogram_shown()));

    let mut h = common::open_app_with_lines(cx, &lines());
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.set_show_histogram(false, cx));
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.histogram_shown());
        assert!(!app.workspace_state.show_histogram);
    });
}

#[gpui::test]
fn live_lines_are_added_to_the_columns(cx: &mut TestAppContext) {
    use common::fake_live::{lines, open};
    let mut f = open(cx, &[("alpha", 0)], 200, 1_000_000);
    f.h.cx.run_until_parked();
    let sum = |f: &common::fake_live::Files| f.h.view.read_with(&f.h.cx, |app, _| totals(app).iter().sum::<u32>());
    assert_eq!(sum(&f), 200);

    // Past the end of the axis: it grows, with room for the next lines.
    f.send("alpha", lines("alpha", 200..210, 0));
    f.pump();
    f.h.cx.run_until_parked();
    let full = f.h.view.read_with(&f.h.cx, |app, _| app.histogram.axis.as_ref().unwrap().full.clone());
    assert_eq!(sum(&f), 210);

    // Within that room: added to the columns, at most twice a second.
    f.send("alpha", lines("alpha", 210..220, 0));
    f.pump();
    f.h.cx.executor().advance_clock(std::time::Duration::from_millis(600));
    f.h.cx.run_until_parked();
    f.h.view.read_with(&f.h.cx, |app, _| assert_eq!(app.histogram.axis.as_ref().unwrap().full, full, "the axis stayed"));
    assert_eq!(sum(&f), 220);
    let levels = f.h.view.read_with(&f.h.cx, |app, _| totals(app));
    // ERROR every fifth line, WARN every fifth.
    assert_eq!((levels[1], levels[2]), (44, 44));
}

#[gpui::test]
fn lines_of_one_instant_get_a_thin_column(cx: &mut TestAppContext) {
    let same: Vec<String> =
        ["INFO", "DEBUG", "INFO", "WARN", "ERROR"].iter().map(|l| format!("2026-07-08 11:04:07.312 {l} line")).collect();
    let h = common::open_app_with_lines(cx, &same);
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let axis = app.histogram.axis.as_ref().expect("an axis");
        assert_eq!(axis.full.end - axis.full.start, 60.0, "a minute around the instant");
        let counts = &app.histogram.shown.counts;
        assert!(counts.len() > 1);
        assert_eq!(counts.iter().filter(|c| c.iter().sum::<u32>() > 0).count(), 1, "one column holds them all");
        assert_eq!(totals(app).iter().sum::<u32>(), 5);
    });
}
