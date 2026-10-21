mod common;

use common::Harness;
use gpui::TestAppContext;
use log_gpui::space_view::EmptyCanvas;
use log_gpui::workspace::log_space::{ImportSource, LogSpaceKind};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

fn empty_canvas(h: &mut Harness) -> Option<EmptyCanvas> {
    h.view.read_with(&h.cx, |app, _| app.empty_canvas())
}

fn create_space(h: &mut Harness, sources: &[ImportSource]) {
    h.view.update(&mut h.cx, |app, cx| {
        app.create_log_space("Test", LogSpaceKind::Imported, sources, cx);
    });
    h.cx.run_until_parked();
}

#[test]
fn every_state_explains_itself() {
    for state in [
        EmptyCanvas::NoLogSpace,
        EmptyCanvas::NoFiles { can_add: true },
        EmptyCanvas::NoFiles { can_add: false },
        EmptyCanvas::LoadFailed { errors: vec!["app.log was not found".into()] },
    ] {
        assert!(!state.title().is_empty(), "{state:?}");
        assert!(!state.detail().is_empty(), "{state:?}");
    }
    assert!(EmptyCanvas::NoLogSpace.detail().contains("Log Space"));
    let failed = EmptyCanvas::LoadFailed { errors: vec!["a.log was not found".into(), "b.log was not found".into()] };
    assert!(failed.detail().contains("a.log") && failed.detail().contains("b.log"));
}

#[gpui::test]
fn nothing_open_shows_no_log_space(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    assert_eq!(empty_canvas(&mut h), Some(EmptyCanvas::NoLogSpace));
}

#[gpui::test]
fn log_space_without_files_offers_add_files(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    create_space(&mut h, &[]);
    assert_eq!(empty_canvas(&mut h), Some(EmptyCanvas::NoFiles { can_add: true }));
}

#[gpui::test]
fn log_space_with_lines_shows_rows(cx: &mut TestAppContext) {
    let mut h = common::open_app_with_lines(cx, &common::sample_lines(3));
    assert_eq!(empty_canvas(&mut h), None);
}

#[cfg(unix)]
#[gpui::test]
fn unreadable_source_file_shows_its_error_on_the_canvas(cx: &mut TestAppContext) {
    let mut h = common::open_app(cx);
    let path = h.dir.path().join("locked.log");
    std::fs::write(&path, "2026-10-19 10:00:01 INFO hello\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(&path).is_ok() {
        // Running as root: permissions don't stop reads, nothing to check.
        return;
    }
    create_space(&mut h, &[ImportSource::Disk(path.to_string_lossy().into_owned())]);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    match empty_canvas(&mut h) {
        Some(EmptyCanvas::LoadFailed { errors }) => {
            assert_eq!(errors.len(), 1);
            assert!(errors[0].contains("locked.log"), "{errors:?}");
        }
        other => panic!("expected LoadFailed, got {other:?}"),
    }
    // The Notice bar still shows it too.
    let notice = h.view.read_with(&h.cx, |app, _| app.notice.clone());
    assert!(notice.is_some_and(|n| n.contains("locked.log")));
}
