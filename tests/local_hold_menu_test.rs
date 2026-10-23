//! A local file followed live offers Load more and Back to Live Window (and
//! no Clear), through `hold_info`.
mod common;

use gpui::TestAppContext;

#[gpui::test]
fn a_local_live_file_has_hold_info_without_clear(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(30));
    // The source says it is Live once its thread has opened the file.
    for _ in 0..200 {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        let live = h.view.read_with(&h.cx, |app, _| app.live_file_statuses().iter().all(|(_, s)| *s == log_gpui::live::SourceStatus::Live));
        if live {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let info = h.view.read_with(&h.cx, |app, _| {
        let id = app.parsed_files.keys().next().unwrap().clone();
        app.hold_info(&id)
    });
    let info = info.expect("a local file followed live has hold info");
    assert!(info.local && !info.command);
    assert_eq!(info.lines, 30);
    assert_eq!(info.load_more_off, Some("Already at the first line"));
    assert!(!info.extended);
}

fn info(at_start: bool, command: bool, off: Option<&'static str>) -> log_gpui::app::live_more::HoldInfo {
    log_gpui::app::live_more::HoldInfo {
        lines: 3_245,
        extended: false,
        at_start,
        loading: false,
        remaining: 1_000_000,
        load_more_off: off,
        command,
        local: true,
    }
}

#[test]
fn load_more_is_offered_only_while_older_lines_are_left() {
    assert!(!info(true, false, Some("Already at the first line")).offers_load_more(), "read whole");
    assert!(info(false, false, None).offers_load_more(), "older lines left the Live Window");
    let at_limit = info(false, false, Some(log_gpui::app::live_more::AT_FULL_READ_LIMIT));
    assert!(at_limit.offers_load_more(), "at the Full Read Limit the button stays, with its reason");
    assert!(!info(false, true, None).offers_load_more(), "a Remote Command is never read again");
}

#[gpui::test]
fn a_local_file_read_whole_offers_no_load_more(cx: &mut gpui::TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(50));
    let id = h.view.read_with(&h.cx, |app, _| app.active_space().unwrap().files[0].file_id.clone());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let info = loop {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        if let Some(info) = h.view.read_with(&h.cx, |app, _| app.hold_info(&id)) {
            break info;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(!info.offers_load_more(), "{info:?}");
}
