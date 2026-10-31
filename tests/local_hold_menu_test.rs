//! A local file is read whole through its Line Index: it has none of the
//! Remote file's hold controls (no lines held, no Clear), and `hold_info`
//! says so.
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
