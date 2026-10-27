//! Sessions saved with the kinds of Log Space before Static and Live
//! (openspec change `static-live-log-spaces`, tasks 1.1 and 1.2).

mod common;

use common::{Harness, open_app_in, open_app_with_session};
use gpui::TestAppContext;
use log_gpui::workspace::log_space::{LogFile, LogSpaceKind};
use log_gpui::workspace::project::ProjectColorAccent;
use log_gpui::workspace::session::{IMPORTED_NOW_STATIC, SessionManifest};
use log_gpui::workspace::state::WorkspaceState;

/// A session as saved before Static and Live, of one space made by `fill`.
fn old_session(kind: LogSpaceKind, fill: impl FnOnce(&mut log_gpui::workspace::log_space::LogSpace)) -> String {
    let mut ws = WorkspaceState::new();
    ws.create_project("Prod", ProjectColorAccent::Blue);
    let space = ws.active_project_mut().unwrap().create_log_space("Payment", kind).unwrap();
    fill(space);
    SessionManifest::from_workspace(&ws)
        .to_json()
        .unwrap()
        .replace("\"kind\": \"Static\"", "\"kind\": \"Imported\"")
        .replace("\"kind\": \"Live\"", "\"kind\": \"Remote\"")
}

fn notice(h: &mut Harness) -> Option<String> {
    h.view.read_with(&h.cx, |app, _| app.notice.clone())
}

#[gpui::test]
fn an_old_imported_space_with_local_files_opens_as_static_with_a_notice_once(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("api.log");
    std::fs::write(&path, "2026-09-05 12:00:00 [INFO] started\n").unwrap();
    let session = old_session(LogSpaceKind::Static, |s| {
        s.add_imported_files(&[path.to_string_lossy().into_owned()]).unwrap();
    });

    let mut h = open_app_with_session(cx, dir, &session);
    assert_eq!(notice(&mut h).as_deref(), Some(IMPORTED_NOW_STATIC));
    let kind = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces[0].kind);
    assert_eq!(kind, LogSpaceKind::Static);

    h.view.update(&mut h.cx, |app, _| app.save_session_now());
    let Harness { dir, .. } = h;
    let mut again = open_app_in(cx, dir);
    assert_eq!(notice(&mut again), None, "the session was saved with the new kinds");
}

#[gpui::test]
fn an_old_imported_space_of_archive_entries_opens_without_the_notice(cx: &mut TestAppContext) {
    let session = old_session(LogSpaceKind::Static, |s| {
        s.files.push(LogFile::from_archive_entry("/tmp/logs.tar.gz", "a.log"));
    });
    let mut h = open_app_with_session(cx, tempfile::tempdir().unwrap(), &session);
    assert_eq!(notice(&mut h), None);
}

#[gpui::test]
fn an_old_remote_space_opens_as_live_without_the_notice(cx: &mut TestAppContext) {
    let session = old_session(LogSpaceKind::Live, |_| {});
    let mut h = open_app_with_session(cx, tempfile::tempdir().unwrap(), &session);
    assert_eq!(notice(&mut h), None);
    let kind = h.view.read_with(&h.cx, |app, _| app.workspace_state.active_project().unwrap().log_spaces[0].kind);
    assert_eq!(kind, LogSpaceKind::Live);
}
