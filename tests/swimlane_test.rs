//! Swimlanes under the Time Histogram in the real app (openspec change
//! `add-swimlane`): what they split by, kept with the view, and what a
//! click on a lane does.
mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::app::AppView;
use log_gpui::time_histogram::{Lane, LaneBy};
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};

/// 2026-10-19 10:00:00 UTC.
const TEN: f64 = 1_792_404_000.0;

fn text(level: &str, minutes: std::ops::Range<u32>) -> String {
    minutes.map(|m| format!("2026-10-19 10:{m:02}:00 {level} request {m}\n")).collect()
}

fn json(services: &[&str], n: usize, first: usize) -> String {
    (0..n)
        .map(|i| {
            let s = services[i % services.len()];
            let i = first + i;
            format!("{{\"time\":\"2026-10-19T10:{:02}:{:02}Z\",\"level\":\"info\",\"service\":\"{s}\",\"req\":\"r{i}\",\"msg\":\"m{i}\"}}\n", i / 60 % 60, i % 60)
        })
        .collect()
}

fn space(h: &mut Harness, files: &[(&str, String)]) {
    let sources: Vec<ImportSource> = files
        .iter()
        .map(|(name, text)| {
            let path = h.dir.path().join(name);
            std::fs::write(&path, text).unwrap();
            ImportSource::Disk(path.to_string_lossy().into_owned())
        })
        .collect();
    h.view.update(&mut h.cx, |app, cx| app.create_log_space("S", LogSpaceKind::Static, &sources, cx));
    h.cx.run_until_parked();
}

fn lane_names(app: &AppView) -> Vec<String> {
    app.histogram.shown.lanes.as_ref().map_or(Vec::new(), |l| l.order().into_iter().map(|lane| app.lane_name(lane)).collect())
}

/// `api.log` and `api.log.1` (ERROR at 10:31), and `db.log` (ERROR at 10:30).
fn services(h: &mut Harness) {
    space(
        h,
        &[
            ("api.log", text("INFO", 0..6) + &text("ERROR", 31..32)),
            ("api.log.1", text("INFO", 40..44)),
            ("db.log", text("ERROR", 30..31) + &text("INFO", 45..47)),
        ],
    );
}

#[gpui::test]
fn a_log_space_of_several_logs_splits_by_log_name(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    services(&mut h);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.lane_by(), LaneBy::Default);
        assert_eq!(app.lane_column().as_deref(), Some("log_name"));
        assert_eq!(lane_names(app), ["api.log", "db.log"], "the most lines first");
        let lanes = app.histogram.shown.lanes.as_ref().unwrap();
        let grid = app.histogram.shown.grid.unwrap();
        let at = |m: f64| grid.column_of(TEN + m * 60.0).unwrap();
        let (api, db) = (lanes.order()[0], lanes.order()[1]);
        assert_eq!(lanes.level(db)[at(30.0)], 3, "db has a red bar at 10:30");
        assert_eq!(lanes.level(api)[at(31.0)], 3, "api has a red bar at 10:31");
        assert_eq!(lanes.level(api)[at(30.0)], 0);
    });
}

#[gpui::test]
fn clicking_a_bar_goes_to_the_lanes_first_line_there_and_a_name_filters(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    services(&mut h);
    let db = h.view.read_with(&h.cx, |app, _| app.histogram.shown.lanes.as_ref().unwrap().order()[1]);
    let pass_before = h.view.read_with(&h.cx, |app, _| app.filter_pass.clone());
    h.view.update(&mut h.cx, |app, cx| {
        app.histogram_mouse_down(TEN + 30.0 * 60.0 + 1.0, 100.0, 1.0, Some(db), cx);
        app.histogram_mouse_up(cx);
    });
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let row = app.selected_row_pos.expect("a selected row");
        assert!(app.rows().source(row).is_some_and(|s| s.ends_with("db.log")));
        assert_eq!(app.filter_index.effective_timestamp(row), Some(TEN + 30.0 * 60.0));
        assert_eq!(app.filter_pass, pass_before, "the Filter is not changed");
    });

    h.view.update(&mut h.cx, |app, cx| app.lane_name_clicked(db, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        let ticked = app.filter_state.column("log_name").map(|c| c.values.clone());
        assert_eq!(ticked, Some(vec!["db.log".to_string()]));
        let rows = app.rows();
        assert!(app.filter_pass.iter().all(|&r| rows.source(r).is_some_and(|s| s.ends_with("db.log"))));
        assert_eq!(lane_names(app).len(), 2, "the lanes count every line, whatever the Filter");
    });
    // Other and (empty) change nothing.
    h.view.update(&mut h.cx, |app, cx| {
        app.lane_name_clicked(Lane::Other, cx);
        app.lane_name_clicked(Lane::Empty, cx);
    });
    let ticked = h.view.read_with(&h.cx, |app, _| app.filter_state.column("log_name").map(|c| c.values.len()));
    assert_eq!(ticked, Some(1));
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.lane_ticked(db), "the name shows it is ticked");
        assert!(app.lane_tint(db, true).is_some(), "a Log Name lane has its file's color");
    });

    // Clicked again, the value is unticked.
    h.view.update(&mut h.cx, |app, cx| app.lane_name_clicked(db, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.lane_ticked(db));
        assert!(app.filter_state.column("log_name").is_none_or(|c| c.values.is_empty()));
    });
}

#[gpui::test]
fn hovering_a_lane_says_what_is_there(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    services(&mut h);
    let db = h.view.read_with(&h.cx, |app, _| app.histogram.shown.lanes.as_ref().unwrap().order()[1]);
    h.view.update(&mut h.cx, |app, cx| app.lane_hover_at(db, TEN + 30.0 * 60.0 + 1.0, cx));
    let text = h.view.read_with(&h.cx, |app, _| app.lane_hover_text()).expect("a hover text");
    assert!(text.starts_with("db.log · 10:30:00"), "{text}");
    assert!(text.ends_with("1 line · Error"), "{text}");
    h.view.update(&mut h.cx, |app, cx| app.lane_hover_clear(cx));
    assert_eq!(h.view.read_with(&h.cx, |app, _| app.lane_hover_text()), None);
}

#[gpui::test]
fn one_split_log_has_no_lanes_and_suggests_its_module(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    space(&mut h, &[("message.0", json(&["auth", "orders"], 40, 0)), ("message.1", json(&["auth", "billing"], 40, 40))]);
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.lane_column(), None);
        assert!(app.histogram.shown.lanes.is_none());
        // A JSON `service` key is also the line's MODULE, which comes first.
        assert_eq!(app.lane_suggestion().as_deref(), Some("module"));
        assert_eq!(app.lane_choices().first().map(|c| c.key.as_str()), Some("module"), "suggested first");
    });

    h.view.update(&mut h.cx, |app, cx| app.set_lane_by(LaneBy::Column("service".into()), cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(lane_names(app), ["auth", "orders", "billing"]);
        assert_eq!(app.lane_suggestion(), None);
        // Kept with the Log Space, as its Filter is.
        let space = &app.workspace_state.active_project().unwrap().log_spaces[0];
        assert_eq!(space.lane_by, LaneBy::Column("service".into()));
        let saved: log_gpui::workspace::log_space::LogSpace = serde_json::from_str(&serde_json::to_string(space).unwrap()).unwrap();
        assert_eq!(saved.lane_by, LaneBy::Column("service".into()));
    });

    h.view.update(&mut h.cx, |app, cx| app.set_lane_by(LaneBy::None, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| assert!(app.histogram.shown.lanes.is_none()));
}

#[gpui::test]
fn a_column_of_too_many_values_cannot_be_chosen(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    space(&mut h, &[("app.log", json(&["auth"], 1_100, 0))]);
    h.view.read_with(&h.cx, |app, _| {
        let req = app.lane_choices().into_iter().find(|c| c.key == "req").expect("req is listed");
        assert!(req.unavailable.as_deref().is_some_and(|why| why.contains("1000")), "{req:?}");
        assert!(app.lane_choices().iter().any(|c| c.key == "service" && c.unavailable.is_none()));
    });
}

#[gpui::test]
fn a_file_view_has_no_lanes_by_default(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    services(&mut h);
    let (space_id, file_id) = h.view.read_with(&h.cx, |app, _| {
        let space = &app.workspace_state.active_project().unwrap().log_spaces[0];
        (space.id.clone(), space.files[0].file_id.clone())
    });
    h.view.update(&mut h.cx, |app, cx| app.open_file_view_tab(&space_id, &file_id, cx));
    h.cx.run_until_parked();
    h.view.read_with(&h.cx, |app, _| {
        assert_eq!(app.lane_by(), LaneBy::Default);
        assert_eq!(app.lane_column(), None);
    });
}

#[gpui::test]
fn the_swimlanes_are_a_strip_of_their_own(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    services(&mut h);
    let draw = |h: &mut Harness, w: f32| {
        h.cx.simulate_resize(gpui::size(gpui::px(w), gpui::px(900.0)));
        h.cx.run_until_parked();
    };
    let width = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| f32::from(app.histogram.bounds.get().size.width));
    let lanes_width = |h: &mut Harness| h.view.read_with(&h.cx, |app, _| f32::from(app.histogram.lanes_bounds.get().size.width));
    draw(&mut h, 1400.0);
    // Both strips drawn, their axes the same width: they line up.
    let (histogram, lanes) = (width(&mut h), lanes_width(&mut h));
    assert!(histogram > 0.0 && (histogram - lanes).abs() < 1.0, "{histogram} vs {lanes}");

    // Lanes off: the histogram stays, takes back the room of the names, and
    // no lanes are counted.
    h.view.update(&mut h.cx, |app, cx| app.set_show_lanes(false, cx));
    draw(&mut h, 1401.0);
    h.view.read_with(&h.cx, |app, _| {
        assert!(app.histogram_shown() && !app.lanes_shown());
        assert!(app.histogram.shown.lanes.is_none());
    });
    assert!(width(&mut h) > histogram + 80.0, "{} vs {histogram}", width(&mut h));

    // Histogram off, lanes on: the lanes still count and draw on the axis.
    h.view.update(&mut h.cx, |app, cx| {
        app.set_show_lanes(true, cx);
        app.set_show_histogram(false, cx);
    });
    draw(&mut h, 1402.0);
    h.view.read_with(&h.cx, |app, _| {
        assert!(!app.histogram_shown() && app.lanes_shown());
        assert_eq!(lane_names(app), ["api.log", "db.log"]);
    });
    assert!(lanes_width(&mut h) > 0.0);

    // Each strip folds on its own.
    h.view.update(&mut h.cx, |app, cx| app.toggle_lanes_collapsed(cx));
    h.view.read_with(&h.cx, |app, _| assert!(app.histogram.lanes_collapsed && !app.histogram.collapsed));
}
