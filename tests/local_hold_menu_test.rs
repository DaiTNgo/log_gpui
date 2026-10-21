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
