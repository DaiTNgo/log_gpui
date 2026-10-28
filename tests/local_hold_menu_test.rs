//! A local file is read whole through its Line Index: it has none of the
//! Remote file's hold controls (no Load more, no Back to Live Window, no
//! Clear), and `hold_info` says so.
mod common;

use gpui::TestAppContext;

#[gpui::test]
fn a_local_file_has_no_hold_info(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_live_lines(cx, &common::sample_lines(30));
    // The source says it is Live once its thread has opened the file.
    for _ in 0..200 {
        h.view.update(&mut h.cx, |app, cx| {
            app.pump_live(cx);
        });
        let live = h.view.read_with(&h.cx, |app, _| {
            app.live_file_statuses().iter().all(|(_, s)| *s == log_gpui::live::SourceStatus::Live)
        });
        if live {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    h.view.read_with(&h.cx, |app, _| {
        let id = app.parsed_files.keys().next().unwrap().clone();
        assert!(app.live.sources.contains_key(&id), "it is followed live");
        assert!(app.hold_info(&id).is_none(), "but holds every line: nothing to load or clear");
    });
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

