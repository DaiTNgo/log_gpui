mod common;

use gpui::TestAppContext;
use log_gpui::app::format_count;

#[test]
fn format_count_groups_thousands() {
    assert_eq!(format_count(0), "0");
    assert_eq!(format_count(999), "999");
    assert_eq!(format_count(1_000), "1,000");
    assert_eq!(format_count(12_345), "12,345");
    assert_eq!(format_count(1_200_000), "1,200,000");
}

fn labels(h: &common::Harness) -> Vec<String> {
    h.view.read_with(&h.cx, |app, _| app.status_bar_labels())
}

#[gpui::test]
fn status_bar_shows_line_count(cx: &mut TestAppContext) {
    let h = common::open_app_with_lines(cx, &common::sample_lines(1_400));
    assert!(labels(&h).contains(&"1,400 lines".to_string()), "{:?}", labels(&h));
}

#[gpui::test]
fn with_nothing_selected_and_no_focus_only_the_count_shows(cx: &mut TestAppContext) {
    let h = common::open_app_with_lines(cx, &common::sample_lines(400));
    assert_eq!(labels(&h), vec!["400 lines".to_string()]);
}

#[gpui::test]
fn a_selected_line_shows_its_number(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(1_400));
    h.view.update(&mut h.cx, |app, cx| app.jump_to_row(1_233, cx));
    assert_eq!(labels(&h)[0], "Ln 1,234");
}

#[gpui::test]
fn in_a_space_of_several_files_the_line_names_its_file(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let mut sources = Vec::new();
    for name in ["a.log", "b.log"] {
        let path = h.dir.path().join(name);
        std::fs::write(&path, common::sample_lines(3).join("\n")).unwrap();
        sources.push(log_gpui::workspace::log_space::ImportSource::Disk(path.to_string_lossy().into_owned()));
    }
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Two", log_gpui::workspace::log_space::LogSpaceKind::Static, &sources, cx);
    });
    h.cx.run_until_parked();
    h.view.update(&mut h.cx, |app, cx| app.jump_to_row(0, cx));
    let first = labels(&h)[0].clone();
    assert!(first == "a.log · Ln 1" || first == "b.log · Ln 1", "{first}");
}

#[gpui::test]
fn the_focus_count_shows_while_the_focus_pane_lists_lines(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(1_200));
    h.view.update_in(&mut h.cx, |app, window, cx| app.activate_search(window, cx));
    h.cx.simulate_input("line 11");
    h.cx.executor().advance_clock(std::time::Duration::from_millis(150));
    h.cx.run_until_parked();
    // line 11, line 110…119, line 1100…1199
    assert!(labels(&h).contains(&"111 lines in Focus".to_string()), "{:?}", labels(&h));

    h.view.update(&mut h.cx, |app, cx| app.toggle_focus_pane(cx));
    assert!(!labels(&h).iter().any(|l| l.contains("Focus")), "{:?}", labels(&h));
}

#[gpui::test]
fn local_files_are_live_once_open(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(40));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        h.cx.run_until_parked();
        let status = h.view.read_with(&h.cx, |app, _| app.worst_live_status());
        if matches!(status, Some((log_gpui::live::SourceStatus::Live, _, _))) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "still {status:?}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
